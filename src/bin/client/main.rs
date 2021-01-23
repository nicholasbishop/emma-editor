mod key_sequence;

use gdk::keys::constants as keys;
use gio::prelude::*;
use gtk::prelude::*;
use key_sequence::KeySequenceAtom;
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
        // Ignore lone modifier presses.
        if e.get_is_modifier() {
            return Inhibit(false);
        }

        let atom = KeySequenceAtom::from_event(e);

        if atom.key == keys::Escape {
            // TODO: for now make it easy to quit
            std::process::exit(0);
        } else if atom.modifiers == gdk::ModifierType::CONTROL_MASK
            && atom.key == keys::f
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
