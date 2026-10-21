use crate::engine::{self, Engine, EngineEvent, MediaInfo};
use crate::icons::Icons;
use creamui_image::ImageData;
use creamui_reactive::Signal;
use creamui_render::{AppHandle, WindowHandle};
use creamui_widgets::ScrollController;
use std::cell::{Cell, RefCell};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Repeat {
    Off,
    All,
    One,
}

#[derive(Clone, PartialEq)]
pub enum Status {
    Empty,
    Loading,
    Playing,
    Error(String),
}

#[derive(Clone)]
pub struct QueueItem {
    pub path: PathBuf,
    pub name: String,
}

pub const SPEEDS: &[f32] = &[0.5, 0.75, 1.0, 1.25, 1.5, 2.0];

#[derive(Clone)]
pub struct PlayerState {
    pub engine: Rc<Engine>,
    pub app: AppHandle,
    pub icons: Icons,
    pub window: Rc<RefCell<Option<WindowHandle>>>,
    pub maximized: Signal<bool>,

    pub queue: Signal<Vec<QueueItem>>,
    pub current: Signal<Option<usize>>,
    pub media: Signal<Option<MediaInfo>>,
    pub status: Signal<Status>,
    pub playing: Signal<bool>,
    pub position: Signal<f64>,
    pub duration: Signal<f64>,
    /// Preview timestamp while the user is dragging the seek bar.
    pub scrub: Signal<Option<f64>>,
    pub volume: Signal<f32>,
    pub muted: Signal<bool>,
    pub speed: Signal<f32>,
    pub repeat: Signal<Repeat>,
    pub shuffle: Signal<bool>,
    pub queue_open: Signal<bool>,
    pub controls_visible: Signal<bool>,
    pub frame: Signal<Option<ImageData>>,
    pub cover: Signal<Option<ImageData>>,
    pub queue_scroll: ScrollController,

    last_move: Rc<Cell<Instant>>,
    last_pos: Rc<Cell<(f32, f32)>>,
    /// Monotonic token used to discard async `ffprobe` results from superseded track loads.
    pub load_token: Rc<AtomicU64>,
    pub rng: Rc<Cell<u64>>,
}

impl PlayerState {
    pub fn new(app: AppHandle) -> Self {
        PlayerState {
            engine: Rc::new(Engine::new()),
            app,
            icons: Icons::load(),
            window: Rc::new(RefCell::new(None)),
            maximized: Signal::new(false),
            queue: Signal::new(Vec::new()),
            current: Signal::new(None),
            media: Signal::new(None),
            status: Signal::new(Status::Empty),
            playing: Signal::new(false),
            position: Signal::new(0.0),
            duration: Signal::new(0.0),
            scrub: Signal::new(None),
            volume: Signal::new(1.0),
            muted: Signal::new(false),
            speed: Signal::new(1.0),
            repeat: Signal::new(Repeat::Off),
            shuffle: Signal::new(false),
            queue_open: Signal::new(false),
            frame: Signal::new(None),
            cover: Signal::new(None),
            queue_scroll: ScrollController::new(0.0),
            controls_visible: Signal::new(true),
            last_move: Rc::new(Cell::new(Instant::now())),
            last_pos: Rc::new(Cell::new((0.0, 0.0))),
            load_token: Rc::new(AtomicU64::new(0)),
            rng: Rc::new(Cell::new(
                std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map(|d| d.subsec_nanos() as u64)
                    .unwrap_or(0x9e3779b9),
            )),
        }
    }

    pub fn poke(&self) {
        self.last_move.set(Instant::now());
        if !self.controls_visible.get() {
            self.controls_visible.set(true);
        }
    }

    /// Ignores pointer deltas below 2 px because some Wayland compositors
    /// re-emit pointer motion events on every surface commit.
    pub fn poke_pointer(&self, pos: (f32, f32)) {
        let last = self.last_pos.get();
        if (pos.0 - last.0).abs() < 2.0 && (pos.1 - last.1).abs() < 2.0 {
            return;
        }
        self.last_pos.set(pos);
        self.last_move.set(Instant::now());
        if !self.controls_visible.get() {
            self.controls_visible.set(true);
        }
    }

    fn chrome_visible(&self) -> bool {
        match self.media.get() {
            Some(media) if media.has_video => {}
            _ => return true,
        }
        if !self.playing.get()
            || self.scrub.peek().is_some()
            || self.queue_open.get()
        {
            return true;
        }
        self.last_move.get().elapsed() < Duration::from_secs(5)
    }

    fn next_load_token(&self) -> u64 {
        self.load_token.fetch_add(1, Ordering::Relaxed) + 1
    }

