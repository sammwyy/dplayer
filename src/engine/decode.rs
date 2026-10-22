//! Subprocess decoder running `ffmpeg` with two raw output streams: RGBA8 video
//! frames and interleaved `f32le` stereo PCM, paced at the input clock via `-readrate`.

use crate::backends::{self, ProcessControl, SpawnedDecoder};
use std::io::Read;
use std::sync::mpsc::{sync_channel, Receiver, Sender, SyncSender};
use std::thread;

pub const AUDIO_RATE: u32 = 48_000;
pub const AUDIO_CHANNELS: u32 = 2;
const AUDIO_CHUNK: usize = 16 * 1024;

fn forward_audio(mut pipe: impl Read, tx: SyncSender<Vec<u8>>) {
    let mut buf = vec![0u8; AUDIO_CHUNK];
    let mut leftover = 0;
    loop {
        match pipe.read(&mut buf[leftover..]) {
            Ok(0) | Err(_) => return,
            Ok(n) => {
                let total = leftover + n;
                let aligned = total & !3;
                if aligned > 0 && tx.send(buf[..aligned].to_vec()).is_err() {
                    return;
                }
                leftover = total - aligned;
                buf.copy_within(aligned..total, 0);
            }
        }
    }
}

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
    video_stream: Option<usize>,
    audio_stream: Option<usize>,
    out_w: u32,
    out_h: u32,
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
    args.extend([
        "-readrate".into(),
        format!("{speed:.4}"),
        "-readrate_initial_burst".into(),
        "0.25".into(),
    ]);
    args.extend(["-i".into(), path.to_string_lossy().into_owned()]);
    if let (Some(index), Some(video_dest)) = (video_stream, video_dest) {
        args.extend([
            "-map".into(),
            format!("0:{index}"),
            "-an".into(),
            "-sn".into(),
            "-dn".into(),
            "-vf".into(),
            format!(
                "fps={:.4},scale={out_w}:{out_h},setsar=1",
                30.0 / speed.max(1.0)
            ),
            "-f".into(),
            "rawvideo".into(),
            "-pix_fmt".into(),
            "rgba".into(),
            video_dest.into(),
        ]);
    }
    if let (Some(index), Some(audio_dest)) = (audio_stream, audio_dest) {
        args.extend([
            "-map".into(),
            format!("0:{index}"),
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
        video_stream: Option<usize>,
        audio_stream: Option<usize>,
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
        } = backends::spawn_ffmpeg(
            video_stream.is_some(),
            audio_stream.is_some(),
            |video_dest, audio_dest| {
                ffmpeg_args(
                    path,
                    pos,
                    speed,
                    video_stream,
                    audio_stream,
                    out_w,
                    out_h,
                    video_stream.map(|_| video_dest),
                    audio_stream.map(|_| audio_dest),
                )
            },
        )?;

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
            let frame_len = (out_w as usize * out_h as usize * 4).max(1);
            thread::spawn(move || {
                let mut pixels = vec![0u8; frame_len];
                loop {
                    match pipe.read_exact(&mut pixels) {
                        Ok(()) => {
                            let frame = VideoFrame {
                                width: out_w,
                                height: out_h,
                                pixels,
                            };
                            // Drop frames if the UI receiver is full so video
                            // decoding does not block on rendering.
                            match video_tx.try_send((generation, frame)) {
                                Ok(()) => pixels = vec![0u8; frame_len],
                                Err(std::sync::mpsc::TrySendError::Full((_, frame))) => {
                                    pixels = frame.pixels;
                                }
                                Err(std::sync::mpsc::TrySendError::Disconnected(_)) => return,
                            }
                        }
                        Err(_) => return,
                    }
                }
            });
        }

        let audio_rx = if let Some(pipe) = audio_reader {
            let (tx, rx) = sync_channel::<Vec<u8>>(8);
            thread::spawn(move || forward_audio(pipe, tx));
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

#[cfg(test)]
mod tests {
    use super::*;

    struct ShortReads<'a>(&'a [u8]);
    impl Read for ShortReads<'_> {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            let n = buf.len().min(self.0.len()).min(3);
            buf[..n].copy_from_slice(&self.0[..n]);
            self.0 = &self.0[n..];
            Ok(n)
        }
    }

    #[test]
    fn pcm_chunks_preserve_samples_across_short_reads() {
        let samples = [1.0f32, -0.25, 0.5];
        let bytes: Vec<_> = samples
            .iter()
            .flat_map(|sample| sample.to_le_bytes())
            .collect();
        let (tx, rx) = sync_channel(4);
        forward_audio(ShortReads(&bytes), tx);
        let chunks: Vec<_> = rx.into_iter().collect();
        assert!(chunks.iter().all(|chunk| chunk.len() % 4 == 0));
        assert_eq!(chunks.concat(), bytes);
    }

    #[test]
    fn maps_actual_stream_indices_not_first_video_or_audio() {
        let args = ffmpeg_args(
            std::path::Path::new("video.mkv"),
            0.0,
            1.0,
            Some(3),
            Some(2),
            1280,
            720,
            Some("pipe:3"),
            Some("pipe:4"),
        );
        let maps: Vec<_> = args
            .windows(2)
            .filter(|pair| pair[0] == "-map")
            .map(|pair| pair[1].as_str())
            .collect();
        assert_eq!(maps, ["0:3", "0:2"]);
        let fast = ffmpeg_args(
            std::path::Path::new("video.mkv"),
            10.0,
            2.0,
            Some(3),
            Some(2),
            1280,
            720,
            Some("pipe:3"),
            Some("pipe:4"),
        );
        assert!(fast
            .windows(2)
            .any(|pair| pair[0] == "-vf" && pair[1].starts_with("fps=15.0000,scale=")));
    }
}
