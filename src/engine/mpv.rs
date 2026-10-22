use super::decode::{EngineEvent, VideoFrame};
use libloading::Library;
use std::ffi::{c_char, c_int, c_void, CStr, CString};
use std::path::Path;
use std::ptr;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{channel, sync_channel, Receiver, Sender, SyncSender};
use std::sync::{Arc, Mutex, OnceLock};
use std::thread;
use std::time::Duration;

#[repr(C)]
struct RenderParam {
    kind: c_int,
    data: *mut c_void,
}

#[repr(C)]
struct Event {
    kind: c_int,
    error: c_int,
    userdata: u64,
    data: *mut c_void,
}

#[repr(C)]
struct EndFile {
    reason: c_int,
    error: c_int,
}

type Create = unsafe extern "C" fn() -> *mut c_void;
type Destroy = unsafe extern "C" fn(*mut c_void);
type SetOption = unsafe extern "C" fn(*mut c_void, *const c_char, *const c_char) -> c_int;
type Initialize = unsafe extern "C" fn(*mut c_void) -> c_int;
type SetProperty = unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int;
type GetProperty = unsafe extern "C" fn(*mut c_void, *const c_char, c_int, *mut c_void) -> c_int;
type Command = unsafe extern "C" fn(*mut c_void, u64, *const *const c_char) -> c_int;
type WaitEvent = unsafe extern "C" fn(*mut c_void, f64) -> *const Event;
type ErrorString = unsafe extern "C" fn(c_int) -> *const c_char;
type RenderCreate =
    unsafe extern "C" fn(*mut *mut c_void, *mut c_void, *const RenderParam) -> c_int;
type RenderFree = unsafe extern "C" fn(*mut c_void);
type RenderUpdate = unsafe extern "C" fn(*mut c_void) -> u64;
type Render = unsafe extern "C" fn(*mut c_void, *const RenderParam) -> c_int;

struct Api {
    _library: Library,
    create: Create,
    destroy: Destroy,
    set_option: SetOption,
    initialize: Initialize,
    set_property: SetProperty,
    get_property: GetProperty,
    command: Command,
    wait_event: WaitEvent,
    error_string: ErrorString,
    render_create: RenderCreate,
    render_free: RenderFree,
    render_update: RenderUpdate,
    render: Render,
}

impl Api {
    fn load() -> Option<Self> {
        #[cfg(target_os = "windows")]
        let paths = {
            let dir = std::env::current_exe().ok()?.parent()?.to_path_buf();
            vec![dir.join("mpv-2.dll"), dir.join("libmpv-2.dll")]
        };
        #[cfg(target_os = "linux")]
        let paths: Vec<std::path::PathBuf> = vec!["libmpv.so.2".into(), "libmpv.so.1".into()];
        #[cfg(not(any(target_os = "linux", target_os = "windows")))]
        let paths: Vec<std::path::PathBuf> = vec!["libmpv.2.dylib".into(), "libmpv.dylib".into()];
        #[cfg(windows)]
        let library = paths.into_iter().find_map(|path| unsafe {
            use libloading::os::windows::{
                Library as WindowsLibrary, LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR,
            };
            WindowsLibrary::load_with_flags(
                path,
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_DEFAULT_DIRS,
            )
            .ok()
            .map(Library::from)
        })?;
        #[cfg(not(windows))]
        let library = paths
            .into_iter()
            .find_map(|path| unsafe { Library::new(path).ok() })?;
        unsafe {
            macro_rules! symbol {
                ($name:literal) => {
                    *library.get(concat!($name, "\0").as_bytes()).ok()?
                };
            }
            Some(Self {
                create: symbol!("mpv_create"),
                destroy: symbol!("mpv_destroy"),
                set_option: symbol!("mpv_set_option_string"),
                initialize: symbol!("mpv_initialize"),
                set_property: symbol!("mpv_set_property"),
                get_property: symbol!("mpv_get_property"),
                command: symbol!("mpv_command_async"),
                wait_event: symbol!("mpv_wait_event"),
                error_string: symbol!("mpv_error_string"),
                render_create: symbol!("mpv_render_context_create"),
                render_free: symbol!("mpv_render_context_free"),
                render_update: symbol!("mpv_render_context_update"),
                render: symbol!("mpv_render_context_render"),
                _library: library,
            })
        }
    }

