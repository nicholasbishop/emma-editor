use crate::action::{Action, Boundary, Direction, Move};
use crate::buffer::{Buffer, BufferId, LinePosition};
use crate::command_line_widget::CommandLineWidget;
use crate::key::{Key, Modifiers};
use crate::key_map::{KeyMap, KeyMapLookup, KeyMapStack};
use crate::key_sequence::{KeySequence, KeySequenceAtom};
use crate::message::{Message, MessageWriter};
use crate::overlay::Overlay;
use crate::pane_tree::{Pane, PaneTree};
use crate::path_chooser::PathChooser;
use crate::search_widget::SearchWidget;
use crate::state::AppState;
use crate::widget::Widget;
use anyhow::{Context, Error, Result, anyhow};
use fs_err as fs;
use std::collections::HashMap;
use std::path::Path;
use tracing::{error, info, instrument};

pub(super) struct KeyHandler {
    base_keymap: KeyMap,
    cur_seq: KeySequence,
}

impl KeyHandler {
    pub(super) fn new() -> Result<Self> {
        Ok(Self {
            base_keymap: KeyMap::base()?,
            cur_seq: KeySequence::default(),
        })
    }
}

fn invalid_active_buffer_error() -> Error {
    anyhow!("internal error: active pane points to invalid buffer")
}

fn active_buffer_mut<'b>(
    pane_tree: &PaneTree,
    buffers: &'b mut HashMap<BufferId, Buffer>,
) -> Result<&'b mut Buffer> {
    let pane = pane_tree.active();
    buffers
        .get_mut(pane.buffer_id())
        .ok_or_else(invalid_active_buffer_error)
}

impl AppState {
    fn active_buffer(&self) -> Result<&Buffer> {
        if let Some(overlay) = &self.overlay {
            return Ok(overlay.buffer());
        }

        let pane = self.pane_tree.active();
        let buf = self
            .buffers
            .get(pane.buffer_id())
            .ok_or_else(invalid_active_buffer_error)?;
        Ok(buf)
    }

    fn active_buffer_mut(&mut self) -> Result<&mut Buffer> {
        if let Some(overlay) = &mut self.overlay {
            return Ok(overlay.buffer_mut());
        }

        let pane = self.pane_tree.active();
        let buf = self
            .buffers
            .get_mut(pane.buffer_id())
            .ok_or_else(invalid_active_buffer_error)?;
        Ok(buf)
    }

    fn active_pane_buffer_mut(&mut self) -> Result<(&Pane, &mut Buffer)> {
        if let Some(overlay) = &mut self.overlay {
            return Ok(overlay.pane_buffer_mut());
        }

        let pane = self.pane_tree.active();
        let buf = self
            .buffers
            .get_mut(pane.buffer_id())
            .ok_or_else(invalid_active_buffer_error)?;
        Ok((pane, buf))
    }

    fn active_pane_mut_buffer_mut(
        &mut self,
    ) -> Result<(&mut Pane, &mut Buffer)> {
        if let Some(overlay) = &mut self.overlay {
            return Ok(overlay.pane_mut_buffer_mut());
        }

        let pane = self.pane_tree.active_mut();
        let buf = self.buffers.get_mut(pane.buffer_id()).ok_or_else(|| {
            anyhow!("internal error: active pane points to invalid buffer")
        })?;
        Ok((pane, buf))
    }

    fn delete_text(
        &mut self,
        boundary: Boundary,
        direction: Direction,
    ) -> Result<()> {
        let (pane, buf) = self.active_pane_buffer_mut()?;
        let pos = buf.cursor(pane.id());
        let boundary = buf.find_boundary(pos, boundary, direction);
        if pos != boundary {
            let range = if pos < boundary {
                pos..boundary
            } else {
                boundary..pos
            };
            buf.delete_text(range);
        }
        Ok(())
    }

    /// Insert a character into the active pane.
    fn insert_char(&mut self, c: char) -> Result<()> {
        let (pane, buf) = self.active_pane_buffer_mut()?;
        let pos = buf.cursor(pane.id());
        buf.insert_char(c, pos);
        Ok(())
    }

    fn move_cursor(&mut self, step: Move, dir: Direction) -> Result<()> {
        let line_height = self.line_height;
        let (pane, buf) = self.active_pane_mut_buffer_mut()?;

        buf.move_cursor(pane.id(), step, dir);

        let cursor = buf.cursor(pane.id());
        pane.maybe_rescroll(buf, cursor, line_height);

        Ok(())
    }

    /// Display an error message.
    fn display_error(&mut self, error: Error) {
        // TODO: think about how to display this in the UI.
        println!("error: {error}");
    }

    fn open_file_at_path(&mut self, path: &Path) -> Result<()> {
        // Load the file in a new buffer.
        let buf = Buffer::from_path(path)?;
        let buf_id = buf.id().clone();
        self.buffers.insert(buf_id.clone(), buf);
        self.pane_tree
            .active_mut()
            .switch_buffer(&mut self.buffers, &buf_id);
        Ok(())
    }

