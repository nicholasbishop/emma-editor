use {
    super::{App, InteractiveState, APP},
    crate::{
        buffer::{Boundary, Buffer, BufferId, CharIndex, Direction},
        key_map::{Action, KeyMap, KeyMapLookup, KeyMapStack, Move},
        key_sequence::{is_modifier, KeySequence, KeySequenceAtom},
        pane_tree::{Pane, PaneTree},
    },
    anyhow::{anyhow, Error},
    fehler::throws,
    gtk4::{self as gtk, gdk, glib::signal::Inhibit, prelude::*},
    std::{collections::HashMap, path::Path},
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

fn invalid_active_buffer_error() -> Error {
    anyhow!("internal error: active pane points to invalid buffer")
}

#[throws]
fn active_buffer_mut<'a, 'b>(
    pane_tree: &'a PaneTree,
    buffers: &'b mut HashMap<BufferId, Buffer>,
) -> &'b mut Buffer {
    let pane = pane_tree.active();
    buffers
        .get_mut(pane.buffer_id())
        .ok_or_else(invalid_active_buffer_error)?
}

impl App {
    #[throws]
    fn active_buffer_mut(&mut self) -> &mut Buffer {
        let pane = self.pane_tree.active();
        let buf = self
            .buffers
            .get_mut(pane.buffer_id())
            .ok_or_else(invalid_active_buffer_error)?;
        buf
    }

    #[throws]
    fn active_pane_buffer_mut(&mut self) -> (&Pane, &mut Buffer) {
        let pane = self.pane_tree.active();
        let buf = self
            .buffers
            .get_mut(pane.buffer_id())
            .ok_or_else(invalid_active_buffer_error)?;
        (pane, buf)
    }

    #[throws]
    fn active_pane_mut_buffer_mut(&mut self) -> (&mut Pane, &mut Buffer) {
        let pane = self.pane_tree.active_mut();
        let buf = self.buffers.get_mut(pane.buffer_id()).ok_or_else(|| {
            anyhow!("internal error: active pane points to invalid buffer")
        })?;
        (pane, buf)
    }

    #[throws]
    fn delete_text(&mut self, boundary: Boundary, direction: Direction) {
        let (pane, buf) = self.active_pane_buffer_mut()?;
        let pos = buf.cursor(pane);
        let boundary = buf.find_boundary(pos, boundary, direction);
        if pos != boundary {
            let range = if pos < boundary {
                pos..boundary
            } else {
                boundary..pos
            };
            buf.delete_text(range);
        }
    }

    #[throws]
    fn insert_char(&mut self, key: gdk::keys::Key) {
        // Insert a character into the active pane.
        if let Some(c) = key.to_unicode() {
            let (pane, buf) = self.active_pane_buffer_mut()?;
            let pos = buf.cursor(pane);
            buf.insert_char(c, pos);
        }
    }

    #[throws]
    fn move_cursor(&mut self, step: Move, dir: Direction) {
        let line_height = self.line_height;
        let (pane, buf) = self.active_pane_mut_buffer_mut()?;
        let text = buf.text();
        let mut cursor = buf.cursor(pane);

        match step {
            Move::Boundary(boundary) => {
                cursor = buf.find_boundary(cursor, boundary, dir);
            }
            Move::Line | Move::Page => {
                let offset = if step == Move::Line { 1 } else { 20 };

                let mut lp = cursor.line_position(buf);

                // When moving between lines, use grapheme offset
                // rather than char offset to keep the cursor more or
                // less visually horizontally aligned. Probably would
                // need to be more sophisticated for non-monospace
                // fonts though.
                let num_graphemes = lp.grapheme_offset(buf);

                if dir == Direction::Dec {
                    lp.line = lp.line.saturating_sub(offset);
                } else {
                    lp.line =
                        std::cmp::min(lp.line + offset, text.len_lines() - 1);
                }
                lp.set_offset_in_graphemes(buf, num_graphemes);
                cursor = CharIndex::from_line_position(lp, buf);
            }
        }

        buf.set_cursor(pane, cursor);

        pane.maybe_rescroll(buf, cursor, line_height);
    }

    fn minibuf(&self) -> &Buffer {
        let id = self.pane_tree.minibuf().buffer_id();

        self.buffers.get(id).expect("missing minibuf buffer")
    }

    fn minibuf_mut(&mut self) -> &mut Buffer {
        let id = self.pane_tree.minibuf().buffer_id();

        self.buffers.get_mut(id).expect("missing minibuf buffer")
    }

