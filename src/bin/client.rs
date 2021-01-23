use gdk::keys::constants as keys;
use gio::prelude::*;
use gtk::prelude::*;

use std::env;

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("emma");
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(350, 70);

    let text = gtk::TextView::new();

    window.add(&text);

    window.add_events(gdk::EventMask::KEY_PRESS_MASK);
    window.connect_key_press_event(|_, e| {
        if e.get_is_modifier() {
            Inhibit(false)
        } else if e.get_keyval() == keys::Escape {
            // TODO: for now make it easy to quit
            std::process::exit(0);
        } else if e.get_state() == gdk::ModifierType::CONTROL_MASK
            && e.get_keyval() == keys::f
        {
            dbg!("C-f");
            Inhibit(true)
        } else {
            Inhibit(false)
        }
    });

    window.show_all();
}

fn main() {
    let application = gtk::Application::new(
        Some("com.github.gtk-rs.examples.basic"),
        Default::default(),
    )
    .expect("Initialization failed...");

    application.connect_activate(|app| {
        build_ui(app);
    });

    application.run(&env::args().collect::<Vec<_>>());
}
