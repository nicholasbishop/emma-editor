use super::{App, InteractiveState, APP};
use crate::buffer::{
    Boundary, Buffer, BufferId, Direction, LinePosition, RelLine,
};
use crate::key_map::{Action, KeyMap, KeyMapLookup, KeyMapStack, Move};
use crate::key_sequence::{is_modifier, KeySequence, KeySequenceAtom};
use crate::pane_tree::{Pane, PaneTree};
use crate::rope::AbsChar;
use anyhow::{anyhow, bail, Error};
use fehler::throws;
use fs_err as fs;
use gtk4::glib::signal::Inhibit;
use gtk4::prelude::*;
use gtk4::{self as gtk, gdk};
use std::collections::HashMap;
use std::path::Path;
use tracing::{error, info, instrument};

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
    #[throws]
    pub(super) fn new() -> Self {
        Self {
            base_keymap: KeyMap::base()?,
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
    fn insert_char(&mut self, key: gdk::Key) {
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
                let offset =
                    RelLine::new(if step == Move::Line { 1 } else { 20 });

                let mut lp = LinePosition::from_abs_char(cursor, buf);

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
                        std::cmp::min(lp.line + offset, text.max_line_index());
                }
                lp.set_offset_in_graphemes(buf, num_graphemes);
                cursor = lp.to_abs_char(buf);
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
        let prompt = match state {
            InteractiveState::OpenFile => Some("Open file: "),
            InteractiveState::Search => Some("Search: "),
            InteractiveState::Initial => None,
        };
        if let Some(prompt) = prompt {
            let minibuf_pane = self.pane_tree.minibuf();
            let minibuf = self
                .buffers
                .get_mut(minibuf_pane.buffer_id())
                .expect("missing minibuf buffer");
            minibuf.set_text(prompt);
            minibuf.set_cursor(minibuf_pane, AbsChar(prompt.len()));
            minibuf.set_marker("prompt_end", AbsChar(prompt.len()));
        }
    }

    fn clear_interactive_state(&mut self) {
        self.set_interactive_state(InteractiveState::Initial);
    }

    /// Display an error message in the minibuf.
    fn display_error(&mut self, error: Error) {
        self.clear_interactive_state();
        // TODO: think about how this error will get unset. On next
        // key press, like emacs? Hide or fade after a timeout?
        self.minibuf_mut().set_text(&format!("{}", error));
    }

    /// Display an informational message in the minibuf.
    fn display_message(&mut self, msg: &str) {
        self.clear_interactive_state();
        // TODO: think about how this error will get unset. On next
        // key press, like emacs? Hide or fade after a timeout?
        self.minibuf_mut().set_text(msg);
    }

    #[throws]
    fn open_file(&mut self) {
        // Get the path to open.
        let minibuf = self.minibuf();
        let text = minibuf
            .text()
            .slice(minibuf.get_marker("prompt_end").unwrap()..)
            .to_string();
        let path = Path::new(&text);

        // Reset the minibuf, which also reselects the previous active
        // pane.
        self.clear_interactive_state();

        // Load the file in a new buffer.
        let buf = Buffer::from_path(path)?;
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
                self.search_next()?;

                let pane = self.pane_tree.active_excluding_minibuf();
                let buf = self
                    .buffers
                    .get_mut(pane.buffer_id())
                    .ok_or_else(invalid_active_buffer_error)?;

                buf.clear_search();
                self.clear_interactive_state();
            }
        }
    }

    #[instrument(skip(self))]
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
    fn search_next(&mut self) {
        let pane = self.pane_tree.active_excluding_minibuf();
        let buf = self
            .buffers
            .get_mut(pane.buffer_id())
            .ok_or_else(invalid_active_buffer_error)?;
        let pos = buf.cursor(pane);
        let line_pos = LinePosition::from_abs_char(pos, buf);

        // Find the next match and move the cursor there.
        if let Some(search) = buf.search_state() {
            if let Some(m) = search.next_match(line_pos) {
                let ci = m.to_abs_char(buf);
                buf.set_cursor(pane, ci);
            }
        }
    }

    #[throws]
    fn handle_action(&mut self, action: Action) {
        info!("handling action {:?}", action);

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
            Action::SearchNext => {
                if self.interactive_state != InteractiveState::Search {
                    bail!("not searching");
                }

                self.search_next()?;
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
            Action::SaveFile => {
                let buf = self.active_buffer_mut()?;
                if let Some(path) = buf.path() {
                    fs::write(path, buf.text().to_string())?;
                    let msg = format!("Saved {}", path.display());
                    self.display_message(&msg);
                } else {
                    todo!("attempted to save a buffer with no path");
                }
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

        if let Err(err) = self.persistence_store() {
            error!("failed to persist state: {err}");
        }
    }

    #[throws]
    fn get_minibuf_keymap(&self) -> KeyMap {
        KeyMap::from_pairs(
            "minibuf",
            vec![
                ("<ctrl>i", Action::Autocomplete),
                ("<ret>", Action::Confirm),
                ("<ctrl>m", Action::Confirm),
            ]
            .into_iter(),
        )?
    }

    #[throws]
    fn get_search_keymap(&self) -> KeyMap {
        KeyMap::from_pairs(
            "search",
            vec![("<ctrl>s", Action::SearchNext)].into_iter(),
        )?
    }

    pub(super) fn handle_key_press(
        &mut self,
        key: gdk::Key,
        state: gdk::ModifierType,
    ) -> Inhibit {
        let mut keymap_stack = KeyMapStack::default();
        keymap_stack.push(Ok(self.key_handler.base_keymap.clone()));

        // TODO: figure these customizations out better
        if self.interactive_state != InteractiveState::Initial {
            keymap_stack.push(self.get_minibuf_keymap());
            if self.interactive_state == InteractiveState::Search {
                keymap_stack.push(self.get_search_keymap());
            }
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
                    error!("failed to handle action: {err}");
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