    fn handle_confirm(&mut self, message_writer: &MessageWriter) -> Result<()> {
        match &self.overlay {
            Some(Overlay::OpenFile(open_file)) => {
                let path = open_file.path();
                self.overlay = None;
                self.open_file_at_path(&path)?;
            }
            Some(Overlay::RunProcess(command_line_widget)) => {
                let mut buf = Buffer::create_for_non_interactive_process();
                let buf_id = buf.id().clone();
                buf.run_non_interactive_process(
                    command_line_widget.command_line(),
                    message_writer,
                )?;

                self.buffers.insert(buf_id.clone(), buf);
                self.pane_tree
                    .active_mut()
                    .switch_buffer(&mut self.buffers, &buf_id);

                self.overlay = None;
            }
            Some(Overlay::Search(_)) => {
                self.overlay = None;

                self.search_next()?;
                let pane = self.pane_tree.active();
                let buf = self
                    .buffers
                    .get_mut(pane.buffer_id())
                    .ok_or_else(invalid_active_buffer_error)?;

                buf.clear_search();
            }
            None => {}
        }

        Ok(())
    }

    #[instrument(skip(self))]
    fn handle_buffer_changed(&mut self) -> Result<()> {
        match &mut self.overlay {
            Some(Overlay::OpenFile(open_file)) => {
                open_file.update_suggestions()?;
            }
            Some(Overlay::Search(search)) => {
                let line_height = self.line_height;

                let pane = self.pane_tree.active();
                let buf = self
                    .buffers
                    .get_mut(pane.buffer_id())
                    .ok_or_else(invalid_active_buffer_error)?;
                let num_lines =
                    (pane.rect().height / line_height.0).round() as usize;
                buf.search(&search.text(), pane, num_lines);
            }
            Some(Overlay::RunProcess(_)) | None => {}
        }

        Ok(())
    }

    fn search_next(&mut self) -> Result<()> {
        let pane = self.pane_tree.active();
        let buf = self
            .buffers
            .get_mut(pane.buffer_id())
            .ok_or_else(invalid_active_buffer_error)?;
        let pos = buf.cursor(pane.id());
        let line_pos = LinePosition::from_abs_char(pos, buf);

        // Find the next match and move the cursor there.
        if let Some(search) = buf.search_state()
            && let Some(m) = search.next_match(line_pos)
        {
            let ci = m.to_abs_char(buf);
            buf.set_cursor(pane.id(), ci);
        }

        Ok(())
    }

