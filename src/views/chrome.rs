use crate::components::*;
use crate::state::{PlayerState, Status};
use creamui_widgets::AppTitlebar;

pub const HEIGHT: f32 = 40.0;

pub fn build(p: &PlayerState, reveal: f32, width: f32) -> BoxedWidget {
    let media = p.media.get();
    let video = media.as_ref().is_some_and(|m| m.has_video) && p.status.get() == Status::Playing;
    let mut bar = AppTitlebar::new(width)
        .height(HEIGHT)
        .start_boxed(icon_button(p, "settings", false, crate::app::open_settings));
    if video {
        let title = media.map(|m| m.title).unwrap_or_default();
        bar = bar.foreground(Color::rgb(255, 255, 255)).center(
            Text::new(ellipsize(&title, ((width - 260.0) / 7.5).max(8.0) as usize))
                .font_size(13.0)
                .bold(true)
                .color(Color::rgba(255, 255, 255, 235)),
        );
    }
    let background: creamui_core::Background = if video {
        creamui_core::LinearGradient::new(180.0, Color::rgba(0, 0, 0, 130), CLEAR).into()
    } else {
        CLEAR.into()
    };
    Box::new(
        bar.layout(Style {
            position: creamui_core::layout::Position::Absolute,
            inset: creamui_core::layout::Rect {
                top: LengthPercentageAuto::Length(-(1.0 - reveal) * HEIGHT),
                left: LengthPercentageAuto::Length(0.0),
                right: LengthPercentageAuto::Length(0.0),
                bottom: LengthPercentageAuto::Auto,
            },
            ..Default::default()
        })
        .background(background),
    )
}
