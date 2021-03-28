use {
    super::App,
    crate::{
        key_map::{Action, KeyMapLookup, KeyMapStack},
        key_sequence::{is_modifier, KeySequenceAtom},
    },
    gtk4::{gdk, glib::signal::Inhibit, prelude::*},
};

impl App {
    pub(super) fn handle_key_press(
        &mut self,
        key: gdk::keys::Key,
        state: gdk::ModifierType,
    ) -> Inhibit {
        let mut keymap_stack = KeyMapStack::default();
        keymap_stack.push(self.base_keymap.clone());

        // Ignore lone modifier presses.
        if is_modifier(&key) {
            return Inhibit(false);
        }

        // TODO: we want to ignore combo modifier presses too if no
        // non-modifier key is selected, e.g. pressing alt and then
        // shift, but currently that is treated as a valid
        // sequence. Need to figure out how to prevent that.

        let atom = KeySequenceAtom::from_event(key, state);
        self.cur_seq.0.push(atom);

        let mut clear_seq = true;
        let inhibit = Inhibit(true);
        match keymap_stack.lookup(&self.cur_seq) {
            KeyMapLookup::BadSequence => {
                // TODO: display some kind of non-blocking error
                dbg!("bad seq", &self.cur_seq);
            }
            KeyMapLookup::Prefix => {
                clear_seq = false;
                // Waiting for the sequence to be completed.
            }
            KeyMapLookup::Action(Action::Exit) => {
                self.window.close();
            }
            _ => {
                todo!();
            }
        }

        if clear_seq {
            self.cur_seq.0.clear();
        }

        inhibit
    }
}