    pub fn enqueue(&self, paths: Vec<PathBuf>) {
        if paths.is_empty() {
            return;
        }
        let mut queue = self.queue.get();
        let start = queue.len();
        for path in paths {
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("Unknown")
                .to_string();
            queue.push(QueueItem { path, name });
        }
        self.queue.set(queue);
        if self.current.get().is_none() {
            self.play_index(start);
        }
    }

    pub fn drop_files(&self, paths: Vec<PathBuf>) {
        let playable = crate::cli::collect_media(paths);
        if playable.is_empty() {
            self.status.set(Status::Error(
                "no playable files in the drop".into(),
            ));
            return;
        }
        self.enqueue(playable);
        self.poke();
    }

    pub fn play_index(&self, index: usize) {
        let path = match self.queue.get().get(index) {
            Some(item) => item.path.clone(),
            None => return,
        };
        self.stop_session();
        self.current.set(Some(index));
        self.status.set(Status::Loading);
        let token = self.next_load_token();
        let state = self.clone();
        self.app.spawn_background(
            move || {
                engine::probe::probe(&path).map(|info| {
                    let cover = if info.attached_picture {
                        engine::cover_art(&path)
                    } else {
                        None
                    };
                    (info, cover)
                })
            },
            move |result| {
                if state.load_token.load(Ordering::Relaxed) != token {
                    return;
                }
                match result {
                    Ok((info, cover)) => state.start_media(info, cover),
                    Err(error) => {
                        state.status.set(Status::Error(error));
                        state.playing.set(false);
                    }
                }
            },
        );
    }

    fn start_media(&self, info: MediaInfo, cover: Option<Vec<u8>>) {
        self.cover.set(
            cover.and_then(|bytes| ImageData::from_bytes(&bytes).ok()),
        );
        match self.engine.start(&info, 0.0, false) {
            Ok(()) => {
                self.duration.set(info.duration);
                self.position.set(0.0);
                self.media.set(Some(info));
                self.status.set(Status::Playing);
                self.playing.set(true);
            }
            Err(error) => {
                self.media.set(None);
                self.status.set(Status::Error(error));
                self.playing.set(false);
            }
        }
    }

    fn stop_session(&self) {
        self.next_load_token();
        self.engine.stop();
        self.frame.set(None);
        self.cover.set(None);
        self.position.set(0.0);
        self.duration.set(0.0);
        self.scrub.set(None);
    }

    pub fn toggle(&self) {
        self.set_playing(!self.playing.get());
    }

    pub fn set_playing(&self, playing: bool) {
        if self.media.get().is_none() {
            return;
        }
        if playing && self.engine.ended() {
            self.seek_to(0.0);
        }
        self.playing.set(playing);
        self.engine.set_paused(!playing);
    }

    pub fn shown_position(&self) -> f64 {
        if let Some(scrub) = self.scrub.get() {
            scrub
        } else {
            self.position.get()
        }
    }

    pub fn seek_to(&self, pos: f64) {
        let Some(media) = self.media.get() else { return };
        let pos = pos.clamp(0.0, self.duration.get().max(pos));
        self.position.set(pos);
        self.engine.seek(&media, pos);
        self.sync_pause_flag();
    }

    pub fn seek_by(&self, delta: f64) {
        let pos = (self.engine.position() + delta).max(0.0);
        self.seek_to(pos.min(self.duration.get().max(pos)));
    }

    pub fn begin_scrub(&self, fraction: f64) {
        self.scrub.set(Some(fraction * self.duration.get()));
    }

    pub fn update_scrub(&self, fraction: f64) {
        if self.scrub.peek().is_some() {
            self.scrub.set(Some(fraction * self.duration.get()));
        }
    }

    pub fn end_scrub(&self) {
        if let Some(pos) = self.scrub.get() {
            self.scrub.set(None);
            self.seek_to(pos);
        }
    }

    pub fn set_volume(&self, volume: f32) {
        let volume = volume.clamp(0.0, 1.0);
        self.volume.set(volume);
        if !self.muted.peek() {
            self.engine.set_volume(volume);
        }
    }

    pub fn toggle_mute(&self) {
        let muted = !self.muted.get();
        self.muted.set(muted);
        self.engine.set_volume(if muted { 0.0 } else { self.volume.get() });
    }

    pub fn cycle_speed(&self, forward: bool) {
        let speeds = SPEEDS;
        let current = self.speed.get();
        let index = speeds
            .iter()
            .position(|s| (*s - current).abs() < 0.001)
            .unwrap_or(2);
        let next = if forward {
            speeds.get(index + 1).copied().unwrap_or(speeds[0])
        } else {
            index
                .checked_sub(1)
                .map(|i| speeds[i])
                .unwrap_or(*speeds.last().unwrap())
        };
        self.speed.set(next);
        if let Some(media) = self.media.get() {
            self.engine.set_speed(&media, next);
            self.sync_pause_flag();
        }
    }

