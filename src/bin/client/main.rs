mod key_map;
mod key_sequence;

use gio::prelude::*;
use gtk::prelude::*;
use key_map::{Action, KeyMap, KeyMapLookup};
use key_sequence::{KeySequence, KeySequenceAtom};
use std::cell::RefCell;
use std::env;
use std::rc::Rc;

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("emma");
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(640, 480);

    let layout = gtk::Box::new(gtk::Orientation::Vertical, 1);

    let text = gtk::TextView::new();
    let minibuf = gtk::TextView::new();
    minibuf.set_size_request(-1, 26); // TODO

    layout.pack_start(&text, true, true, 0);
    layout.pack_start(&minibuf, false, true, 0);

    window.add(&layout);

    let keymap = KeyMap::new();
    let cur_seq = Rc::new(RefCell::new(KeySequence::default()));

    window.add_events(gdk::EventMask::KEY_PRESS_MASK);
    window.connect_key_press_event(move |_, e| {
        // Ignore lone modifier presses.
        if e.get_is_modifier() {
            return Inhibit(false);
        }

        let atom = KeySequenceAtom::from_event(e);
        cur_seq.borrow_mut().0.push(atom);

        let mut clear_seq = true;
        let res = match keymap.lookup(&cur_seq.borrow()) {
            KeyMapLookup::NoEntry => {
                // Allow default handling to occur, e.g. inserting a
                // character into the text widget.
                Inhibit(false)
            }
            KeyMapLookup::BadSequence => {
                // TODO: display some kind of non-blocking error
                dbg!("bad seq");
                Inhibit(true)
            }
            KeyMapLookup::Prefix => {
                clear_seq = false;
                // Waiting for the sequence to be completed.
                Inhibit(true)
            }
            KeyMapLookup::Action(Action::Exit) => {
                std::process::exit(0);
            }
            KeyMapLookup::Action(Action::OpenFile) => {
                dbg!("todo: open file");
                Inhibit(true)
            }
            KeyMapLookup::Action(Action::NextView) => {
                todo!("next view");
            }
        };

        if clear_seq {
            cur_seq.borrow_mut().0.clear();
        }

        res
    });

    window.show_all();
}

fn main() {
    let application =
        gtk::Application::new(Some("org.emma.Emma"), Default::default())
            .expect("Initialization failed...");

    application.connect_activate(|app| {
        build_ui(app);
    });

    application.run(&env::args().collect::<Vec<_>>());
}
