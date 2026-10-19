//! the title's sound through ALSA, or AAudio on Android, stretched in time
//! to keep the device's buffer about a tenth of a second full whatever the
//! frame rate.

use std::sync::mpsc;

use zakuro_core::AUDIO_SAMPLE_RATE;

/// where the sound goes: how many frames it holds that have not played yet,
/// and taking more, all of them before it returns.
trait Device {
    /// frames queued, at the title's rate.
    fn queued(&mut self) -> i64;
    fn write(&mut self, frames: &[[i16; 2]]);
    /// how much to keep queued, in seconds, which is about how far the
    /// sound lags the picture.
    fn target(&self) -> f64 {
        0.1
    }
}

pub fn play(samples: mpsc::Receiver<Vec<[i16; 2]>>) {
    std::thread::spawn(move || {
        let Some(mut device) = open() else {
            for _ in samples {}
            return;
        };
        let target = AUDIO_SAMPLE_RATE * device.target();
        let mut stretch = Stretch::default();
        let mut ratio = 1.0f64;
        let mut chunk = Vec::new();
        for samples in samples {
            let queued = device.queued();
            let wanted = (1.0 + (target - queued as f64) / target).clamp(0.75, 2.5);
            ratio += (wanted - ratio) * 0.1;
            chunk.clear();
            stretch.push(&samples, ratio, &mut chunk);
            device.write(&chunk);
        }
    });
}

#[cfg(not(target_os = "android"))]
fn open() -> Option<Box<dyn Device>> {
    Alsa::open().map(|alsa| Box::new(alsa) as Box<dyn Device>)
}

#[cfg(target_os = "android")]
fn open() -> Option<Box<dyn Device>> {
    AAudio::open().map(|aaudio| Box::new(aaudio) as Box<dyn Device>)
}

/// the sound through ALSA's default device, from libasound when the system
/// has it.
#[cfg(not(target_os = "android"))]
struct Alsa {
    _library: libloading::Library,
    pcm: *mut u8,
    write: unsafe extern "C" fn(*mut u8, *const u8, u64) -> i64,
    recover: unsafe extern "C" fn(*mut u8, i32, i32) -> i32,
    delay: unsafe extern "C" fn(*mut u8, *mut i64) -> i32,
}

#[cfg(not(target_os = "android"))]
impl Alsa {
    fn open() -> Option<Alsa> {
        // SAFETY: plain ALSA calls on the device this opens, with buffers
        // that live as long as each call, and functions kept no longer than
        // the library they came from
        unsafe {
            let Ok(alsa) = libloading::Library::new("libasound.so.2") else {
                log::warn!("no libasound, playing without sound");
                return None;
            };
            let open: libloading::Symbol<unsafe extern "C" fn(*mut *mut u8, *const u8, i32, i32) -> i32> = alsa.get(b"snd_pcm_open\0").ok()?;
            let set: libloading::Symbol<unsafe extern "C" fn(*mut u8, i32, i32, u32, u32, i32, u32) -> i32> = alsa.get(b"snd_pcm_set_params\0").ok()?;
            let write = *alsa.get::<unsafe extern "C" fn(*mut u8, *const u8, u64) -> i64>(b"snd_pcm_writei\0").ok()?;
            let recover = *alsa.get::<unsafe extern "C" fn(*mut u8, i32, i32) -> i32>(b"snd_pcm_recover\0").ok()?;
            let delay = *alsa.get::<unsafe extern "C" fn(*mut u8, *mut i64) -> i32>(b"snd_pcm_delay\0").ok()?;
            let mut pcm = std::ptr::null_mut();
            if open(&mut pcm, b"default\0".as_ptr(), 0, 0) < 0 || set(pcm, 2, 3, 2, AUDIO_SAMPLE_RATE.round() as u32, 1, 150_000) < 0 {
                log::warn!("could not open the sound device, playing without sound");
                return None;
            }
            Some(Alsa { _library: alsa, pcm, write, recover, delay })
        }
    }
}

#[cfg(not(target_os = "android"))]
impl Device for Alsa {
    fn queued(&mut self) -> i64 {
        let mut queued = 0i64;
        // SAFETY: the device open made, and a number for ALSA to fill
        if unsafe { (self.delay)(self.pcm, &mut queued) } < 0 {
            queued = 0;
        }
        queued
    }

    fn write(&mut self, frames: &[[i16; 2]]) {
        let mut done = 0;
        while done < frames.len() {
            // SAFETY: the frames left, interleaved stereo as the device was set up
            let written = unsafe { (self.write)(self.pcm, frames[done..].as_ptr().cast(), (frames.len() - done) as u64) };
            if written < 0 {
                // SAFETY: as above
                if unsafe { (self.recover)(self.pcm, written as i32, 1) } < 0 {
                    break;
                }
            } else {
                done += written as usize;
            }
        }
    }
}

/// the sound through AAudio at the device's own rate, which the title's is
/// resampled to here: a stream at the title's rate would have the system
/// resample it, and lose the low-latency path for it.
#[cfg(target_os = "android")]
struct AAudio {
    stream: ndk::audio::AudioStream,
    /// the device's frames to one of the title's.
    step: f64,
    /// where the resampler is between the last frame and the next.
    at: f64,
    last: [f32; 2],
    out: Vec<[i16; 2]>,
}