    pub fn cycle_repeat(&self) {
        let next = match self.repeat.get() {
            Repeat::Off => Repeat::All,
            Repeat::All => Repeat::One,
            Repeat::One => Repeat::Off,
        };
        self.repeat.set(next);
    }

    pub fn toggle_shuffle(&self) {
        self.shuffle.set(!self.shuffle.get());
    }

    pub fn advance(&self, forward: bool) -> bool {
        let queue = self.queue.get();
        let Some(index) = self.current.get() else { return false };
        let next = if self.shuffle.get() && queue.len() > 1 {
            let mut s = self.rng.get();
            s ^= s << 13;
            s ^= s >> 7;
            s ^= s << 17;
            self.rng.set(s);
            let step = 1 + (s as usize % (queue.len() - 1));
            (index + step) % queue.len()
        } else if forward {
            index + 1
        } else if index == 0 {
            return false;
        } else {
            index - 1
        };
        if next >= queue.len() {
            return false;
        }
        self.play_index(next);
        true
    }

    pub fn prev(&self) {
        if self.engine.position() > 3.0 || !self.advance(false) {
            self.seek_to(0.0);
        }
    }

    fn on_track_end(&self) {
        match self.repeat.get() {
            Repeat::One => self.seek_to(0.0),
            Repeat::All => {
                if !self.advance(true) {
                    self.play_index(0);
                }
            }
            Repeat::Off => {
                if !self.advance(true) {
                    self.playing.set(false);
                    self.engine.set_paused(true);
                }
            }
        }
    }

    pub fn remove_at(&self, index: usize) {
        let mut queue = self.queue.get();
        if index >= queue.len() {
            return;
        }
        queue.remove(index);
        self.queue.set(queue.clone());
        match self.current.get() {
            Some(current) if current == index => {
                if queue.is_empty() {
                    self.current.set(None);
                    self.stop_session();
                    self.media.set(None);
                    self.status.set(Status::Empty);
                    self.playing.set(false);
                } else {
                    self.play_index(index.min(queue.len() - 1));
                }
            }
            Some(current) if current > index => self.current.set(Some(current - 1)),
            _ => {}
        }
    }

    pub fn start_pump(&self) {
        let rx = self.engine.events();
        let state = self.clone();
        self.app.spawn_background(
            move || rx.lock().unwrap().recv().ok(),
            move |event| {
                match event {
                    Some(EngineEvent::Exited(gen, code, tail))
                        if gen == state.engine.generation() =>
                    {
                        state.engine.mark_exited(code, tail);
                    }
                    Some(_) => {}
                    None => return,
                }
                state.start_pump();
            },
        );
    }

    pub fn start_video_pump(&self) {
        let rx = self.engine.video();
        let state = self.clone();
        self.app.spawn_background(
            move || rx.lock().unwrap().recv().ok(),
            move |frame| {
                if let Some((gen, f)) = frame {
                    if gen == state.engine.generation() {
                        if let Ok(image) =
                            ImageData::from_premultiplied_rgba(f.width, f.height, f.pixels)
                        {
                            state.frame.set(Some(image));
                        }
                    }
                    state.start_video_pump();
                }
            },
        );
    }

    pub fn start_ticker(&self) {
        let state = self.clone();
        self.app.spawn_background(
            || {
                std::thread::sleep(Duration::from_millis(100));
            },
            move |_| {
                state.tick();
                state.start_ticker();
            },
        );
    }

    fn tick(&self) {
        self.controls_visible.set_if_changed(self.chrome_visible());
        if self.status.get() != Status::Playing {
            return;
        }
        if self.scrub.peek().is_none() {
            let pos = self.engine.position().min(self.duration.get().max(0.0));
            self.position.set_if_changed(pos);
        }
        if self.engine.ended() {
            self.on_track_end();
        } else if self.engine.failed() {
            let tail = self.engine.exit_tail();
            self.status.set(Status::Error(if tail.is_empty() {
                "playback failed".into()
            } else {
                tail
            }));
            self.playing.set(false);
        }
    }

    /// Re-applies the pause state after operations (`seek`, `set_speed`) that
    /// respawn the decoder subprocess.
    fn sync_pause_flag(&self) {
        self.engine.set_paused(!self.playing.peek());
    }

    pub fn toggle_maximize(&self) {
        if let Some(handle) = self.window.borrow().as_ref() {
            let next = !self.maximized.peek();
            handle.set_maximized(next);
            self.maximized.set(next);
        }
    }

    pub fn open_dialog(&self) {
        let state = self.clone();
        self.app.spawn_background(
            || {
                rfd::FileDialog::new()
                    .set_title("Open media")
                    .add_filter("Media", crate::MEDIA_EXTENSIONS)
                    .add_filter("All files", &["*"])
                    .pick_files()
            },
            move |picked| {
                if let Some(paths) = picked {
                    state.enqueue(paths);
                }
            },
        );
    }
}
