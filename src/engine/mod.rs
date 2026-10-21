mod audio;
mod decode;
pub(crate) mod probe;

pub use audio::{AudioClock, AudioOutput};
pub use decode::{DecodeSession, EngineEvent, VideoFrame};
pub use probe::MediaInfo;

use std::path::PathBuf;
use std::sync::atomic::Ordering;
use std::sync::mpsc::{channel, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex};
use std::time::Instant;

pub use decode::{AUDIO_CHANNELS, AUDIO_RATE};

/// Extracts the first attached picture stream from `path` as PNG bytes.
pub fn cover_art(path: &std::path::Path) -> Option<Vec<u8>> {
    let output = crate::backends::background_command("ffmpeg")
        .args([
            "-v",
            "error",
            "-i",
            &path.to_string_lossy(),
            "-an",
            "-vcodec",
            "png",
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-",
        ])
        .output()
        .ok()?;
    if output.status.success() && !output.stdout.is_empty() {
        Some(output.stdout)
    } else {
        None
    }
}

struct Inner {
    session: Option<DecodeSession>,
    clock: AudioClock,
    /// Media timestamp (in seconds) at which the current decode session started.
    base: f64,
    speed: f32,
    paused: bool,
    has_audio: bool,
    has_video: bool,
    path: PathBuf,
    out_w: u32,
    out_h: u32,
    /// Wall-clock fallback state for streams without an audio track.
    resume: Instant,
    frozen: f64,
    exited: bool,
    exit_code: i32,
    exit_tail: String,
    /// Monotonic counter incremented on each session spawn to discard events
    /// from superseded decode processes.
    generation: u64,
}

pub struct Engine {
    events_tx: Sender<EngineEvent>,
    events_rx: Arc<Mutex<Receiver<EngineEvent>>>,
    video_tx: SyncSender<(u64, VideoFrame)>,
    video_rx: Arc<Mutex<Receiver<(u64, VideoFrame)>>>,
    audio: Option<AudioOutput>,
    inner: Mutex<Inner>,
}

impl Engine {
    pub fn new() -> Self {
        let (events_tx, events_rx) = channel();
        let (video_tx, video_rx) = std::sync::mpsc::sync_channel(2);
        Engine {
            events_tx,
            events_rx: Arc::new(Mutex::new(events_rx)),
            video_tx,
            video_rx: Arc::new(Mutex::new(video_rx)),
            audio: AudioOutput::open(),
            inner: Mutex::new(Inner {
                session: None,
                clock: AudioClock::default(),
                base: 0.0,
                speed: 1.0,
                paused: false,
                has_audio: false,
                has_video: false,
                path: PathBuf::new(),
                out_w: 0,
                out_h: 0,
                resume: Instant::now(),
                frozen: 0.0,
                exited: false,
                exit_code: 0,
                exit_tail: String::new(),
                generation: 0,
            }),
        }
    }

    pub fn events(&self) -> Arc<Mutex<Receiver<EngineEvent>>> {
        self.events_rx.clone()
    }

    pub fn video(&self) -> Arc<Mutex<Receiver<(u64, VideoFrame)>>> {
        self.video_rx.clone()
    }

    pub fn start(&self, info: &MediaInfo, pos: f64, paused: bool) -> Result<(), String> {
        self.spawn_session(info.clone(), pos, self.speed(), paused)
    }

    fn spawn_session(
        &self,
        info: MediaInfo,
        pos: f64,
        speed: f32,
        paused: bool,
    ) -> Result<(), String> {
        let (out_w, out_h) = info.output_size();
        let decode_audio = info.has_audio && self.audio.is_some();
        let mut inner = self.inner.lock().unwrap();
        inner.generation += 1;
        // Drain queued frames from the previous session to free bounded channel capacity.
        if let Ok(rx) = self.video_rx.try_lock() {
            while rx.try_recv().is_ok() {}
        }
        let generation = inner.generation;
        let session = DecodeSession::spawn(
            &info.path,
            pos,
            speed,
            info.has_video,
            decode_audio,
            out_w,
            out_h,
            self.events_tx.clone(),
            self.video_tx.clone(),
            generation,
        )
        .map_err(|e| format!("could not start ffmpeg: {e}"))?;

        let mut session = session;
        if let Some(player) = self.audio.as_ref() {
            if let Some(rx) = session.audio_rx.take() {
                inner.clock = player.play_stream(rx);
                if !paused {
                    player.player.play();
                }
            } else {
                player.player.stop();
                inner.clock = AudioClock::default();
            }
        }
        if paused {
            session.suspend();
        }

        inner.session = Some(session);
        inner.base = pos;
        inner.speed = speed;
        inner.paused = paused;
        inner.has_audio = decode_audio;
        inner.has_video = info.has_video;
        inner.path = info.path;
        inner.out_w = out_w;
        inner.out_h = out_h;
        inner.resume = Instant::now();
        inner.frozen = 0.0;
        inner.exited = false;
        inner.exit_code = 0;
        inner.exit_tail.clear();
        Ok(())
    }

