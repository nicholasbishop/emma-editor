#![warn(clippy::use_self)]

mod app;
mod buffer;
mod config;
mod grapheme;
mod key;
mod key_map;
mod key_sequence;
mod overlay;
mod pane_tree;
mod path_chooser;
mod process;
mod rope;
mod search_widget;
mod shell;
mod theme;
mod util;
mod widget;

use gtk4 as gtk;
use gtk4::prelude::*;

fn main() {
    tracing_subscriber::fmt::init();

    let application = gtk::Application::builder()
        .application_id("org.emma.Emma")
        .register_session(true)
        .build();

    application.connect_startup(app::init);

    application.run();
}
