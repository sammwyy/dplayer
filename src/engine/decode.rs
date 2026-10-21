//! Subprocess decoder running `ffmpeg` with two raw output pipes: RGBA8 video
//! frames on fd 3 and interleaved `f32le` stereo PCM on fd 4, paced at the
//! input clock via `-readrate`.

use std::fs::File;
use std::io::Read;
use std::os::unix::io::FromRawFd;
use std::os::unix::process::CommandExt;
use std::process::{Command, Stdio};
use std::sync::mpsc::{sync_channel, Receiver, Sender, SyncSender};
use std::thread;

const VIDEO_FD: i32 = 3;
const AUDIO_FD: i32 = 4;
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
    pid: u32,
    pub audio_rx: Option<Receiver<Vec<u8>>>,
    _waiter: thread::JoinHandle<()>,
}

fn pipe() -> (File, File) {
    let mut fds = [0i32; 2];
    if unsafe { libc::pipe(fds.as_mut_ptr()) } != 0 {
        panic!("pipe() failed: {}", std::io::Error::last_os_error());
    }
    unsafe { (File::from_raw_fd(fds[0]), File::from_raw_fd(fds[1])) }
}

fn ffmpeg_args(
    path: &std::path::Path,
    pos: f64,
    speed: f32,
    has_video: bool,
    has_audio: bool,
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
    if has_video {
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
            format!("pipe:{VIDEO_FD}"),
        ]);
    }
    if has_audio {
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
            format!("pipe:{AUDIO_FD}"),
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
        let (video_read, video_write) = pipe();
        let (audio_read, audio_write) = pipe();
        let video_w = std::os::unix::io::IntoRawFd::into_raw_fd(video_write);
        let audio_w = std::os::unix::io::IntoRawFd::into_raw_fd(audio_write);

        let mut command = Command::new("ffmpeg");
        command
            .args(ffmpeg_args(path, pos, speed, has_video, has_audio))
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        unsafe {
            command.pre_exec(move || {
                libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
                // Avoid clobbering either pipe write end if its fd already
                // matches the other stream's target descriptor.
                let mut v = video_w;
                let mut a = audio_w;
                if v == AUDIO_FD {
                    v = libc::fcntl(v, libc::F_DUPFD, AUDIO_FD + 1);
                }
                if a == VIDEO_FD {
                    a = libc::fcntl(a, libc::F_DUPFD, AUDIO_FD + 1);
                }
                if has_video {
                    libc::dup2(v, VIDEO_FD);
                }
                if has_audio {
                    libc::dup2(a, AUDIO_FD);
                }
                Ok(())
            });
        }
        let mut child = command.spawn().inspect_err(|_| unsafe {
            libc::close(video_w);
            libc::close(audio_w);
        })?;
        // Close parent copies of the write descriptors so reader threads observe EOF when ffmpeg exits.
        unsafe {
            libc::close(video_w);
            libc::close(audio_w);
        }

        let pid = child.id();
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

        if has_video {
            let mut pipe = video_read;
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

        let audio_rx = if has_audio {
            let (tx, rx) = sync_channel::<Vec<u8>>(32);
            let mut pipe = audio_read;
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
            pid,
            audio_rx,
            _waiter: waiter,
        })
    }

    pub fn suspend(&self) {
        unsafe {
            libc::kill(self.pid as i32, libc::SIGSTOP);
        }
    }
    pub fn resume(&self) {
        unsafe {
            libc::kill(self.pid as i32, libc::SIGCONT);
        }
    }
}

impl Drop for DecodeSession {
    fn drop(&mut self) {
        unsafe {
            libc::kill(self.pid as i32, libc::SIGKILL);
        }
    }
}
