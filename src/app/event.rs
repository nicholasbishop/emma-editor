use crate::app::{AppState, InteractiveState};
use crate::buffer::{Boundary, Buffer, BufferId, Direction, LinePosition};
use crate::key_map::{Action, KeyMap, KeyMapLookup, KeyMapStack, Move};
use crate::key_sequence::{KeySequence, KeySequenceAtom, is_modifier};
use crate::overlay::Overlay;
use crate::pane_tree::{Pane, PaneTree};
use crate::path_chooser::PathChooser;
use crate::search_widget::SearchWidget;
use crate::widget::Widget;
use anyhow::{Error, Result, anyhow, bail};
use fs_err as fs;
use glib::{ControlFlow, IOCondition};
use gtk4::glib::signal::Propagation;
use gtk4::prelude::*;
use gtk4::{self as gtk, gdk, glib};
use std::cell::RefCell;
use std::collections::HashMap;
use std::os::fd::AsRawFd;
use std::path::Path;
use std::rc::Rc;
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

        buf.move_cursor(pane.id(), step, dir)?;

        let cursor = buf.cursor(pane.id());
        pane.maybe_rescroll(buf, cursor, line_height);

        Ok(())
    }

    fn minibuf_mut(&mut self) -> &mut Buffer {
        let id = self.pane_tree.minibuf().buffer_id();

        self.buffers.get_mut(id).expect("missing minibuf buffer")
    }

    /// Display an error message in the minibuf.
    fn display_error(&mut self, error: Error) {
        // TODO: think about how this error will get unset. On next
        // key press, like emacs? Hide or fade after a timeout?
        self.minibuf_mut().set_text(&format!("{error}"));
    }

    /// Display an informational message in the minibuf.
    fn display_message(&mut self, msg: &str) {
        // TODO: think about how this error will get unset. On next
        // key press, like emacs? Hide or fade after a timeout?
        self.minibuf_mut().set_text(msg);
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

    fn handle_confirm(&mut self) -> Result<()> {
        match &self.overlay {
            Some(Overlay::OpenFile(open_file)) => {
                let path = open_file.path();
                self.overlay = None;
                self.open_file_at_path(&path)?;
                return Ok(());
            }
            Some(Overlay::Search(_)) => {
                self.overlay = None;

                self.search_next()?;
                let pane = self.pane_tree.active_excluding_minibuf();
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

                let pane = self.pane_tree.active_excluding_minibuf();
                let buf = self
                    .buffers
                    .get_mut(pane.buffer_id())
                    .ok_or_else(invalid_active_buffer_error)?;
                let num_lines =
                    (pane.rect().height / line_height.0).round() as usize;
                buf.search(&search.text(), pane, num_lines);
            }
            None => {}
        }

        Ok(())
    }

    fn search_next(&mut self) -> Result<()> {
        let pane = self.pane_tree.active_excluding_minibuf();
        let buf = self
            .buffers
            .get_mut(pane.buffer_id())
            .ok_or_else(invalid_active_buffer_error)?;
        let pos = buf.cursor(pane.id());
        let line_pos = LinePosition::from_abs_char(pos, buf);

        // Find the next match and move the cursor there.
        if let Some(search) = buf.search_state() {
            if let Some(m) = search.next_match(line_pos) {
                let ci = m.to_abs_char(buf);
                buf.set_cursor(pane.id(), ci);
            }
        }

        Ok(())
    }

    fn handle_action(
        &mut self,
        // TODO: just optional for tests
        window: Option<gtk::ApplicationWindow>,
        action: Action,
        // TODO: ugly
        app_state: Rc<RefCell<Self>>,
    ) -> Result<()> {
        info!("handling action {:?}", action);

        let buffer_changed;

        match action {
            Action::Exit => {
                // TODO: unwrap
                window.unwrap().close();
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
                    .find(|b| **b != active_buffer_id && !b.is_minibuf())
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
                let mut buf = Buffer::create_for_non_interactive_process();
                let buf_id = buf.id().clone();
                buf.run_non_interactive_process()?;

                let proc = buf.non_interactive_process().unwrap();
                let _source_id = glib::source::unix_fd_add_local(
                    proc.output_fd().as_raw_fd(),
                    IOCondition::IN,
                    move |_raw_fd, _condition| {
                        // Read from the FD until we can't (with some
                        // kind of stopping point, in case the FD keeps
                        // returning a flood of data?)

                        let mut app_state = app_state.borrow_mut();
                        // Find the buffer by ID.
                        // TODO: unwrap
                        let buf = app_state.buffers.get_mut(&buf_id).unwrap();

                        let proc = buf.non_interactive_process_mut().unwrap();

                        let output = proc.read_output().unwrap();
                        if output.is_empty() {
                            // Process finished.
                            proc.wait();
                            return ControlFlow::Break;
                        }

                        // TODO: not great
                        let output = String::from_utf8(output).unwrap();

                        // TODO: add a way to insert text directly.
                        let mut s = buf.text().to_string();
                        s.push_str(&output);
                        buf.set_text(&s);

                        // Keep the callback.
                        ControlFlow::Continue
                    },
                );

                let buf_id = buf.id().clone();
                self.buffers.insert(buf_id.clone(), buf);
                self.pane_tree
                    .active_mut()
                    .switch_buffer(&mut self.buffers, &buf_id);

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

        Ok(())
    }

    fn get_minibuf_keymap(&self) -> Result<KeyMap> {
        KeyMap::from_pairs(
            "minibuf",
            vec![
                ("<ctrl>i", Action::Autocomplete),
                ("<ret>", Action::Confirm),
                ("<ctrl>m", Action::Confirm),
            ]
            .into_iter(),
        )
    }

    fn get_search_keymap(&self) -> Result<KeyMap> {
        KeyMap::from_pairs(
            "search",
            vec![("<ctrl>s", Action::SearchNext)].into_iter(),
        )
    }

    pub(super) fn handle_key_press(
        &mut self,
        window: gtk::ApplicationWindow,
        key: gdk::Key,
        state: gdk::ModifierType,
        // TODO: ugly
        app_state: Rc<RefCell<Self>>,
    ) -> Propagation {
        let mut keymap_stack = KeyMapStack::default();
        keymap_stack.push(Ok(self.key_handler.base_keymap.clone()));

        // TODO: figure these customizations out better
        if self.interactive_state != InteractiveState::Initial {
            keymap_stack.push(self.get_minibuf_keymap());
            if self.interactive_state == InteractiveState::Search {
                keymap_stack.push(self.get_search_keymap());
            }
        }

        if let Some(overlay) = &self.overlay {
            keymap_stack.push(overlay.get_keymap());
        }

        // Ignore lone modifier presses.
        if is_modifier(&key) {
            return Propagation::Proceed;
        }

        // TODO: we want to ignore combo modifier presses too if no
        // non-modifier key is selected, e.g. pressing alt and then
        // shift, but currently that is treated as a valid
        // sequence. Need to figure out how to prevent that.

        let atom = KeySequenceAtom::from_event(key, state);
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
                if let Err(err) =
                    self.handle_action(Some(window), action, app_state)
                {
                    error!("failed to handle action: {err}");
                    self.display_error(err);
                }
            }
        }

        if clear_seq {
            self.key_handler.cur_seq.0.clear();
        }

        Propagation::Stop
    }
}