    fn error(&self, code: c_int) -> String {
        unsafe { CStr::from_ptr((self.error_string)(code)) }
            .to_string_lossy()
            .into_owned()
    }
}

fn shared_api() -> Option<Arc<Api>> {
    static API: OnceLock<Mutex<Option<Arc<Api>>>> = OnceLock::new();
    let mut api = API.get_or_init(|| Mutex::new(None)).lock().ok()?;
    if api.is_none() {
        *api = Api::load().map(Arc::new);
    }
    api.clone()
}

pub fn available() -> bool {
    shared_api()
        .and_then(|api| Worker::new(api, c"".to_owned(), (16, 16)).ok())
        .is_some()
}

enum Control {
    Pause(bool),
    Seek(f64),
    Speed(f32),
    Volume(f32),
}

pub(super) struct MpvPlayback {
    pub pos: f64,
    pub speed: f32,
    pub paused: bool,
    pub volume: f32,
}

pub struct MpvSession {
    controls: Sender<Control>,
    position: Arc<AtomicU64>,
}

impl MpvSession {
    pub fn open(
        path: &Path,
        dimensions: (u32, u32),
        playback: MpvPlayback,
        events: Sender<EngineEvent>,
        video: SyncSender<(u64, VideoFrame)>,
        generation: u64,
    ) -> Option<Self> {
        let api = shared_api()?;
        #[cfg(unix)]
        let filename = {
            use std::os::unix::ffi::OsStrExt;
            CString::new(path.as_os_str().as_bytes()).ok()?
        };
        #[cfg(not(unix))]
        let filename = CString::new(path.to_string_lossy().as_bytes()).ok()?;
        let (controls, rx) = channel();
        let (ready_tx, ready_rx) = sync_channel(1);
        let position = Arc::new(AtomicU64::new(playback.pos.to_bits()));
        let clock = position.clone();
        thread::spawn(move || {
            let result = Worker::new(api, filename, dimensions);
            let Ok(mut worker) = result else {
                let _ = ready_tx.send(false);
                return;
            };
            worker.set_double(b"speed\0", playback.speed as f64);
            worker.set_double(b"volume\0", (playback.volume * 100.0) as f64);
            worker.set_flag(b"pause\0", playback.paused);
            let loaded = worker.load();
            let _ = ready_tx.send(loaded);
            if loaded {
                worker.run(rx, clock, events, video, generation, playback.pos);
            }
        });
        ready_rx.recv().ok().filter(|ready| *ready)?;
        Some(Self { controls, position })
    }

    pub fn position(&self) -> f64 {
        f64::from_bits(self.position.load(Ordering::Relaxed))
    }

    pub fn seek(&self, pos: f64) {
        self.position.store(pos.to_bits(), Ordering::Relaxed);
        let _ = self.controls.send(Control::Seek(pos));
    }

    pub fn pause(&self, paused: bool) {
        let _ = self.controls.send(Control::Pause(paused));
    }

    pub fn speed(&self, speed: f32) {
        let _ = self.controls.send(Control::Speed(speed));
    }

    pub fn volume(&self, volume: f32) {
        let _ = self.controls.send(Control::Volume(volume));
    }
}

struct Worker {
    api: Arc<Api>,
    handle: *mut c_void,
    render: *mut c_void,
    filename: CString,
    dimensions: (u32, u32),
}

impl Worker {
    fn new(api: Arc<Api>, filename: CString, dimensions: (u32, u32)) -> Result<Self, String> {
        let handle = unsafe { (api.create)() };
        if handle.is_null() {
            return Err("mpv_create failed".into());
        }
        let mut worker = Self {
            api,
            handle,
            render: ptr::null_mut(),
            filename,
            dimensions,
        };
        let vo = unsafe { (worker.api.set_option)(handle, c"vo".as_ptr(), c"libmpv".as_ptr()) };
        if vo < 0 {
            return Err(worker.api.error(vo));
        }
        let keep_open =
            unsafe { (worker.api.set_option)(handle, c"keep-open".as_ptr(), c"yes".as_ptr()) };
        if keep_open < 0 {
            return Err(worker.api.error(keep_open));
        }
        if dimensions.0 == 0 {
            let _ = unsafe { (worker.api.set_option)(handle, c"vid".as_ptr(), c"no".as_ptr()) };
        }
        let initialized = unsafe { (worker.api.initialize)(handle) };
        if initialized < 0 {
            return Err(worker.api.error(initialized));
        }
        if dimensions.0 > 0 && dimensions.1 > 0 {
            let mut params = [
                RenderParam {
                    kind: 1,
                    data: c"sw".as_ptr().cast_mut().cast(),
                },
                RenderParam {
                    kind: 0,
                    data: ptr::null_mut(),
                },
            ];
            let result = unsafe {
                (worker.api.render_create)(&mut worker.render, handle, params.as_mut_ptr())
            };
            if result < 0 {
                return Err(worker.api.error(result));
            }
        }
        Ok(worker)
    }

