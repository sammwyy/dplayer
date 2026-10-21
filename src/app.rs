use crate::config::Config;
use crate::state::PlayerState;
use creamui_render::platform::Key;
use creamui_render::{AppBuilder, AppHandle, BlurRegion, WindowHandle, WindowOptions};
use creamui_theme::{Color, Theme};
use creamui_theme_watch::ThemeListener;
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};

pub fn run(paths: Vec<PathBuf>) {
    crate::backends::init_env();
    let options = WindowOptions {
        title: "Media Player".into(),
        width: 960,
        height: 640,
        transparent: true,
        decorations: true,
        blur: Some(BlurRegion::Window),
        theme: load_theme(),
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

/// Opens the player preferences in their own window (`dplayer --settings`).
pub fn show_settings() {
    crate::backends::init_env();
    let settings = crate::settings::State::load();
    let window = settings.window.clone();
    let options = WindowOptions {
        title: "Media Player Settings".into(),
        width: 780,
        height: 600,
        transparent: true,
        decorations: true,
        blur: Some(BlurRegion::Window),
        theme: load_theme(),
        ..Default::default()
    };
    let theme_events = Arc::new(Mutex::new(ThemeListener::start()));
    AppBuilder::new()
        .on_started(move |app| {
            let ready_window = window.clone();
            app.append_window(
                options,
                Color::rgba(0, 0, 0, 0),
                move |handle| {
                    crate::views::chrome::request_hybrid(&handle);
                    let closer = handle.clone();
                    handle.on_keyboard_input(move |event, _| {
                        let escape = event.pressed && event.key == Key::Escape;
                        if escape {
                            closer.close();
                        }
                        escape
                    });
                    *ready_window.borrow_mut() = Some(handle);
                },
                move |size| crate::settings::build(&settings, size),
            );
            watch_theme_reloads(app, theme_events, window);
        })
        .run();
}

/// Spawns the settings window as a separate instance of this binary.
pub fn open_settings() {
    match std::env::current_exe()
        .and_then(|exe| std::process::Command::new(exe).arg("--settings").spawn())
    {
        Ok(mut child) => {
            std::thread::spawn(move || {
                let _ = child.wait();
            });
        }
        Err(error) => eprintln!("dplayer: couldn't open settings: {error}"),
    }
}

/// The system theme with the user's accent preference applied.
pub(crate) fn load_theme() -> Theme {
    Config::load().appearance.accent.apply(load_system_theme())
}

pub(crate) fn load_system_theme() -> Theme {
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
                    window.set_theme(load_theme());
                }
                watch_theme_reloads(next_app, next_listener, next_window);
            }
        },
    );
}
