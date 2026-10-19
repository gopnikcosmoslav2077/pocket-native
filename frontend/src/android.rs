//! Pocket Native as an Android app: a NativeActivity whose window the
//! screens go to, whose key and motion events become the pad's buttons,
//! sticks and touch, and whose files directory holds the game.
//!
//! The app's files directory, /sdcard/Android/data/<package>/files, holds
//! game.3ds, sysdata/shared_font.bin and titles/, which the build's install
//! script pushes there; the frontend keeps data/ (saves, log) and config/
//! beside them. The title's recompiled code comes in the APK as
//! libtitle.so, loaded by name from the app's own libraries.

use std::path::{Path, PathBuf};
use std::sync::atomic::Ordering;
use std::time::Duration;

use android_activity::input::{Axis, InputEvent, KeyAction, Keycode, MotionAction};
use android_activity::{AndroidApp, InputStatus, MainEvent, PollEvent};

use crate::input::{self, Pad, A, B, DOWN, FN, L1, L2, L3, LEFT, R1, R2, R3, RIGHT, SELECT, START, UP, X, Y};

/// how far the frontend's sticks go, as input::stick takes them.
const STICK: f32 = 1800.0;
/// AINPUT_SOURCE_CLASS_JOYSTICK and AINPUT_SOURCE_TOUCHSCREEN's bits.
const JOYSTICK: u32 = 0x0000_0010;
const TOUCHSCREEN: u32 = 0x0000_1002;

#[no_mangle]
fn android_main(app: AndroidApp) {
    let files = app.external_data_path().or_else(|| app.internal_data_path()).unwrap_or_else(|| PathBuf::from("/data/local/tmp/pocket-native"));
    let data = files.join("data");
    let _ = std::fs::create_dir_all(&data);
    let _ = std::fs::create_dir_all(files.join("config"));
    set_up(&files, &data);
    std::panic::set_hook(Box::new(|info| {
        log::error!("{info}");
        let _ = std::fs::write("data/panic.txt", info.to_string());
    }));

    // the screens can only go up once the activity has its window
    let mut window = None;
    while window.is_none() {
        app.poll_events(Some(Duration::from_millis(100)), |event| {
            if let PollEvent::Main(MainEvent::InitWindow { .. }) = event {
                window = app.native_window();
            }
        });
    }
    let window = window.unwrap();
    let (window_width, window_height) = (window.width(), window.height());
    zakuro_gpu::set_window(window.ptr().as_ptr().cast());
    for axis in [Axis::Z, Axis::Rz, Axis::Rx, Axis::Ry, Axis::HatX, Axis::HatY, Axis::Ltrigger, Axis::Rtrigger, Axis::Gas, Axis::Brake] {
        app.enable_motion_axis(axis);
    }

    let pad = input::share();
    let rom = files.join("game.3ds").to_string_lossy().into_owned();
    let game = std::thread::Builder::new().name("game".into()).spawn(move || crate::run(vec![rom])).expect("a thread for the game");

    loop {
        app.poll_events(Some(Duration::from_millis(8)), |event| {
            // the window going away, the app sent to the background or closed,
            // ends the game as quitting from the menu does
            if let PollEvent::Main(MainEvent::TerminateWindow { .. } | MainEvent::Destroy | MainEvent::Pause) = event {
                input::QUIT.store(true, Ordering::Relaxed);
            }
        });
        if let Ok(mut events) = app.input_events_iter() {
            while events.next(|event| handle(event, &pad, (window_width, window_height))) {}
        }
        // run only returns when it panicked, it ends the process otherwise
        if game.is_finished() {
            std::process::exit(1);
        }
    }
}

/// the frontend's places and switches, as run.sh gives them on ArkOS.
fn set_up(files: &Path, data: &Path) {
    let _ = std::env::set_current_dir(files);
    std::env::set_var("POCKET_NATIVE_LOG", data.join("log.txt"));
    std::env::set_var("ZAKURO_FAST", "1");
    std::env::set_var("ZAKURO_DATA", data);
    // by name, the app's own libraries are where the system looks first
    std::env::set_var("ZAKURO_RECOMPILED", "libtitle.so");
    std::env::set_var("ZAKURO_SHADER_MANIFEST", data.join("shaders.txt"));
    std::env::set_var("POCKET_NATIVE_TITLES", files.join("titles"));
    std::env::set_var("POCKET_NATIVE_CONFIG", files.join("config"));
    let drops = data.join("drops.txt");
    if let Ok(rules) = std::fs::read_to_string(drops) {
        std::env::set_var("ZAKURO_DROP_DRAWS", rules.trim());
    }
}

