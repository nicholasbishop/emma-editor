mod app;
mod buffer;
mod grapheme;
mod key_map;
mod key_sequence;
mod pane_tree;
mod theme;
mod util;

use gtk4::{self as gtk, prelude::*};

fn main() {
    tracing_subscriber::fmt::init();

    let application =
        gtk::Application::new(Some("org.emma.Emma"), Default::default())
            .expect("initialization failed");

    application.connect_activate(|app| app::init(app));

    application.run(&[]);
}
