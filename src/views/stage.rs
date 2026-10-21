use crate::components::*;
use crate::state::{PlayerState, Status};
use creamui_image::Image;
use creamui_core::runtime::ImageFit;
use creamui_widgets::RawSpinner;

fn click_catch(p: &PlayerState, child: BoxedWidget) -> BoxedWidget {
    let p = p.clone();
    Box::new(
        RawButton::new(
            Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(0.0),
                min_size: creamui_core::layout::Size {
                    width: Dimension::Length(0.0),
                    height: Dimension::Length(0.0),
                },
                justify_content: Some(JustifyContent::Center),
                align_items: Some(AlignItems::Center),
                ..Default::default()
            },
            move || p.toggle(),
        )
        .keyboard_focus(false)
        .background(CLEAR)
        .child(child),
    )
}

fn welcome(p: &PlayerState) -> BoxedWidget {
    let t = use_theme();
    let open = p.clone();
    let open2 = p.clone();

    let kbd = |key: &str, text: &str| -> BoxedWidget {
        view(
            Style {
                align_items: Some(AlignItems::Center),
                ..row(6.0)
            },
            vec![
                Box::new(
                    RawView::new(pad(
                        Style {
                            justify_content: Some(JustifyContent::Center),
                            align_items: Some(AlignItems::Center),
                            ..row(0.0)
                        },
                        7.0,
                        3.0,
                    ))
                    .background(t.colors.surface_elevated)
                    .corner_radius(6.0)
                    .border(t.colors.border, 1.0)
                    .child(Box::new(
                        Text::new(key).font_size(10.5).color(t.colors.text_primary),
                    )),
                ),
                label(text, 11.5, true),
            ],
        )
    };

    view(
        Style {
            flex_grow: 1.0,
            flex_basis: Dimension::Length(0.0),
            justify_content: Some(JustifyContent::Center),
            align_items: Some(AlignItems::Center),
            ..column(0.0)
        },
        vec![view(
                    Style {
                        align_items: Some(AlignItems::Center),
                        ..column(0.0)
                    },
                    vec![
                        Box::new(
                            RawButton::new(
                                Style {
                                    size: creamui_core::layout::Size {
                                        width: Dimension::Length(96.0),
                                        height: Dimension::Length(96.0),
                                    },
                                    justify_content: Some(JustifyContent::Center),
                                    align_items: Some(AlignItems::Center),
                                    ..Default::default()
                                },
                                move || open.open_dialog(),
                            )
                            .background(creamui_core::LinearGradient::new(
                                135.0,
                                t.colors.accent,
                                t.colors.accent.mix(Color::rgba(0, 0, 0, 255), 0.25),
                            ))
                            .corner_radius(48.0)
                            .border(Color::rgba(255, 255, 255, 30), 1.0)
                            .box_shadow(creamui_core::BoxShadow::new(
                                0.0,
                                12.0,
                                36.0,
                                -8.0,
                                t.colors.accent.mix(Color::rgba(0, 0, 0, 0), 0.45),
                            ))
                            .hover_style(StateStyle::new().background(
                                t.colors.accent.mix(Color::rgba(255, 255, 255, 255), 0.12),
                            ))
                            .child(glyph(p, "play", 40.0, t.colors.selection_text)),
                        ),
                        view(fixed(0.0, 24.0), vec![]),
                        label("Media Player", 22.0, false),
                        view(fixed(0.0, 6.0), vec![]),
                        label("No media loaded", 13.0, true),
                        view(fixed(0.0, 22.0), vec![]),
                        view(
                            Style {
                                align_items: Some(AlignItems::Center),
                                ..row(14.0)
                            },
                            vec![
                                kbd("O", "open a file"),
                                label("\u{00b7}", 12.0, true),
                                label("or drop files anywhere", 11.5, true),
                            ],
                        ),
                        view(fixed(0.0, 10.0), vec![]),
                        Box::new(
                            RawButton::new(
                                pad(
                                    Style {
                                        justify_content: Some(JustifyContent::Center),
                                        ..row(0.0)
                                    },
                                    18.0,
                                    8.0,
                                ),
                                move || open2.open_dialog(),
                            )
                            .background(t.colors.surface_elevated)
                            .corner_radius(8.0)
                            .border(t.colors.border, 1.0)
                            .hover_style(StateStyle::new().background(
                                t.colors.surface_elevated
                                    .mix(Color::rgba(255, 255, 255, 255), 0.08),
                            ))
                            .child(Box::new(
                                Text::new("Open Media")
                                    .font_size(12.5)
                                    .color(t.colors.text_primary),
                            )),
                        ),
                    ],
                ),
        ],
    )
}

