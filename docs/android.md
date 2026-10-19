# Android: the Anbernic RG DS under GammaOS Next

Work in progress. The goal is Pocket Native on the RG DS (RK3568, four
Cortex-A55 cores, Mali-G52, two 640x480 touch screens) under GammaOS Next
(Android 14), with the 3DS's top screen on the top panel and its bottom
screen, touch included, on the bottom one.

## What carries over and what does not

The engine, the recompiler, the OpenGL ES renderer, the menu and the
per-game profiles are plain Rust and C and build for Android as they are.
Android has `libEGL.so` and `libGLESv2.so`, which the renderer already falls
back to.

Everything that talks to the handheld is Linux-only and needs an Android
counterpart:

| ArkOS | Android |
|---|---|
| KMS/GBM or `/dev/fb0` for the panel | an `ANativeWindow` per display, from an activity |
| evdev for buttons and sticks | Android key and motion events |
| the right stick and R2 for touch | real touch on the bottom display |
| ALSA | AAudio |
| `power.sh` writing sysfs | GammaOS's performance profiles, or `su` with Magisk |
| `title.so` built by gcc against glibc | `title.so` built by the NDK's clang against bionic |
| `title.so` loaded from the card | packaged in the APK's `lib/arm64-v8a/`, since Android 10 and later will not load code from an app's data folder |

## Phases

1. **Headless bring-up (this branch).** Build `pocket-native` and a game's
   `title.so` for Android, push them with adb and run `--probe` from the
   shell. Drawing goes to an offscreen EGL surface on the handheld's own
   Mali driver, so this tests bionic, fastmem, the recompiled code and the
   renderer before any app exists.
2. **One-screen APK.** A NativeActivity (or the `android-activity` crate)
   with an `ANativeWindow` output next to `Output::Kms` and `Output::Fbdev`
   in `engine/gpu/src/raster/gles.rs`, input from Android events in place of
   `frontend/src/input.rs`'s evdev reader, AAudio in `frontend/src/sound.rs`.
   The existing one-panel layouts work as they are.
3. **Two screens.** A second activity or `Presentation` on display 1 hands
   its window to the GPU thread; the top screen fills the top panel
   (400x240 at 1.6 times, 640x384) and the bottom screen the bottom panel at
   exactly twice its size, with touch going straight to the 3DS.
4. **Polish.** The build makes APKs, the menu opens with L3+R2 (a short
   press of the RG button belongs to the system), power through GammaOS's
   profiles.

## Phase 1: running a probe

What changed for Android so far:

- fastmem (`engine/core/src/memory/`) builds on Android too. It was behind
  `cfg(target_os = "linux")`, which is false on Android, and would have
  quietly turned itself off.
- the host profiler (`frontend/src/profiler.rs`) names `ITIMER_PROF`
  itself, which the libc crate leaves out on Android.
- `scripts/android/`: a build container with the NDK, the build, and a
  probe over adb. CI cross-builds `pocket-native` for
  `aarch64-linux-android` on every push.

On a Linux PC with podman or Docker, and adb (Android platform-tools):

```bash
scripts/android/build.sh path/to/game.3ds
scripts/android/probe.sh <title id>            # 3600 frames
scripts/android/probe.sh <title id> 600        # a quicker look
```

The first build makes the container, about 1 GB more than the ArkOS one
because of the NDK. The build prints the title id. The probe pushes
`build/Pocket Native Android/pocket-native` to `/data/local/tmp/pocket-native`
on the handheld (later pushes skip what has not changed), runs the game
there and pulls the report into `build/probe-<title id>-android`, with
`probe-log.txt`, the screenshots and `device.txt` saying what drew them.

Turn on USB debugging in GammaOS first: Settings, About, tap the build
number seven times, then Developer options, USB debugging.

What to look for in the report:

- `probe-log.txt` should log `Pocket Native` and the title, then a
  `speed` line every 300 frames. A load error naming `libEGL` or
  `OpenGL ES 3.1` is the driver; a crash at once with no `speed` line is
  most likely the title's library or fastmem.
- the screenshots show whether the renderer draws right on the Mali-G52.
- the speeds are as fast as it goes with nothing waiting for the panel, so
  above 100% means there is room left.

## First results

Pokémon Alpha Sapphire on an RG DS under GammaOS Next 1.4 (Android 14,
Mali-G52 r25p0): the recompiled code loads and the game boots and draws with
no errors. The first runs of a scene go at 18 to 84% while the Mali driver
compiles its shaders; once the shader manifest has them, the title screen and
intro run at 200 to 300% of the console's speed.
