use {
    super::{App, InteractiveState, APP},
    crate::{
        buffer::{Buffer, Position},
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
    fn insert_char(&mut self, key: gdk::keys::Key) {
        // Insert a character into the active pane.
        if let Some(c) = key.to_unicode() {
            let pane = self.pane_tree.active();
            let buf = self
                .buffers
                .get_mut(pane.buffer_id())
                .expect("invalid buffer");
            let pos = buf.cursor(pane);
            buf.insert_char(c, pos);
        }
    }

    fn move_cursor(&mut self, step: Move, dir: Direction) {
        let pane = self.pane_tree.active_mut();
        let buf = self.buffers.get_mut(pane.buffer_id()).unwrap();
        let text = buf.text();
        let mut cursor = buf.cursor(pane);

        match step {
            Move::Char => {
                if dir == Direction::Dec {
                    cursor.0 = prev_grapheme_boundary(
                        &text.slice(0..text.len_chars()),
                        cursor.0,
                    );
                } else {
                    cursor.0 = next_grapheme_boundary(
                        &text.slice(0..text.len_chars()),
                        cursor.0,
                    );
                }
            }
            Move::Line => {
                let mut lp = cursor.line_position(buf);

                // When moving between lines, use grapheme offset
                // rather than char offset to keep the cursor more or
                // less visually horizontally aligned. Probably would
                // need to be more sophisticated for non-monospace
                // fonts though.
                if dir == Direction::Dec {
                    if lp.line > 0 {
                        let num_graphemes = lp.grapheme_offset(buf);

                        lp.line -= 1;
                        lp.set_offset_in_graphemes(buf, num_graphemes);
                    }
                } else {
                    if lp.line + 1 < text.len_lines() {
                        let num_graphemes = lp.grapheme_offset(buf);

                        lp.line += 1;
                        lp.set_offset_in_graphemes(buf, num_graphemes);
                    }
                }
                cursor = Position::from_line_position(lp, buf);
            }
            Move::LineEnd => {
                let mut lp = cursor.line_position(buf);
                if dir == Direction::Dec {
                    // TODO: add logic to initially move to
                    // first-non-whitespace char.
                    lp.offset = 0;
                } else {
                    lp.offset = text.line(lp.line).len_chars() - 1;
                }
                cursor = Position::from_line_position(lp, buf);
            }
            Move::Page => {
                dbg!("TODO");
            }
            Move::BufferEnd => {
                if dir == Direction::Dec {
                    cursor.0 = 0;
                } else {
                    cursor.0 = text.len_chars();
                }
            }
        }

        buf.set_cursor(pane, cursor);
    }

    fn minibuf_mut(&mut self) -> &mut Buffer {
        let id = self.pane_tree.minibuf().buffer_id();

        self.buffers.get_mut(id).expect("missing minibuf buffer")
    }

    fn handle_action(&mut self, action: Action) {
        match action {
            Action::Exit => {
                self.window.close();
            }
            Action::Move(step, dir) => {
                self.move_cursor(step, dir);
            }
            Action::SplitPane(orientation) => {
                self.pane_tree.split(
                    orientation,
                    self.buffers
                        .get_mut(self.pane_tree.active().buffer_id())
                        .expect("invalid buffer ID"),
                );
            }
            Action::PreviousPane => {
                let pane_id;
                {
                    let panes = self.pane_tree.panes();
                    let index = panes
                        .iter()
                        .position(|pane| pane.is_active())
                        .expect("no active pane");
                    let prev = if index == 0 {
                        panes.len() - 1
                    } else {
                        index - 1
                    };
                    pane_id = panes[prev].id().clone();
                }
                self.pane_tree.set_active(&pane_id);
            }
            Action::NextPane => {
                let pane_id;
                {
                    let panes = self.pane_tree.panes();
                    let index = panes
                        .iter()
                        .position(|pane| pane.is_active())
                        .expect("no active pane");
                    let next = if index + 1 == panes.len() {
                        0
                    } else {
                        index + 1
                    };
                    pane_id = panes[next].id().clone();
                }
                self.pane_tree.set_active(&pane_id);
            }
            Action::OpenFile => {
                self.interactive_state = InteractiveState::OpenFile;
                // TODO: prompt
                self.pane_tree.set_minibuf_interactive(true);
            }
            Action::Cancel => {
                self.interactive_state = InteractiveState::Initial;
                self.pane_tree.set_minibuf_interactive(false);
                self.minibuf_mut().clear();
            }
            todo => {
                dbg!(todo);
            }
        }
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

        let atom = KeySequenceAtom::from_event(key.clone(), state);
        self.key_handler.cur_seq.0.push(atom);

        let mut clear_seq = true;
        let inhibit = Inhibit(true);
        match keymap_stack.lookup(&self.key_handler.cur_seq) {
            KeyMapLookup::NoEntry => {
                self.insert_char(key);
            }
            KeyMapLookup::BadSequence => {
                // TODO: display some kind of non-blocking error
                dbg!("bad seq", &self.key_handler.cur_seq);
            }
            KeyMapLookup::Prefix => {
                clear_seq = false;
                // Waiting for the sequence to be completed.
            }
            KeyMapLookup::Action(action) => {
                self.handle_action(action);
            }
        }

        if clear_seq {
            self.key_handler.cur_seq.0.clear();
        }

        // Not every action requires redraw, but most do, no harm
        // occasionally redrawing when not needed.
        self.widget.queue_draw();

        inhibit
    }
}
