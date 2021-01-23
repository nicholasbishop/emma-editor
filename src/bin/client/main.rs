mod key_sequence;

use gio::prelude::*;
use gtk::prelude::*;
use key_sequence::{KeySequence, KeySequenceAtom};
use std::cell::RefCell;
use std::collections::HashMap;
use std::env;
use std::rc::Rc;

#[derive(Clone, Debug, Eq, PartialEq)]
enum Action {
    Exit,
    OpenFile,
}

#[derive(Clone, Debug)]
enum KeyMapValue {
    Action(Action),
    Prefix(KeyMap),
}

#[derive(Clone, Debug, Default)]
struct KeyMap {
    items: HashMap<KeySequenceAtom, KeyMapValue>,
}

impl KeyMap {
    fn new() -> KeyMap {
        let mut map = KeyMap::default();
        // TODO: for now make it easy to quit
        map.insert(KeySequence::parse("<esc>").unwrap(), Action::Exit);
        map.insert(
            KeySequence::parse("<ctrl>x+<ctrl>f").unwrap(),
            Action::OpenFile,
        );
        map
    }

    fn insert(&mut self, seq: KeySequence, _action: Action) {
        todo!();
    }

    fn lookup(&self, _seq: &KeySequence) -> Option<&KeyMapValue> {
        todo!();
    }
}

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("emma");
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(350, 70);

    let text = gtk::TextView::new();

    window.add(&text);

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

        match keymap.lookup(&cur_seq.borrow()) {
            None => {
                // TODO: if this is a sequence and the terminal has no
                // match then we should treat it as an error rather
                // than inhibiting.

                // Allow default handling to occur, e.g. inserting a
                // character into the text widget.
                Inhibit(false)
            }
            Some(KeyMapValue::Action(Action::Exit)) => {
                std::process::exit(0);
            }
            Some(KeyMapValue::Action(Action::OpenFile)) => {
                dbg!("C-f");
                Inhibit(true)
            }
            Some(KeyMapValue::Prefix(_)) => {
                // Waiting for the sequence to be completed.
                Inhibit(false)
            }
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