    fn load(&self) -> bool {
        let args = [c"loadfile".as_ptr(), self.filename.as_ptr(), ptr::null()];
        unsafe { (self.api.command)(self.handle, 0, args.as_ptr()) >= 0 }
    }

    fn set_flag(&self, name: &[u8], enabled: bool) {
        let mut flag: c_int = enabled.into();
        unsafe {
            (self.api.set_property)(
                self.handle,
                name.as_ptr().cast(),
                3,
                (&mut flag as *mut c_int).cast(),
            );
        }
    }

    fn set_double(&self, name: &[u8], value: f64) {
        let mut value = value;
        unsafe {
            (self.api.set_property)(
                self.handle,
                name.as_ptr().cast(),
                5,
                (&mut value as *mut f64).cast(),
            );
        }
    }

    fn seek(&self, pos: f64) {
        self.set_double(b"time-pos\0", pos);
    }

    fn run(
        &mut self,
        rx: Receiver<Control>,
        position: Arc<AtomicU64>,
        events: Sender<EngineEvent>,
        video: SyncSender<(u64, VideoFrame)>,
        generation: u64,
        start_pos: f64,
    ) {
        let mut ticks = 0u32;
        let mut waiting_for_seek = start_pos > 0.0;
        let mut ended = false;
        loop {
            let first = match rx.recv_timeout(Duration::from_millis(10)) {
                Ok(command) => Some(command),
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => return,
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => None,
            };
            let (mut pause, mut seek, mut speed, mut volume) = (None, None, None, None);
            for command in first.into_iter().chain(rx.try_iter().take(128)) {
                match command {
                    Control::Pause(value) => pause = Some(value),
                    Control::Seek(value) => seek = Some(value),
                    Control::Speed(value) => speed = Some(value),
                    Control::Volume(value) => volume = Some(value),
                }
            }
            if let Some(value) = speed {
                self.set_double(b"speed\0", value as f64);
            }
            if let Some(value) = volume {
                self.set_double(b"volume\0", (value * 100.0) as f64);
            }
            if let Some(value) = pause {
                self.set_flag(b"pause\0", value);
            }
            if let Some(value) = seek {
                self.seek(value);
                waiting_for_seek = true;
                ended = false;
            }
            loop {
                let event = unsafe { (self.api.wait_event)(self.handle, 0.0) };
                if event.is_null() {
                    break;
                }
                let event = unsafe { &*event };
                if event.kind == 0 {
                    break;
                }
                if event.kind == 8 && start_pos > 0.0 {
                    self.seek(start_pos);
                }
                if event.kind == 21 {
                    waiting_for_seek = false;
                }
                if event.kind == 7 && !event.data.is_null() {
                    let end = unsafe { &*(event.data as *const EndFile) };
                    if !ended && (end.reason == 4 || (end.reason == 0 && !waiting_for_seek)) {
                        ended = true;
                        let tail = if end.reason == 4 {
                            self.api.error(end.error)
                        } else {
                            String::new()
                        };
                        let _ = events.send(EngineEvent::Exited(
                            generation,
                            if end.reason == 4 { end.error } else { 0 },
                            tail,
                        ));
                    }
                }
                if event.kind == 1 {
                    return;
                }
            }
            ticks += 1;
            if ticks >= 5 {
                ticks = 0;
                if !waiting_for_seek {
                    let mut time = 0.0f64;
                    let result = unsafe {
                        (self.api.get_property)(
                            self.handle,
                            c"time-pos".as_ptr(),
                            5,
                            (&mut time as *mut f64).cast(),
                        )
                    };
                    if result >= 0 && time.is_finite() {
                        position.store(time.to_bits(), Ordering::Relaxed);
                    }
                }
                if !ended && !waiting_for_seek {
                    let mut eof = 0;
                    let result = unsafe {
                        (self.api.get_property)(
                            self.handle,
                            c"eof-reached".as_ptr(),
                            3,
                            (&mut eof as *mut c_int).cast(),
                        )
                    };
                    if result >= 0 && eof != 0 {
                        ended = true;
                        let _ = events.send(EngineEvent::Exited(generation, 0, String::new()));
                    }
                }
            }
            if !self.render.is_null() && unsafe { (self.api.render_update)(self.render) } & 1 != 0 {
                self.render_frame(&video, generation);
            }
        }
    }

