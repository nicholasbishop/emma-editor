#![warn(clippy::use_self)]
// TODO
#![expect(unused)]

mod app;
mod buffer;
mod config;
mod grapheme;
mod key_map;
mod key_sequence;
mod pane_tree;
mod rope;
mod shell;
mod theme;
mod util;

use app::App;
use iced::Font;

fn main() -> iced::Result {
    tracing_subscriber::fmt::init();

    iced::application("emma", App::update, App::view)
        .default_font(Font::MONOSPACE)
        .subscription(App::subscription)
        .run_with(App::new)
}
