pub mod controls;
pub mod queue;
pub mod stage;
pub(crate) mod chrome;

use crate::components::*;
use crate::state::PlayerState;
use creamui_core::Size;

pub fn build(p: &PlayerState, size: Size) -> BoxedWidget {
    creamui_reactive::with_context_scope(|| {
        let mut children = vec![stage::build(p)];
        let chrome = p.controls_visible.get();
        if chrome {
            if let Some(bar) = chrome::build(p) {
                children.push(bar);
            }
        }
        if chrome {
            children.push(controls::build(p));
        }
        if p.queue_open.get() {
            children.push(queue::build(p));
        }
        view(
            Style {
                size: creamui_core::layout::Size {
                    width: Dimension::Length(size.width),
                    height: Dimension::Length(size.height),
                },
                ..column(0.0)
            },
            children,
        )
    })
}
