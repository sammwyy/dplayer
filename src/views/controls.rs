use crate::components::*;
use crate::state::{PlayerState, Repeat, Status};
use creamui_widgets::{RawSlider, Text};

fn frac(point: Point, rect: Rect) -> f64 {
    (point.x / rect.width.max(1.0)).clamp(0.0, 1.0) as f64
}

fn seekbar(p: &PlayerState) -> BoxedWidget {
    let t = use_theme();
    let duration = p.duration.get();
    let fraction = if duration > 0.0 {
        (p.shown_position() / duration).clamp(0.0, 1.0) as f32
    } else {
        0.0
    };
    let (start, update, end) = (p.clone(), p.clone(), p.clone());
    let rail = view(
        Style {
            flex_grow: 1.0,
            flex_basis: Dimension::Length(0.0),
            align_items: Some(AlignItems::Center),
            ..row(0.0)
        },
        vec![Box::new(
            RawView::new(Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(0.0),
                size: creamui_core::layout::Size {
                    width: Dimension::Auto,
                    height: Dimension::Length(5.0),
                },
                ..row(0.0)
            })
            .background(Color::rgba(255, 255, 255, 38))
            .corner_radius(2.5)
            .child(Box::new(
                RawView::new(Style {
                    size: creamui_core::layout::Size {
                        width: Dimension::Percent(fraction),
                        height: Dimension::Length(5.0),
                    },
                    ..row(0.0)
                })
                .background(t.colors.accent)
                .corner_radius(2.5)
                .child(Box::new(
                    RawView::new(Style {
                        position: creamui_core::layout::Position::Absolute,
                        size: creamui_core::layout::Size {
                            width: Dimension::Length(13.0),
                            height: Dimension::Length(13.0),
                        },
                        inset: creamui_core::layout::Rect {
                            top: LengthPercentageAuto::Length(-4.0),
                            right: LengthPercentageAuto::Length(-7.0),
                            bottom: LengthPercentageAuto::Auto,
                            left: LengthPercentageAuto::Auto,
                        },
                        ..row(0.0)
                    })
                    .background(t.colors.text_primary)
                    .corner_radius(6.5)
                    .box_shadow(creamui_core::BoxShadow::new(
                        0.0,
                        1.0,
                        6.0,
                        0.0,
                        Color::rgba(0, 0, 0, 140),
                    )),
                )),
            )),
        )],
    );
    Box::new(
        RawButton::new(
            Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(0.0),
                size: creamui_core::layout::Size {
                    width: Dimension::Auto,
                    height: Dimension::Length(20.0),
                },
                align_items: Some(AlignItems::Center),
                ..row(0.0)
            },
            || {},
        )
        .keyboard_focus(false)
        .background(CLEAR)
        .with_drag_start(move |point, rect| start.begin_scrub(frac(point, rect)))
        .with_drag(move |point, rect| update.update_scrub(frac(point, rect)))
        .with_drag_end(move || end.end_scrub())
        .child(rail),
    )
}

fn icon_button(
    p: &PlayerState,
    icon: &str,
    active: bool,
    on_click: impl Fn() + 'static,
) -> BoxedWidget {
    let t = use_theme();
    let color = if active {
        t.colors.accent
    } else {
        t.colors.text_secondary
    };
    Box::new(
        RawButton::new(
            Style {
                justify_content: Some(JustifyContent::Center),
                align_items: Some(AlignItems::Center),
                ..fixed(30.0, 30.0)
            },
            on_click,
        )
        .keyboard_focus(false)
        .corner_radius(8.0)
        .hover_style(StateStyle::new().background(t.colors.surface_hover))
        .child(Box::new(Icon::new(p.icons.get(icon), color).size(16.0))),
    )
}

fn play_button(p: &PlayerState) -> BoxedWidget {
    let t = use_theme();
    let playing = p.playing.get();
    let icon = p.icons.get(if playing { "pause" } else { "play" });
    let p = p.clone();
    Box::new(
        RawButton::new(
            Style {
                justify_content: Some(JustifyContent::Center),
                align_items: Some(AlignItems::Center),
                ..fixed(38.0, 38.0)
            },
            move || p.toggle(),
        )
        .keyboard_focus(false)
        .background(t.colors.accent)
        .corner_radius(19.0)
        .hover_style(StateStyle::new().background(
            t.colors.accent.mix(t.colors.text_primary, 0.12),
        ))
        .child(Box::new(Icon::new(icon, t.colors.selection_text).size(19.0))),
    )
}

