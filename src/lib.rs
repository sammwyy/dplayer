pub mod app;
pub mod backends;
pub mod cli;
mod components;
mod engine;
mod icons;
mod shortcuts;
mod state;
mod views;

pub const MEDIA_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mkv", "webm", "avi", "mov", "ts", "mts", "m2ts", "mpg", "mpeg", "wmv", "flv",
    "3gp", "ogv", "mp3", "flac", "ogg", "oga", "opus", "m4a", "aac", "wav", "wv", "aiff", "aif",
    "wma",
];

pub use app::run;
