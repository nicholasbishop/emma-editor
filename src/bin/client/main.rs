mod key_sequence;

use gio::prelude::*;
use gtk::prelude::*;
use key_sequence::{KeySequence, KeySequenceAtom};
use std::cell::RefCell;
use std::collections::BTreeMap;
use std::env;
use std::rc::Rc;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Action {
    Exit,
    OpenFile,
}

enum KeyMapLookup {
    Action(Action),
    Prefix,
    NoEntry,
    BadSequence,
}

#[derive(Clone, Debug, Default)]
struct KeyMap(BTreeMap<KeySequence, Action>);

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

    fn insert(&mut self, seq: KeySequence, action: Action) {
        self.0.insert(seq, action);
    }

    fn lookup(&self, seq: &KeySequence) -> KeyMapLookup {
        // First check for the exact sequence
        if let Some(action) = self.0.get(seq) {
            return KeyMapLookup::Action(*action);
        }

        // Then check if the sequence could be a prefix for something
        // in the map.
        if self.contains_prefix(seq) {
            return KeyMapLookup::Prefix;
        }

        // At this point we know the sequence is not in the map.

        // If the sequence's length is 1 and it doesn't have any
        // modifiers then just pass it along; this handles things like
        // pressing the letter 'a' where we just want the default
        // insertion action to occur.
        if seq.0.len() == 1 && seq.0[0].modifiers.is_empty() {
            return KeyMapLookup::NoEntry;
        }

        // TODO: special "<ctrl>g" type thing to kill any sequence

        KeyMapLookup::BadSequence
    }

    /// Check if `seq` matches a prefix.
    fn contains_prefix(&self, seq: &KeySequence) -> bool {
        // TODO: should be able to make this more efficient by
        // starting the search at the appropriate place.
        for k in self.0.keys() {
            if k.starts_with(&seq) {
                return true;
            }
        }
        false
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
        };

        if clear_seq {
            cur_seq.borrow_mut().0.clear();
        }

        res
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
