use creamui_core::{BoxedWidget, Renderer, Size};
use creamui_render::{Damage, Rasterizer, SceneRecorder};
use creamui_theme::{Color, ThemeProvider};

pub fn enabled() -> bool {
    std::env::var_os("DPLAYER_SNAP").is_some()
}

pub fn snapshot(name: &str, w: u32, h: u32, build: impl FnOnce(Size) -> BoxedWidget) {
    let dir = std::path::PathBuf::from(std::env::var_os("DPLAYER_SNAP").unwrap());
    let theme = crate::app::load_theme();
    creamui_reactive::with_context_scope(|| {
        creamui_reactive::provide_context(ThemeProvider::new(theme.clone()));
        creamui_reactive::provide_context(creamui_reactive::Signal::new(
            creamui_render::WindowDecorations {
                mode: creamui_render::WindowDecorationMode::Server,
                ..Default::default()
            },
        ));
        let size = Size {
            width: w as f32,
            height: h as f32,
        };
        let mut recorder = SceneRecorder::new();
        recorder.begin(w, h, 1.0, Color::rgba(0, 0, 0, 255), theme.colors);
        Renderer::new().render(build(size), size, &mut recorder);
        let mut raster = Rasterizer::new(w, h);
        raster.render(&recorder.finish(), &Damage::Full);
        raster.pixmap().save_png(dir.join(format!("{name}.png"))).unwrap();
    });
}