#[cfg(test)]
pub mod tests {
    use super::*;

    /// Test running a non-interactive process in a buffer.
    #[gtk4::test]
    fn test_non_interactive_process() -> Result<()> {
        let app_state =
            Rc::new(RefCell::new(crate::app::tests::create_empty_app_state()));

        app_state.clone().borrow_mut().handle_action(
            None,
            Action::RunNonInteractiveProcess,
            app_state.clone(),
        )?;

        fn get_buf_text(app_state: Rc<RefCell<AppState>>) -> String {
            let state = app_state.borrow_mut();
            let buf = state
                .buffers
                .values()
                .find(|b| b.non_interactive_process().is_some())
                .unwrap();
            buf.text().to_string()
        }

        fn is_process_running(app_state: Rc<RefCell<AppState>>) -> bool {
            let state = app_state.borrow_mut();
            let buf = state
                .buffers
                .values()
                .find(|b| b.non_interactive_process().is_some())
                .unwrap();
            buf.non_interactive_process().unwrap().is_running()
        }

        assert_eq!(get_buf_text(app_state.clone()), "");
        assert!(is_process_running(app_state.clone()));

        loop {
            glib::MainContext::default().iteration(true);
            if !is_process_running(app_state.clone()) {
                break;
            }
        }

        assert_eq!(get_buf_text(app_state), "hello!\n");

        Ok(())
    }
}

#[cfg(any())] // TODO: reenable
#[cfg(test)]
pub mod tests {
    use super::*;
    use anyhow::Result;

    fn path_to_string(p: &Path) -> String {
        p.to_str().unwrap().to_owned()
    }

    // TODO: experimental test.
    #[test]
    fn test_file_open() -> Result<()> {
        let mut app_state = crate::app::tests::create_empty_app_state();

        let (pane, buf) = app_state.active_pane_mut_buffer_mut()?;

        let buf_id = buf.id().clone();
        let pane_id = pane.id().clone();
        assert!(!buf_id.is_minibuf());

        // Create test files.
        let tmp_dir = tempfile::tempdir()?;
        let tmp_dir = tmp_dir.path();
        let tmp_path1 = tmp_dir.join("testfile1");
        fs::write(&tmp_path1, "test data 1\n")?;
        let tmp_path2 = tmp_dir.join("testfile2");
        fs::write(&tmp_path2, "test data 2\n")?;

        // Open the test file non-interactively.
        app_state.open_file_at_path(&tmp_path1)?;

        // Test interactive open.
        app_state.handle_action(None, Action::PathChooser)?;
        assert!(*app_state.pane_tree.active().id() != pane_id);
        assert_eq!(
            app_state.minibuf().text().to_string(),
            format!("Open file: {}", path_to_string(tmp_dir))
        );
        assert_eq!(app_state.minibuf().cursors().len(), 1);

        // Check that the cursor can't move into the prompt.
        app_state.handle_action(
            None,
            Action::Move(Move::Boundary(Boundary::LineEnd), Direction::Dec),
        )?;
        assert_eq!(
            app_state.minibuf().cursors().values().next().unwrap().0,
            11
        );

        // Check the default path.
        assert_eq!(app_state.get_interactive_text()?, path_to_string(tmp_dir));

        // Type one character into the minibuf.
        app_state.handle_action(None, Action::Insert('/'))?;
        assert_eq!(
            app_state.get_interactive_text()?,
            path_to_string(tmp_dir) + "/"
        );

        // TODO: make it easier to just insert text.
        for c in "testfile2".chars() {
            app_state.handle_action(None, Action::Insert(c))?;
        }
        app_state.handle_action(None, Action::Confirm)?;

        app_state.handle_action(None, Action::Cancel)?;
        assert!(*app_state.pane_tree.active().id() == pane_id);

        Ok(())
    }
}