    pub fn stop(&self) {
        let mut inner = self.inner.lock().unwrap();
        inner.session = None;
        inner.exited = false;
        if let Some(player) = &self.audio {
            player.player.stop();
        }
    }

    pub fn generation(&self) -> u64 {
        self.inner.lock().unwrap().generation
    }

    pub fn set_paused(&self, paused: bool) {
        let mut inner = self.inner.lock().unwrap();
        if inner.paused == paused {
            return;
        }
        inner.paused = paused;
        if let Some(session) = &inner.session {
            if paused {
                if let Some(player) = &self.audio {
                    player.player.pause();
                }
                session.suspend();
            } else {
                session.resume();
                if let Some(player) = &self.audio {
                    player.player.play();
                }
            }
        }
        if !inner.has_audio {
            if paused {
                inner.frozen += inner.resume.elapsed().as_secs_f64();
            } else {
                inner.resume = Instant::now();
            }
        }
    }

    pub fn is_paused(&self) -> bool {
        self.inner.lock().unwrap().paused
    }

    pub fn position(&self) -> f64 {
        let inner = self.inner.lock().unwrap();
        if inner.session.is_none() {
            return inner.base;
        }
        let elapsed = if inner.has_audio {
            let samples = inner.clock.played.load(Ordering::Relaxed) as f64;
            samples / (AUDIO_CHANNELS as f64 * AUDIO_RATE as f64)
        } else {
            let running = if inner.paused {
                0.0
            } else {
                inner.resume.elapsed().as_secs_f64()
            };
            inner.frozen + running
        };
        inner.base + elapsed * inner.speed as f64
    }

    pub fn ended(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        if inner.session.is_none() {
            return false;
        }
        if inner.has_audio {
            inner.clock.drained.load(Ordering::Relaxed)
        } else {
            inner.exited
        }
    }

    pub fn mark_exited(&self, code: i32, tail: String) {
        let mut inner = self.inner.lock().unwrap();
        inner.exited = true;
        inner.exit_code = code;
        inner.exit_tail = tail;
    }

    pub fn failed(&self) -> bool {
        let inner = self.inner.lock().unwrap();
        inner.exited && inner.exit_code != 0
    }

    pub fn exit_tail(&self) -> String {
        self.inner.lock().unwrap().exit_tail.clone()
    }

    pub fn seek(&self, info: &MediaInfo, pos: f64) {
        let (speed, paused) = {
            let inner = self.inner.lock().unwrap();
            (inner.speed, inner.paused)
        };
        let _ = self.spawn_session(info.clone(), pos.max(0.0), speed, paused);
    }

    /// Sets the rate used by the next session without spawning one.
    pub fn prime_speed(&self, speed: f32) {
        self.inner.lock().unwrap().speed = speed;
    }

    pub fn speed(&self) -> f32 {
        self.inner.lock().unwrap().speed
    }

    pub fn set_speed(&self, info: &MediaInfo, speed: f32) {
        let pos = self.position();
        let paused = self.is_paused();
        let _ = self.spawn_session(info.clone(), pos, speed, paused);
    }

    pub fn set_volume(&self, volume: f32) {
        if let Some(player) = &self.audio {
            player.player.set_volume(volume.max(0.0));
        }
    }

    pub fn has_audio_output(&self) -> bool {
        self.audio.is_some()
    }
}

impl Drop for Engine {
    fn drop(&mut self) {
        if let Ok(mut inner) = self.inner.lock() {
            inner.session = None;
        }
    }
}