    pub fn handle_action(
        &mut self,
        action: Action,
        message_writer: &MessageWriter,
    ) -> Result<()> {
        info!("handling action {:?}", action);

        let buffer_changed;

        match action {
            Action::Exit => {
                message_writer.send(Message::Close)?;
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
                self.overlay = Some(Overlay::Search(SearchWidget::new()));
                buffer_changed = false;
            }
            Action::SearchNext => {
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
                self.pane_tree.make_previous_pane_active();
                buffer_changed = false;
            }
            Action::NextPane => {
                self.pane_tree.make_next_pane_active();
                buffer_changed = false;
            }
            Action::DeleteBuffer => {
                let active_buffer_id = self.active_buffer()?.id().clone();

                // TODO: ensure there's at least one other buffer to switch to.
                // TODO: if multiple panes are pointed at the buffer,
                // switch each of them to a different buffer.
                // For now, just pick some other buffer.
                let new_buffer_id = self
                    .buffers
                    .keys()
                    .find(|b| **b != active_buffer_id)
                    .unwrap()
                    .clone();

                // Switch any pane pointed to the buffer to something else.
                for pane in self.pane_tree.panes_mut() {
                    if *pane.buffer_id() == active_buffer_id {
                        pane.switch_buffer(&mut self.buffers, &new_buffer_id);
                    }
                }

                // Delete the buffer.
                self.buffers.retain(|b, _| *b != active_buffer_id);

                buffer_changed = false;
            }
            Action::OpenFile => {
                let buf = self.active_buffer()?;
                // TODO: actually should have a buf.directory() method,
                // since in the future a dir buffer might display a
                // directory rather than a file. Or a shell buffer.
                let default_path = buf
                    .path()
                    .and_then(|p| p.parent())
                    .map(|p| p.to_owned())
                    .unwrap_or_else(|| {
                        std::env::current_dir().unwrap_or_default()
                    });

                self.overlay =
                    Some(Overlay::OpenFile(PathChooser::new(&default_path)?));

                buffer_changed = false;
            }
            Action::SaveFile => {
                let buf = self.active_buffer_mut()?;
                if let Some(path) = buf.path() {
                    fs::write(path, buf.text().to_string())?;
                } else {
                    todo!("attempted to save a buffer with no path");
                }
                buffer_changed = false;
            }
            Action::Confirm => {
                self.handle_confirm(message_writer)?;
                buffer_changed = false;
            }
            Action::Cancel => {
                self.overlay = None;
                // TODO: clear search highlight
                buffer_changed = false;
            }
            Action::Autocomplete => {
                if let Some(Overlay::OpenFile(open_file)) = &mut self.overlay {
                    open_file.autocomplete()?;
                }
                buffer_changed = true;
            }
            Action::RunNonInteractiveProcess => {
                self.overlay =
                    Some(Overlay::RunProcess(CommandLineWidget::new()));
                buffer_changed = false;
            }
            Action::ProcessFinished(buf_id) => {
                let buf = self
                    .buffers
                    .get_mut(&buf_id)
                    .context(format!("invalid buffer: {buf_id}"))?;

                buf.set_non_interactive_process_finished()?;

                buffer_changed = false;
            }
            Action::AppendToBuffer(buf_id, content) => {
                let buf = self
                    .buffers
                    .get_mut(&buf_id)
                    .context("invalid buffer: {buf_id}")?;

                // TODO: add a way to insert text directly.
                let mut s = buf.text().to_string();
                s.push_str(&content);
                buf.set_text(&s);

                buffer_changed = true;
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

        Ok(())
    }

    pub fn handle_key_press(
        &mut self,
        key: Key,
        modifiers: Modifiers,
        message_writer: &MessageWriter,
    ) {
        let mut keymap_stack = KeyMapStack::default();
        keymap_stack.push(Ok(self.key_handler.base_keymap.clone()));

        if let Some(overlay) = &self.overlay {
            keymap_stack.push(overlay.get_keymap());
        }

        // Ignore lone modifier presses.
        if key.is_modifier() {
            return;
        }

        // TODO: we want to ignore combo modifier presses too if no
        // non-modifier key is selected, e.g. pressing alt and then
        // shift, but currently that is treated as a valid
        // sequence. Need to figure out how to prevent that.

        let atom = KeySequenceAtom::from_event(key, modifiers);
        self.key_handler.cur_seq.0.push(atom);

        let mut clear_seq = true;
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
                if let Err(err) = self.handle_action(action, message_writer) {
                    error!("failed to handle action: {err}");
                    self.display_error(err);
                }
            }
        }

        if clear_seq {
            self.key_handler.cur_seq.0.clear();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::message::create_message_pipe;

    // TODO: simplify AppState::load, then maybe won't need this anymore.
    pub(crate) fn create_empty_app_state() -> AppState {
        AppState::load(&[], Err(anyhow!("")))
    }

    // TODO: experimenting with gtk test.
    #[test]
    fn test_app_state() {
        let app_state = create_empty_app_state();

        let panes = app_state.pane_tree.panes();
        assert_eq!(panes.len(), 1);
        assert_eq!(app_state.pane_tree.active().id(), panes[0].id());

        // Scratch buffer.
        assert_eq!(app_state.buffers.len(), 1);
    }

    /// Test running a non-interactive process in a buffer.
    #[test]
    fn test_non_interactive_process() -> Result<()> {
        let mut state = create_empty_app_state();

        let (mut reader, writer) = create_message_pipe()?;

        state.handle_action(Action::RunNonInteractiveProcess, &writer)?;

        // Type in the program.
        for c in "echo hello world!".chars() {
            state.handle_action(Action::Insert(c), &writer)?;
        }
        state.handle_action(Action::Confirm, &writer)?;

        let buf_id = {
            let buf = state
                .buffers
                .values()
                .find(|b| b.non_interactive_process().is_some())
                .unwrap();
            buf.id().clone()
        };

        assert_eq!(state.buffers[&buf_id].text().to_string(), "");

        // Run the event loop up to and including `ProcessFinished`.
        let mut running = true;
        while running {
            let Message::Action(action) = reader.read()? else {
                panic!();
            };
            if matches!(action, Action::ProcessFinished(_)) {
                running = false;
            }
            state.handle_action(action, &writer)?;
        }

        // Verify the final buffer text.
        assert_eq!(state.buffers[&buf_id].text().to_string(), "hello world!\n");

        Ok(())
    }

    // TODO: experimental test.
    #[test]
    fn test_file_open() -> Result<()> {
        let (_reader, writer) = create_message_pipe()?;

        let mut state = create_empty_app_state();

        // Create test files.
        let tmp_dir = tempfile::tempdir()?;
        let tmp_dir = tmp_dir.path();
        let tmp_path1 = tmp_dir.join("testfile1");
        fs::write(&tmp_path1, "test data 1\n")?;
        let tmp_path2 = tmp_dir.join("testfile2");
        fs::write(&tmp_path2, "test data 2\n")?;

        // Open the test file non-interactively.
        state.open_file_at_path(&tmp_path1)?;

        // Test interactive open.
        state.handle_action(Action::OpenFile, &writer)?;

        // Type in the path.
        for c in "/testfile2".chars() {
            state.handle_action(Action::Insert(c), &writer)?;
        }
        state.handle_action(Action::Confirm, &writer)?;

        Ok(())
    }
}