pub fn build(p: &PlayerState) -> BoxedWidget {
    let t = use_theme();
    let duration = p.duration.get();
    let position = p.shown_position();

    let seek_row = view(row(10.0), vec![
        view(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(46.0),
                    height: Dimension::Auto,
                },
                flex_shrink: 0.0,
                ..row(0.0)
            },
            vec![label(format_time(position), 11.5, true)],
        ),
        seekbar(p),
        view(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(46.0),
                    height: Dimension::Auto,
                },
                flex_shrink: 0.0,
                justify_content: Some(JustifyContent::End),
                ..row(0.0)
            },
            vec![label(format_time(duration), 11.5, true)],
        ),
    ]);

    let mut left: Vec<BoxedWidget> = vec![
        {
            let p = p.clone();
            let active = p.shuffle.get();
            icon_button(&p, "shuffle", active, {
                let p = p.clone();
                move || p.toggle_shuffle()
            })
        },
        {
            let p = p.clone();
            icon_button(&p, "prev", false, {
                let p = p.clone();
                move || p.prev()
            })
        },
        play_button(p),
        {
            let p = p.clone();
            icon_button(&p, "next", false, {
                let p = p.clone();
                move || {
                    p.advance(true);
                }
            })
        },
        {
            let p = p.clone();
            let repeat = p.repeat.get();
            let icon = if repeat == Repeat::One {
                "repeat-one"
            } else {
                "repeat"
            };
            icon_button(&p, icon, repeat != Repeat::Off, {
                let p = p.clone();
                move || p.cycle_repeat()
            })
        },
    ];

    let info: BoxedWidget = match (p.media.get(), p.status.get()) {
        (Some(media), _) => {
            let mut lines = vec![Box::new(
                Text::new(media.title.clone()).font_size(12.5),
            ) as BoxedWidget];
            let mut detail = String::new();
            if media.has_video {
                detail.push_str(&media.video_codec);
                detail.push_str(&format!(" · {}×{}", media.width, media.height));
            }
            if !media.audio_codec.is_empty() {
                if !detail.is_empty() {
                    detail.push_str(" · ");
                }
                detail.push_str(&media.audio_codec);
            }
            if let Some(artist) = &media.artist {
                detail.push_str(&format!(" · {artist}"));
            }
            if let Some(album) = &media.album {
                detail.push_str(&format!(" · {album}"));
            }
            if !detail.is_empty() {
                lines.push(label(detail, 10.5, true));
            }
            view(
                pad(
                    Style {
                        align_items: Some(AlignItems::Start),
                        ..column(1.0)
                    },
                    10.0,
                    0.0,
                ),
                lines,
            )
        }
        (None, Status::Loading) => label("Loading…", 12.0, true),
        _ => view(row(0.0), vec![]),
    };

    let right: Vec<BoxedWidget> = vec![
        {
            let p = p.clone();
            icon_button(&p, "open", false, {
                let p = p.clone();
                move || p.open_dialog()
            })
        },
        {
            let p = p.clone();
            let open = p.queue_open.get();
            icon_button(&p, "queue", open, {
                let p = p.clone();
                move || {
                    p.queue_open.set(!p.queue_open.get());
                }
            })
        },
        {
            let p = p.clone();
            let speed = p.speed.get();
            let p2 = p.clone();
            Box::new(
                RawButton::new(
                    pad(
                        Style {
                            justify_content: Some(JustifyContent::Center),
                            ..fixed(46.0, 30.0)
                        },
                        0.0,
                        0.0,
                    ),
                    move || p2.cycle_speed(true),
                )
                .keyboard_focus(false)
                .corner_radius(8.0)
                .hover_style(StateStyle::new().background(t.colors.surface_hover))
                .child(Box::new(
                    Text::new(format!("{}×", speed))
                        .font_size(11.5)
                        .color(if (speed - 1.0).abs() > 0.001 {
                            t.colors.accent
                        } else {
                            t.colors.text_secondary
                        }),
                )),
            ) as BoxedWidget
        },
        {
            let p = p.clone();
            let has_audio = p.engine.has_audio_output();
            let muted = p.muted.get() || !has_audio;
            icon_button(&p, if muted { "mute" } else { "volume" }, muted, {
                let p = p.clone();
                move || p.toggle_mute()
            })
        },
        {
            let p = p.clone();
            Box::new(
                RawSlider::new(
                    Style {
                        size: creamui_core::layout::Size {
                            width: Dimension::Length(84.0),
                            height: Dimension::Length(30.0),
                        },
                        flex_shrink: 0.0,
                        ..row(0.0)
                    },
                    p.volume.get(),
                    t.colors.surface_hover,
                    t.colors.accent,
                    t.colors.text_primary,
                    move |v| p.set_volume(v),
                )
                .track(4.0, 2.0)
                .handle(11.0, 5.5),
            ) as BoxedWidget
        },
    ];

    let transport = view(row(6.0), {
        let mut row_children = left.split_off(0);
        row_children.push(info);
        row_children.push(view(
            Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(0.0),
                justify_content: Some(JustifyContent::End),
                ..row(6.0)
            },
            right,
        ));
        row_children
    });

    Box::new(
        RawButton::new(
            Style {
                position: creamui_core::layout::Position::Absolute,
            inset: creamui_core::layout::Rect {
                left: LengthPercentageAuto::Length(16.0),
                right: LengthPercentageAuto::Length(16.0),
                bottom: LengthPercentageAuto::Length(14.0),
                top: LengthPercentageAuto::Auto,
            },
            padding: creamui_core::layout::Rect {
                left: LengthPercentage::Length(12.0),
                right: LengthPercentage::Length(12.0),
                top: LengthPercentage::Length(9.0),
                bottom: LengthPercentage::Length(9.0),
            },
            ..column(6.0)
            },
            || {},
        )
        .keyboard_focus(false)
        .background(t.colors.surface_elevated.mix(Color::rgba(8, 8, 10, 255), 0.45))
        .corner_radius(14.0)
        .border(Color::rgba(255, 255, 255, 26), 1.0)
        .with_children(vec![seek_row, transport]),
    )
}
