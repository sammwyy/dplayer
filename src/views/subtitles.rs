use crate::components::*;
use crate::state::PlayerState;
use creamui_core::Size;
use creamui_widgets::{RawScrollView, Text};

pub fn build(p: &PlayerState, size: Size) -> BoxedWidget {
    let t = use_theme();
    let selected = p.subtitle.get();
    let mut tracks = vec![(None, "Off".to_string())];
    if let Some(media) = p.media.get() {
        tracks.extend(
            media
                .subtitles
                .into_iter()
                .map(|track| (Some(track.source), track.label)),
        );
    }
    let menu_height = (tracks.len() as f32 * 38.0)
        .min((size.height - 56.0).max(40.0))
        .min(310.0);
    let rows = tracks
        .into_iter()
        .map(|(source, name)| {
            let state = p.clone();
            let active = selected == source;
            Box::new(
                RawButton::new(pad(row(8.0), 12.0, 9.0), move || {
                    state.select_subtitle(source.clone())
                })
                .keyboard_focus(false)
                .background(if active {
                    t.colors.selection_background
                } else {
                    t.colors.surface_elevated
                })
                .hover_style(StateStyle::new().background(t.colors.surface_hover))
                .child(Box::new(
                    Text::new(ellipsize(&name, 34))
                        .font_size(12.0)
                        .color(t.colors.text_primary),
                )),
            ) as BoxedWidget
        })
        .collect();
    Box::new(
        RawView::new(Style {
            position: creamui_core::layout::Position::Absolute,
            inset: creamui_core::layout::Rect {
                top: LengthPercentageAuto::Length(crate::views::chrome::HEIGHT + 8.0),
                left: LengthPercentageAuto::Length(if size.width >= 350.0 { 76.0 } else { 8.0 }),
                right: LengthPercentageAuto::Auto,
                bottom: LengthPercentageAuto::Auto,
            },
            size: creamui_core::layout::Size {
                width: Dimension::Length(260.0_f32.min((size.width - 16.0).max(0.0))),
                height: Dimension::Length(menu_height),
            },
            ..column(0.0)
        })
        .background(t.colors.surface_elevated)
        .border(t.colors.border, 1.0)
        .corner_radius(12.0)
        .child(Box::new(
            RawScrollView::controlled(column(2.0), p.subtitles_scroll.clone())
                .scrollbar_color(t.colors.border_strong)
                .scrollbar_hover_color(t.colors.text_disabled)
                .scrollbar_pressed_color(t.colors.text_secondary)
                .child(view(column(2.0), rows)),
        )),
    )
}
