use crate::state::PlayerState;
use creamui_render::{AppBuilder, AppHandle, BlurRegion, WindowHandle, WindowOptions};
use creamui_theme::{Color, Theme};
use creamui_theme_watch::ThemeListener;
use std::cell::RefCell;
use std::fs;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub fn run(paths: Vec<PathBuf>) {
    ensure_wayland_display();
    let options = WindowOptions {
        title: "Media Player".into(),
        width: 960,
        height: 640,
        transparent: true,
        decorations: true,
        blur: Some(BlurRegion::Window),
        theme: load_system_theme(),
        ..Default::default()
    };
    let theme_events = Arc::new(Mutex::new(ThemeListener::start()));
    AppBuilder::new()
        .on_started(move |app| {
            let state = PlayerState::new(app.clone());
            let window = state.window.clone();
            let keys = state.clone();
            let pumps = state.clone();
            state.enqueue(paths.clone());
            app.append_window(
                options.clone(),
                Color::rgba(0, 0, 0, 0),
                move |handle| {
                    crate::views::chrome::request_hybrid(&handle);
                    let activity = keys.clone();
                    handle.on_pointer_moved(move |p| activity.poke_pointer((p.x, p.y)));
                    let presence = keys.clone();
                    handle.on_pointer_presence(move |inside| {
                        if inside {
                            presence.poke();
                        }
                    });
                    let dropped = keys.clone();
                    handle.on_files_dropped(move |paths| dropped.drop_files(paths));
                    handle.on_keyboard_input(move |event, modifiers| {
                        let pressed = keys.key(event, modifiers);
                        if pressed {
                            keys.poke();
                        }
                        pressed
                    });
                    *window.borrow_mut() = Some(handle);
                },
                move |size| crate::views::build(&state, size),
            );
            pumps.start_pump();
            pumps.start_video_pump();
            pumps.start_ticker();
            watch_theme_reloads(app.clone(), theme_events.clone(), pumps.window.clone());
        })
        .run();
}

fn load_system_theme() -> Theme {
    match creamui_theme_loader::SystemThemeLoader::new().load() {
        Ok(appearance) => {
            if let Some(font_family) = &appearance.font_family {
                creamui_fonts::use_system_font(font_family);
            }
            appearance.theme
        }
        Err(error) => {
            eprintln!("dplayer: failed to load CreamUI system theme: {error}");
            Theme::light()
        }
    }
}

fn watch_theme_reloads(
    app: AppHandle,
    listener: Arc<Mutex<ThemeListener>>,
    window: Rc<RefCell<Option<WindowHandle>>>,
) {
    let next_app = app.clone();
    let next_listener = listener.clone();
    let next_window = window.clone();
    app.spawn_background(
        move || listener.lock().is_ok_and(|listener| listener.recv()),
        move |changed| {
            if changed {
                if let Some(window) = window.borrow().as_ref() {
                    window.set_theme(load_system_theme());
                }
                watch_theme_reloads(next_app, next_listener, next_window);
            }
        },
    );
}

fn ensure_wayland_display() {
    if std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return;
    }
    let Some(runtime) = std::env::var_os("XDG_RUNTIME_DIR") else {
        return;
    };
    use std::os::unix::fs::FileTypeExt;
    let mut sockets = fs::read_dir(runtime)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.file_type().is_ok_and(|t| t.is_socket()))
        .filter_map(|e| e.file_name().into_string().ok())
        .filter(|n| n.starts_with("wayland-"))
        .collect::<Vec<_>>();
    sockets.sort();
    if let Some(socket) = sockets.first() {
        std::env::set_var("WAYLAND_DISPLAY", socket);
    }
}
