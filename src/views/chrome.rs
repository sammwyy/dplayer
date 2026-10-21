use crate::components::*;
use crate::state::PlayerState;
use creamui_core::WindowDragHandle;
use creamui_render::{CompositorIntegrationRequest, WindowDecorationMode, WindowHandle};

pub(crate) fn request_hybrid(handle: &WindowHandle) {
    handle.set_compositor_integration(Some(CompositorIntegrationRequest::Hybrid));
}

pub fn build(p: &PlayerState) -> Option<BoxedWidget> {
    let decorations = creamui_render::use_window_decorations();
    let controls = decorations.controls;
    let client = decorations.mode == WindowDecorationMode::Client;
    // Reserve the compositor's top-right window control inset in Hybrid mode
    // so the drag region does not intercept clicks meant for window buttons.
    let right_pad = if decorations.mode == WindowDecorationMode::Hybrid
        && controls.width > 0
        && controls.height > 0
        && (controls.y as f32) < 36.0
    {
        controls.width as f32 + 8.0
    } else {
        6.0
    };

    // Initiate window moves from a hit-tested button rather than `CUIWindowDragArea`
    // because the fullscreen stage button underneath would otherwise claim the hit test.
    let window_drag: Option<WindowDragHandle> = creamui_reactive::try_use_context();
    let mut children: Vec<BoxedWidget> = vec![Box::new(
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

    Some(Box::new(
        RawView::new(Style {
            position: creamui_core::layout::Position::Absolute,
            inset: creamui_core::layout::Rect {
                top: LengthPercentageAuto::Length(0.0),
                left: LengthPercentageAuto::Length(0.0),
                right: LengthPercentageAuto::Length(0.0),
                bottom: LengthPercentageAuto::Auto,
            },
            size: creamui_core::layout::Size {
                width: Dimension::Auto,
                height: Dimension::Length(36.0),
            },
            padding: creamui_core::layout::Rect {
                left: LengthPercentage::Length(6.0),
                right: LengthPercentage::Length(right_pad),
                top: LengthPercentage::Length(2.0),
                bottom: LengthPercentage::Length(0.0),
            },
            ..row(0.0)
        })
        .background(creamui_core::LinearGradient::new(
            180.0,
            Color::rgba(0, 0, 0, 110),
            Color::rgba(0, 0, 0, 0),
        ))
        .with_children(children),
    ))
}
