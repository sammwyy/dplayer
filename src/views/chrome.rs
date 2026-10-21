use crate::components::*;
use crate::state::{PlayerState, Status};
use creamui_core::WindowDragHandle;
use creamui_render::{CompositorIntegrationRequest, WindowDecorationMode, WindowHandle};

pub(crate) fn request_hybrid(handle: &WindowHandle) {
    handle.set_compositor_integration(Some(CompositorIntegrationRequest::Hybrid));
}

pub const HEIGHT: f32 = 40.0;

pub fn build(p: &PlayerState, reveal: f32, width: f32) -> Option<BoxedWidget> {
    let decorations = creamui_render::use_window_decorations();
    let controls = decorations.controls;
    let client = decorations.mode == WindowDecorationMode::Client;
    // Reserve the compositor's window control inset in Hybrid mode so the
    // drag region does not intercept clicks meant for window buttons.
    let (left_pad, right_pad) = if decorations.mode == WindowDecorationMode::Hybrid
        && controls.width > 0
        && controls.height > 0
        && (controls.y as f32) < HEIGHT
    {
        let center = controls.x as f32 + controls.width as f32 / 2.0;
        if center < width / 2.0 {
            ((controls.x + controls.width) as f32 + 8.0, 8.0)
        } else {
            (8.0, controls.width as f32 + 8.0)
        }
    } else {
        (8.0, 8.0)
    };

    // Initiate window moves from a hit-tested button rather than `CUIWindowDragArea`
    // because the fullscreen stage button underneath would otherwise claim the hit test.
    let window_drag: Option<WindowDragHandle> = creamui_reactive::try_use_context();
    let settings = icon_button(p, "settings", false, crate::app::open_settings);
    let mut children: Vec<BoxedWidget> = vec![settings, Box::new(
        RawButton::new(
            Style {
                flex_grow: 1.0,
                flex_basis: Dimension::Length(0.0),
                min_size: creamui_core::layout::Size {
                    width: Dimension::Length(0.0),
                    height: Dimension::Length(0.0),
                },
                ..row(0.0)
            },
            || {},
        )
        .keyboard_focus(false)
        .with_drag_start(move |_, _| {
            if let Some(drag) = &window_drag {
                drag.start_drag();
            }
        }),
    )];
    if client {
        let maximized = p.maximized.clone();
        let mut buttons: Vec<BoxedWidget> = Vec::new();
        for (index, icon) in ["minimize", "maximize", "close"].into_iter().enumerate() {
            let window = p.window.clone();
            let maximized = maximized.clone();
            buttons.push(icon_button(p, icon, false, move || {
                if let Some(handle) = window.borrow().as_ref() {
                    match index {
                        0 => handle.minimize(),
                        1 => {
                            let next = !maximized.peek();
                            handle.set_maximized(next);
                            maximized.set(next);
                        }
                        _ => handle.close(),
                    }
                }
            }));
        }
        children.push(view(
            Style {
                align_items: Some(AlignItems::Center),
                ..row(4.0)
            },
            buttons,
        ));
    }

    let video = p.media.get().is_some_and(|m| m.has_video) && p.status.get() == Status::Playing;
    let background = if video {
        creamui_core::LinearGradient::new(180.0, Color::rgba(0, 0, 0, 120), CLEAR)
    } else {
        creamui_core::LinearGradient::new(180.0, CLEAR, CLEAR)
    };
    Some(Box::new(
        RawView::new(Style {
            position: creamui_core::layout::Position::Absolute,
            inset: creamui_core::layout::Rect {
                top: LengthPercentageAuto::Length(-(1.0 - reveal) * HEIGHT),
                left: LengthPercentageAuto::Length(0.0),
                right: LengthPercentageAuto::Length(0.0),
                bottom: LengthPercentageAuto::Auto,
            },
            size: creamui_core::layout::Size {
                width: Dimension::Auto,
                height: Dimension::Length(HEIGHT),
            },
            padding: creamui_core::layout::Rect {
                left: LengthPercentage::Length(left_pad),
                right: LengthPercentage::Length(right_pad),
                top: LengthPercentage::Length(4.0),
                bottom: LengthPercentage::Length(0.0),
            },
            ..row(0.0)
        })
        .background(background)
        .with_children(children),
    ))
}