    fn render_frame(&self, tx: &SyncSender<(u64, VideoFrame)>, generation: u64) {
        let (width, height) = self.dimensions;
        let mut pixels = vec![0u8; width as usize * height as usize * 4];
        let mut size = [width as c_int, height as c_int];
        let mut stride = width as usize * 4;
        let params = [
            RenderParam {
                kind: 17,
                data: size.as_mut_ptr().cast(),
            },
            RenderParam {
                kind: 18,
                data: c"rgb0".as_ptr().cast_mut().cast(),
            },
            RenderParam {
                kind: 19,
                data: (&mut stride as *mut usize).cast(),
            },
            RenderParam {
                kind: 20,
                data: pixels.as_mut_ptr().cast(),
            },
            RenderParam {
                kind: 0,
                data: ptr::null_mut(),
            },
        ];
        if unsafe { (self.api.render)(self.render, params.as_ptr()) } >= 0 {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel[3] = 255;
            }
            let _ = tx.try_send((
                generation,
                VideoFrame {
                    width,
                    height,
                    pixels,
                },
            ));
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        unsafe {
            if !self.render.is_null() {
                (self.api.render_free)(self.render);
            }
            (self.api.destroy)(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn live_controls_keep_the_same_session() {
        let (controls, rx) = channel();
        let session = MpvSession {
            controls,
            position: Arc::new(AtomicU64::new(0.0f64.to_bits())),
        };
        session.seek(12.5);
        session.speed(1.5);
        session.pause(true);
        session.volume(0.4);
        assert_eq!(session.position(), 12.5);
        assert!(matches!(rx.recv().unwrap(), Control::Seek(12.5)));
        assert!(matches!(rx.recv().unwrap(), Control::Speed(1.5)));
        assert!(matches!(rx.recv().unwrap(), Control::Pause(true)));
        assert!(matches!(rx.recv().unwrap(), Control::Volume(v) if (v - 0.4).abs() < 0.001));
    }

    #[test]
    fn renders_with_installed_libmpv_when_fixture_is_provided() {
        let Some(path) = std::env::var_os("DPLAYER_MPV_TEST_FILE") else {
            return;
        };
        assert!(available());
        let (events, exits) = channel();
        let (video, frames) = sync_channel(2);
        let session = MpvSession::open(
            Path::new(&path),
            (320, 180),
            MpvPlayback {
                pos: 0.0,
                speed: 1.0,
                paused: false,
                volume: 0.0,
            },
            events,
            video,
            1,
        )
        .expect("libmpv software renderer must initialize");
        let frame = loop {
            let (_, frame) = frames
                .recv_timeout(Duration::from_secs(10))
                .expect("libmpv must produce a video frame");
            if frame
                .pixels
                .chunks_exact(4)
                .any(|pixel| pixel[0] != 0 || pixel[1] != 0 || pixel[2] != 0)
            {
                break frame;
            }
        };
        assert_eq!((frame.width, frame.height), (320, 180));
        assert!(frame.pixels.chunks_exact(4).all(|pixel| pixel[3] == 255));
        session.seek(0.2);
        session.speed(1.5);
        session.pause(true);
        assert_eq!(session.position(), 0.2);
        session.pause(false);
        assert!(matches!(
            exits.recv_timeout(Duration::from_secs(5)),
            Ok(EngineEvent::Exited(1, 0, _))
        ));
        session.seek(0.2);
        session.pause(false);
        thread::sleep(Duration::from_millis(250));
        assert!(session.position() >= 0.2);
        assert!(matches!(
            exits.recv_timeout(Duration::from_secs(5)),
            Ok(EngineEvent::Exited(1, 0, _))
        ));
    }
}
