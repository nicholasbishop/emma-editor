#![warn(clippy::use_self)]

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

use gtk4 as gtk;
use gtk4::prelude::*;

fn main() {
    tracing_subscriber::fmt::init();

    let application = gtk::Application::builder()
        .application_id("org.emma.Emma")
        .register_session(true)
        .build();
    // let application =
    //     gtk::Application::new(Some("org.emma.Emma"), Default::default());

    application.connect_startup(app::init);

    application.run();
}
