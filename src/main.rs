#![warn(clippy::use_self)]

mod app;
mod buffer;
mod config;
mod grapheme;
mod key_map;
mod key_sequence;
mod open_file;
mod pane_tree;
mod rope;
mod shell;
mod theme;
mod util;

use relm4::RelmApp;

fn main() {
    tracing_subscriber::fmt::init();

    let app = RelmApp::new("emma");
    app.run::<app::AppState>(());
}
