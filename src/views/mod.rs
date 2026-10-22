pub mod audio;
pub(crate) mod chrome;
pub mod controls;
pub mod queue;
pub mod stage;
pub mod subtitles;

use crate::components::*;
use crate::state::{PlayerState, Status};
use creamui_core::Size;

/// Ease-out cubic, so the chrome decelerates as it settles.
fn ease(progress: f32) -> f32 {
    1.0 - (1.0 - progress.clamp(0.0, 1.0)).powi(3)
}

pub fn build(p: &PlayerState, size: Size) -> BoxedWidget {
    creamui_reactive::with_context_scope(|| {
        p.set_view_size(size.width, size.height);
        let mut children = vec![stage::build(p, size)];
        let reveal = ease(p.chrome_progress.get());
        if reveal > 0.0 {
            children.push(chrome::build(p, reveal, size.width));
            if p.status.get() != Status::Empty {
                children.push(controls::build(p, reveal, size.width));
            }
        }
        if p.queue_open.get() {
            children.push(queue::build(p));
        }
        if p.subtitles_open.get() && p.media.get().as_ref().is_some_and(|m| m.has_video) {
            children.push(subtitles::build(p, size));
            if p.subtitle.get().is_some() {
                children.push(subtitles::drag_handle(p, size));
            }
        }
        if p.audio_open.get() {
            children.push(audio::build(p, size));
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
