use {
    super::{App, APP},
    crate::{
        buffer::Position,
        grapheme::{next_grapheme_boundary, prev_grapheme_boundary},
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
                    cursor.0 = prev_grapheme_boundary(
                        &text.slice(0..text.len_chars()),
                        cursor.0,
                    );
                } else {
                    // TODO: figure out why this if is needed but Dec
                    // doesn't need it. (Cursor isn't drawn at end of
                    // buffer?)
                    if cursor.0 + 1 < text.len_chars() {
                        cursor.0 = next_grapheme_boundary(
                            &text.slice(0..text.len_chars()),
                            cursor.0,
                        );
                    }
                }
            }
            Move::Line => {
                // When moving between lines, use grapheme offset
                // rather than char offset to keep the cursor more or
                // less visually horizontally aligned. Probably would
                // need to be more sophisticated for non-monospace
                // fonts though.
                if dir == Direction::Dec {
                    let mut lp = cursor.line_position(buf);
                    if lp.line > 0 {
                        let num_graphemes = lp.grapheme_offset(buf);

                        lp.line -= 1;
                        lp.set_offset_in_graphemes(buf, num_graphemes);

                        cursor = Position::from_line_position(lp, buf);
                    }
                } else {
                    let mut lp = cursor.line_position(buf);
                    if lp.line + 1 < text.len_lines() {
                        let num_graphemes = lp.grapheme_offset(buf);

                        lp.line += 1;
                        lp.set_offset_in_graphemes(buf, num_graphemes);

                        cursor = Position::from_line_position(lp, buf);
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
            KeyMapLookup::Action(Action::SplitPane(orientation)) => {
                self.pane_tree.split(orientation)
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