fn error(p: &PlayerState, message: &str) -> BoxedWidget {
    let t = use_theme();
    let open = p.clone();
    view(
        Style {
            flex_grow: 1.0,
            flex_basis: Dimension::Length(0.0),
            justify_content: Some(JustifyContent::Center),
            align_items: Some(AlignItems::Center),
            ..column(12.0)
        },
        vec![
            glyph(p, "app", 40.0, t.colors.text_secondary),
            label("Couldn't play this file", 15.0, false),
            label(message, 11.5, true),
            Box::new(
                RawButton::new(
                    pad(
                        Style {
                            justify_content: Some(JustifyContent::Center),
                            ..row(0.0)
                        },
                        18.0,
                        8.0,
                    ),
                    move || open.open_dialog(),
                )
                .background(t.colors.accent)
                .corner_radius(9.0)
                .hover_style(StateStyle::new().background(
                    t.colors.accent.mix(t.colors.text_primary, 0.12),
                ))
                .child(Box::new(
                    Text::new("Open another file")
                        .font_size(12.5)
                        .color(t.colors.selection_text),
                )),
            ),
        ],
    )
}

fn audio_stage(p: &PlayerState) -> BoxedWidget {
    let t = use_theme();
    let center: BoxedWidget = match p.cover.get() {
        Some(cover) => Box::new(
            Image::new(cover).fit(ImageFit::Contain).layout(Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Percent(0.62),
                    height: Dimension::Percent(0.72),
                },
                ..Default::default()
            }),
        ),
        None => Box::new(
            RawView::new(fixed(150.0, 150.0))
                .background(t.colors.surface_elevated)
                .corner_radius(75.0)
                .layout(Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Length(150.0),
                        height: Dimension::Length(150.0),
                    },
                    justify_content: Some(JustifyContent::Center),
                    align_items: Some(AlignItems::Center),
                    ..Default::default()
                })
                .child(glyph(p, "music", 64.0, t.colors.accent)),
        ),
    };
    view(
        Style {
            justify_content: Some(JustifyContent::Center),
            align_items: Some(AlignItems::Center),
            ..column(0.0)
        },
        vec![center],
    )
}

pub fn build(p: &PlayerState) -> BoxedWidget {
    let media = p.media.get();
    let content: BoxedWidget = match p.status.get() {
        Status::Empty => welcome(p),
        Status::Loading => view(
            Style {
                justify_content: Some(JustifyContent::Center),
                align_items: Some(AlignItems::Center),
                ..column(10.0)
            },
            vec![
                Box::new(RawSpinner::new(use_theme().colors.accent).size(30.0)),
                label("Loading…", 12.0, true),
            ],
        ),
        Status::Error(message) => error(p, &message),
        Status::Playing => match &media {
            Some(media) if media.has_video => {
                let image: BoxedWidget = match p.frame.get() {
                    Some(frame) => Box::new(
                        Image::new(frame).fit(ImageFit::Contain).layout(Style {
                            size: creamui_core::layout::Size {
                                width: Dimension::Percent(1.0),
                                height: Dimension::Percent(1.0),
                            },
                            ..Default::default()
                        }),
                    ),
                    None => view(row(0.0), vec![]),
                };
                click_catch(p, image)
            }
            Some(_) => click_catch(p, audio_stage(p)),
            None => welcome(p),
        },
    };

    Box::new(
        RawView::new(Style {
            flex_grow: 1.0,
            flex_basis: Dimension::Length(0.0),
            min_size: creamui_core::layout::Size {
                width: Dimension::Length(0.0),
                height: Dimension::Length(0.0),
            },
            ..column(0.0)
        })
        .background(BLACK)
        .with_children(vec![content]),
    )
}
