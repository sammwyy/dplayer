//! Subprocess decoder running `ffmpeg` with two raw output streams: RGBA8 video
//! frames and interleaved `f32le` stereo PCM, paced at the input clock via `-readrate`.

use crate::backends::{self, ProcessControl, SpawnedDecoder};
use std::io::Read;
use std::sync::mpsc::{sync_channel, Receiver, Sender, SyncSender};
use std::thread;

pub const AUDIO_RATE: u32 = 48_000;
pub const AUDIO_CHANNELS: u32 = 2;
const AUDIO_CHUNK: usize = 16 * 1024;
const MAX_WIDTH: u32 = 1600;

pub struct VideoFrame {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

pub enum EngineEvent {
    /// Carries `(generation, exit_code, stderr_tail)`.
    Exited(u64, i32, String),
}

pub struct DecodeSession {
    control: ProcessControl,
    pub audio_rx: Option<Receiver<Vec<u8>>>,
    _waiter: thread::JoinHandle<()>,
}

fn ffmpeg_args(
    path: &std::path::Path,
    pos: f64,
    speed: f32,
    video_dest: Option<&str>,
    audio_dest: Option<&str>,
) -> Vec<String> {
    let mut args: Vec<String> = vec![
        "-hide_banner".into(),
        "-nostdin".into(),
        "-v".into(),
        "error".into(),
    ];
    if pos > 0.05 {
        args.extend(["-ss".into(), format!("{pos:.3}")]);
    }
    // Pace demuxing at `speed` times realtime; combined with `atempo` on the
    // audio stream, this scales playback rate while keeping A/V synchronized.
    args.extend(["-readrate".into(), format!("{speed:.4}")]);
    args.extend(["-i".into(), path.to_string_lossy().into_owned()]);
    if let Some(video_dest) = video_dest {
        args.extend([
            "-map".into(),
            "0:v:0".into(),
            "-an".into(),
            "-sn".into(),
            "-dn".into(),
            "-vf".into(),
            format!("scale='min({MAX_WIDTH},iw)':-2,setsar=1,fps=30"),
            "-f".into(),
            "rawvideo".into(),
            "-pix_fmt".into(),
            "rgba".into(),
            video_dest.into(),
        ]);
    }
    if let Some(audio_dest) = audio_dest {
        args.extend([
            "-map".into(),
            "0:a:0".into(),
            "-vn".into(),
            "-sn".into(),
            "-dn".into(),
        ]);
        if (speed - 1.0).abs() > 0.001 {
            args.extend(["-af".into(), format!("atempo={speed:.4}")]);
        }
        args.extend([
            "-f".into(),
            "f32le".into(),
            "-ac".into(),
            AUDIO_CHANNELS.to_string(),
            "-ar".into(),
            AUDIO_RATE.to_string(),
            audio_dest.into(),
        ]);
    }
    args
}

impl DecodeSession {
    /// `out_w` and `out_h` must match the scaled dimensions from
    /// [`crate::engine::probe::MediaInfo::output_size`] so raw frame reads align
    /// with frame boundaries.
    #[allow(clippy::too_many_arguments)]
    pub fn spawn(
        path: &std::path::Path,
        pos: f64,
        speed: f32,
        has_video: bool,
        has_audio: bool,
        out_w: u32,
        out_h: u32,
        events_tx: Sender<EngineEvent>,
        video_tx: SyncSender<(u64, VideoFrame)>,
        generation: u64,
    ) -> std::io::Result<Self> {
        let SpawnedDecoder {
            mut child,
            control,
            video_reader,
            audio_reader,
        } = backends::spawn_ffmpeg(has_video, has_audio, |video_dest, audio_dest| {
            ffmpeg_args(
                path,
                pos,
                speed,
                has_video.then_some(video_dest),
                has_audio.then_some(audio_dest),
            )
        })?;

        let stderr = child.stderr.take();
        let waiter = thread::spawn(move || {
            let status = child.wait();
            let mut tail = String::new();
            if let Some(mut err) = stderr {
                let mut buf = String::new();
                let _ = err.read_to_string(&mut buf);
                tail = buf.lines().rev().take(4).collect::<Vec<_>>().join("\n");
            }
            let code = status.map(|s| s.code().unwrap_or(-1)).unwrap_or(-1);
            let _ = events_tx.send(EngineEvent::Exited(generation, code, tail));
        });

        if let Some(mut pipe) = video_reader {
            let frame_len = (out_w * out_h * 4) as usize;
            thread::spawn(move || {
                let mut pixels = vec![0u8; frame_len.max(1)];
                loop {
                    match pipe.read_exact(&mut pixels) {
                        Ok(()) => {
                            let frame = VideoFrame {
                                width: out_w,
                                height: out_h,
                                pixels: pixels.clone(),
                            };
                            // Drop frames if the UI receiver is full so video
                            // decoding does not block on rendering.
                            let _ = video_tx.try_send((generation, frame));
                        }
                        Err(_) => return,
                    }
                }
            });
        }

        let audio_rx = if let Some(mut pipe) = audio_reader {
            let (tx, rx) = sync_channel::<Vec<u8>>(32);
            thread::spawn(move || {
                let mut buf = vec![0u8; AUDIO_CHUNK];
                loop {
                    match pipe.read(&mut buf) {
                        Ok(0) | Err(_) => return,
                        Ok(n) => {
                            if tx.send(buf[..n].to_vec()).is_err() {
                                return;
                            }
                        }
                    }
                }
            });
            Some(rx)
        } else {
            None
        };

        Ok(DecodeSession {
            control,
            audio_rx,
            _waiter: waiter,
        })
    }

    pub fn suspend(&self) {
        self.control.suspend();
    }

    pub fn resume(&self) {
        self.control.resume();
    }
}

impl Drop for DecodeSession {
    fn drop(&mut self) {
        self.control.kill();
    }
}
