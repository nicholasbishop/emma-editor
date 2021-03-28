use {
    super::{App, APP},
    crate::{
        key_map::{Action, Direction, KeyMap, KeyMapLookup, KeyMapStack, Move},
        key_sequence::{is_modifier, KeySequence, KeySequenceAtom},
    },
    gtk4::{self as gtk, gdk, glib::signal::Inhibit, prelude::*},
};

pub(super) fn create_gtk_key_handler(window: &gtk::ApplicationWindow) {
    let key_controller = gtk::EventControllerKey::new();
    key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    key_controller.connect_key_pressed(|_self, keyval, _keycode, state| {
        APP.with(|app| {
            app.borrow_mut()
                .as_mut()
                .unwrap()
                .handle_key_press(keyval, state)
        })
    });
    window.add_controller(&key_controller);
}

pub(super) struct KeyHandler {
    base_keymap: KeyMap,
    cur_seq: KeySequence,
}

impl KeyHandler {
    pub(super) fn new() -> KeyHandler {
        KeyHandler {
            base_keymap: KeyMap::new(),
            cur_seq: KeySequence::default(),
        }
    }
}

impl App {
    fn move_cursor(&mut self, step: Move, dir: Direction) {
        let pane = self.pane_tree.active_mut();
        let buf = self.buffers.get(pane.buffer_id()).unwrap();
        let text = buf.text();
        let mut cursor = pane.cursor();

        match step {
            Move::Char => {
                if dir == Direction::Dec {
                    if cursor.line_offset == 0 {
                        if cursor.line > 0 {
                            let line = text.line(cursor.line);
                            cursor.line -= 1;
                            cursor.line_offset = line.len_chars();
                        }
                    } else {
                        cursor.line_offset -= 1;
                    }
                } else {
                    let line = text.line(cursor.line);
                    if cursor.line_offset + 1 == line.len_chars() {
                        if cursor.line + 1 < text.len_lines() {
                            cursor.line += 1;
                            cursor.line_offset = 0;
                        }
                    } else {
                        cursor.line_offset += 1;
                    }
                }
            }
            _ => todo!(),
        }

        pane.set_cursor(cursor);
        self.widget.queue_draw();
    }

    pub(super) fn handle_key_press(
        &mut self,
        key: gdk::keys::Key,
        state: gdk::ModifierType,
    ) -> Inhibit {
        let mut keymap_stack = KeyMapStack::default();
        keymap_stack.push(self.key_handler.base_keymap.clone());

        // Ignore lone modifier presses.
        if is_modifier(&key) {
            return Inhibit(false);
        }

        // TODO: we want to ignore combo modifier presses too if no
        // non-modifier key is selected, e.g. pressing alt and then
        // shift, but currently that is treated as a valid
        // sequence. Need to figure out how to prevent that.

        let atom = KeySequenceAtom::from_event(key, state);
        self.key_handler.cur_seq.0.push(atom);

        let mut clear_seq = true;
        let inhibit = Inhibit(true);
        match keymap_stack.lookup(&self.key_handler.cur_seq) {
            KeyMapLookup::BadSequence => {
                // TODO: display some kind of non-blocking error
                dbg!("bad seq", &self.key_handler.cur_seq);
            }
            KeyMapLookup::Prefix => {
                clear_seq = false;
                // Waiting for the sequence to be completed.
            }
            KeyMapLookup::Action(Action::Exit) => {
                self.window.close();
            }
            KeyMapLookup::Action(Action::Move(step, dir)) => {
                self.move_cursor(step, dir);
            }
            _ => {
                todo!();
            }
        }

        if clear_seq {
            self.key_handler.cur_seq.0.clear();
        }

        inhibit
    }
}