/// one event into the pad, handled when it was the gamepad's or the touch
/// screen's, so that the system does not take B for back.
fn handle(event: &InputEvent, pad: &std::sync::Mutex<Pad>, window: (i32, i32)) -> InputStatus {
    match event {
        InputEvent::KeyEvent(key) => {
            let Some(code) = button(key.key_code()) else { return InputStatus::Unhandled };
            let mut pad = pad.lock().unwrap();
            match key.action() {
                KeyAction::Down => {
                    pad.keys.insert(code);
                }
                KeyAction::Up => {
                    pad.keys.remove(&code);
                }
                _ => {}
            }
            InputStatus::Handled
        }
        InputEvent::MotionEvent(motion) => {
            let source: u32 = motion.source().into();
            let mut pad = pad.lock().unwrap();
            if source & TOUCHSCREEN == TOUCHSCREEN {
                pad.touch = match motion.action() {
                    MotionAction::Down | MotionAction::Move | MotionAction::PointerDown => {
                        let pointer = motion.pointer_at_index(0);
                        Some(to_layout(pointer.x(), pointer.y(), window))
                    }
                    _ => None,
                };
                InputStatus::Handled
            } else if source & JOYSTICK == JOYSTICK {
                let pointer = motion.pointer_at_index(0);
                let axis = |axis: Axis| pointer.axis_value(axis);
                // the right stick is Z and RZ on most Android pads, RX and RY on some
                let (right_x, right_y) = if axis(Axis::Z) == 0.0 && axis(Axis::Rz) == 0.0 {
                    (axis(Axis::Rx), axis(Axis::Ry))
                } else {
                    (axis(Axis::Z), axis(Axis::Rz))
                };
                pad.axes[0] = (axis(Axis::X) * STICK) as i32;
                pad.axes[1] = (axis(Axis::Y) * STICK) as i32;
                pad.axes[3] = (right_x * STICK) as i32;
                pad.axes[4] = (right_y * STICK) as i32;
                // triggers and a d-pad that come as axes, as buttons
                let held = [
                    (L2, axis(Axis::Ltrigger).max(axis(Axis::Brake)) > 0.5),
                    (R2, axis(Axis::Rtrigger).max(axis(Axis::Gas)) > 0.5),
                    (LEFT, axis(Axis::HatX) < -0.5),
                    (RIGHT, axis(Axis::HatX) > 0.5),
                    (UP, axis(Axis::HatY) < -0.5),
                    (DOWN, axis(Axis::HatY) > 0.5),
                ];
                for (code, down) in held {
                    if down {
                        pad.keys.insert(code);
                    } else if !HATS_AS_KEYS.load(Ordering::Relaxed) {
                        pad.keys.remove(&code);
                    }
                }
                InputStatus::Handled
            } else {
                InputStatus::Unhandled
            }
        }
        _ => InputStatus::Unhandled,
    }
}

/// set once the d-pad has come as keys, after which the hat's axes no
/// longer let go of keys the d-pad holds.
static HATS_AS_KEYS: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// an Android key as the R36S numbers it, which the frontend goes by.
fn button(key: Keycode) -> Option<u16> {
    let code = match key {
        Keycode::ButtonA => A,
        Keycode::ButtonB => B,
        Keycode::ButtonX => X,
        Keycode::ButtonY => Y,
        Keycode::ButtonL1 => L1,
        Keycode::ButtonR1 => R1,
        Keycode::ButtonL2 => L2,
        Keycode::ButtonR2 => R2,
        Keycode::ButtonThumbl => L3,
        Keycode::ButtonThumbr => R3,
        Keycode::ButtonSelect => SELECT,
        Keycode::ButtonStart => START,
        Keycode::ButtonMode | Keycode::Menu => FN,
        Keycode::DpadUp | Keycode::DpadDown | Keycode::DpadLeft | Keycode::DpadRight => {
            HATS_AS_KEYS.store(true, Ordering::Relaxed);
            match key {
                Keycode::DpadUp => UP,
                Keycode::DpadDown => DOWN,
                Keycode::DpadLeft => LEFT,
                _ => RIGHT,
            }
        }
        _ => return None,
    };
    Some(code)
}

/// a place in the window in the layout's size, which the renderer scales to
/// fit the window and centres.
fn to_layout(x: f32, y: f32, (width, height): (i32, i32)) -> (f32, f32) {
    let (layout_width, layout_height) = zakuro_gpu::layout_size();
    let scale = (width as f32 / layout_width as f32).min(height as f32 / layout_height as f32);
    let left = (width as f32 - layout_width as f32 * scale) / 2.0;
    let top = (height as f32 - layout_height as f32 * scale) / 2.0;
    ((x - left) / scale, (y - top) / scale)
}