#[cfg(target_os = "android")]
impl AAudio {
    fn open() -> Option<AAudio> {
        use ndk::audio::{AudioDirection, AudioFormat, AudioPerformanceMode, AudioSharingMode, AudioStreamBuilder};
        let opened = AudioStreamBuilder::new().and_then(|builder| {
            builder
                .direction(AudioDirection::Output)
                .format(AudioFormat::PCM_I16)
                .channel_count(2)
                .sharing_mode(AudioSharingMode::Shared)
                .performance_mode(AudioPerformanceMode::LowLatency)
                .open_stream()
        });
        let stream = match opened {
            Ok(stream) => stream,
            Err(error) => {
                log::warn!("could not open AAudio, playing without sound, {error:?}");
                return None;
            }
        };
        // two bursts, the least the device plays without gaps, play keeping
        // its own queue on top
        let burst = stream.frames_per_burst().max(64);
        let _ = stream.set_buffer_size_in_frames(burst * 2);
        if let Err(error) = stream.request_start() {
            log::warn!("could not start AAudio, playing without sound, {error:?}");
            return None;
        }
        let rate = stream.sample_rate().max(1);
        log::info!("sound through AAudio at {rate} Hz, {burst} frames a burst, {:?}", stream.performance_mode());
        Some(AAudio { step: AUDIO_SAMPLE_RATE / rate as f64, stream, at: 0.0, last: [0.0; 2], out: Vec::new() })
    }
}

#[cfg(target_os = "android")]
impl Device for AAudio {
    fn queued(&mut self) -> i64 {
        let device = (self.stream.frames_written() - self.stream.frames_read()).max(0);
        (device as f64 * self.step) as i64
    }

    fn target(&self) -> f64 {
        0.05
    }

    fn write(&mut self, frames: &[[i16; 2]]) {
        // linear between the title's frames, at the device's rate
        self.out.clear();
        for frame in frames {
            let next = frame.map(f32::from);
            while self.at < 1.0 {
                let t = self.at as f32;
                self.out.push([0, 1].map(|channel| (self.last[channel] + (next[channel] - self.last[channel]) * t) as i16));
                self.at += self.step;
            }
            self.at -= 1.0;
            self.last = next;
        }
        let mut done = 0;
        while done < self.out.len() {
            // SAFETY: the frames left, interleaved stereo 16-bit as the stream
            // was opened, waiting at most a tenth of a second for room
            match unsafe { self.stream.write(self.out[done..].as_ptr().cast(), (self.out.len() - done) as i32, 100_000_000) } {
                Ok(0) | Err(_) => break,
                Ok(written) => done += written as usize,
            }
        }
    }
}

const SEGMENT: usize = 1024;
const OVERLAP: usize = 256;
const SEEK: usize = 192;
const HOP: usize = SEGMENT - OVERLAP;

/// stretches sound in time without changing its pitch, WSOLA: segments of
/// the input are laid one after another, each moved within a small window to
/// where it best continues the last and crossfaded into it.
#[derive(Default)]
struct Stretch {
    input: Vec<[f32; 2]>,
    at: f64,
    tail: Vec<[f32; 2]>,
}

impl Stretch {
    fn push(&mut self, samples: &[[i16; 2]], ratio: f64, out: &mut Vec<[i16; 2]>) {
        self.input.extend(samples.iter().map(|&[l, r]| [l as f32, r as f32]));
        let step = HOP as f64 / ratio;
        loop {
            let base = self.at.round() as usize;
            if base + SEEK + SEGMENT > self.input.len() {
                break;
            }
            let mut best = (f32::MIN, base);
            if self.tail.is_empty() {
                best.1 = base;
            } else {
                for start in (base.saturating_sub(SEEK)..=base + SEEK).step_by(4) {
                    let score: f32 = (0..OVERLAP)
                        .step_by(2)
                        .map(|i| {
                            let (a, b) = (self.tail[i], self.input[start + i]);
                            (a[0] + a[1]) * (b[0] + b[1])
                        })
                        .sum();
                    if score > best.0 {
                        best = (score, start);
                    }
                }
            }
            let start = best.1;
            for i in 0..HOP {
                let sample = self.input[start + i];
                let mixed = if i < OVERLAP && !self.tail.is_empty() {
                    let w = i as f32 / OVERLAP as f32;
                    [self.tail[i][0] * (1.0 - w) + sample[0] * w, self.tail[i][1] * (1.0 - w) + sample[1] * w]
                } else {
                    sample
                };
                out.push(mixed.map(|value| value.clamp(-32768.0, 32767.0) as i16));
            }
            self.tail = self.input[start + HOP..start + SEGMENT].to_vec();
            self.at += step;
        }
        let used = (self.at as usize).saturating_sub(SEEK);
        if used > 0 {
            self.input.drain(..used);
            self.at -= used as f64;
        }
    }
}