    fn set_interactive_state(&mut self, state: InteractiveState) {
        let is_interactive = state != InteractiveState::Initial;
        self.interactive_state = state;
        self.pane_tree.set_minibuf_interactive(is_interactive);
        self.minibuf_mut().clear();
    }

    fn clear_interactive_state(&mut self) {
        self.set_interactive_state(InteractiveState::Initial);
    }

    fn display_error(&mut self, error: Error) {
        self.clear_interactive_state();
        // TODO: think about how this error will get unset. On next
        // key press, like emacs? Hide or fade after a timeout?
        self.minibuf_mut().set_text(&format!("{}", error));
    }

    #[throws]
    fn open_file(&mut self) {
        // Get the path to open.
        let text = self.minibuf().text().to_string();
        let path = Path::new(&text);

        // Reset the minibuf, which also reselects the previous active
        // pane.
        self.clear_interactive_state();

        // Load the file in a new buffer.
        let buf = Buffer::from_path(path, &self.theme)?;
        let buf_id = buf.id().clone();
        self.buffers.insert(buf_id.clone(), buf);
        self.pane_tree
            .active_mut()
            .switch_buffer(&mut self.buffers, &buf_id);
    }

    #[throws]
    fn handle_confirm(&mut self) {
        match self.interactive_state {
            InteractiveState::Initial => {}
            InteractiveState::OpenFile => {
                self.open_file()?;
            }
            InteractiveState::Search => {
                todo!();
            }
        }
    }

    #[throws]
    fn handle_buffer_changed(&mut self) {
        if self.interactive_state == InteractiveState::Search {
            let minibuf = self.minibuf();
            let search_for = minibuf.text().to_string();

            let line_height = self.line_height;

            let pane = self.pane_tree.active_excluding_minibuf();
            let buf = self
                .buffers
                .get_mut(pane.buffer_id())
                .ok_or_else(invalid_active_buffer_error)?;
            let num_lines =
                (pane.rect().height / line_height.0).round() as usize;
            buf.search(&search_for, pane, num_lines);
        }
    }

    #[throws]
    fn handle_action(&mut self, action: Action) {
        let buffer_changed;

        match action {
            Action::Exit => {
                self.window.close();
                buffer_changed = false;
            }
            Action::Insert(key) => {
                self.insert_char(key)?;
                buffer_changed = true;
            }
            Action::Move(step, dir) => {
                self.move_cursor(step, dir)?;
                buffer_changed = false;
            }
            Action::Delete(boundary, direction) => {
                self.delete_text(boundary, direction)?;
                buffer_changed = true;
            }
            Action::InteractiveSearch => {
                self.set_interactive_state(InteractiveState::Search);
                // TODO: prompt

                buffer_changed = false;
            }
            Action::Undo => {
                let buf = self.active_buffer_mut()?;
                buf.undo();
                buffer_changed = true;
            }
            Action::Redo => {
                let buf = self.active_buffer_mut()?;
                buf.redo();
                buffer_changed = true;
            }
            Action::SplitPane(orientation) => {
                let buf =
                    active_buffer_mut(&self.pane_tree, &mut self.buffers)?;
                self.pane_tree.split(orientation, buf);
                buffer_changed = false;
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
                buffer_changed = false;
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
                buffer_changed = false;
            }
            Action::OpenFile => {
                self.set_interactive_state(InteractiveState::OpenFile);
                // TODO: prompt

                buffer_changed = false;
            }
            Action::Confirm => {
                self.handle_confirm()?;
                buffer_changed = false;
            }
            Action::Cancel => {
                self.clear_interactive_state();
                buffer_changed = false;
            }
            todo => {
                buffer_changed = false;
                dbg!(todo);
            }
        }

        if buffer_changed {
            self.handle_buffer_changed()?;
        }
    }

    fn get_minibuf_keymap(&self) -> KeyMap {
        let mut map = KeyMap::new();

        let mut insert = |keys, action| {
            map.insert(KeySequence::parse(keys).unwrap(), action)
        };

        insert("<ctrl>i", Action::Autocomplete);
        insert("<ret>", Action::Confirm);
        insert("<ctrl>m", Action::Confirm);
        map
    }

    pub(super) fn handle_key_press(
        &mut self,
        key: gdk::keys::Key,
        state: gdk::ModifierType,
    ) -> Inhibit {
        let mut keymap_stack = KeyMapStack::default();
        keymap_stack.push(self.key_handler.base_keymap.clone());

        // TODO: figure these customizations out better
        if self.interactive_state != InteractiveState::Initial {
            keymap_stack.push(self.get_minibuf_keymap());
        }

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
            KeyMapLookup::Action(action) => {
                if let Err(err) = self.handle_action(action) {
                    self.display_error(err);
                }
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
