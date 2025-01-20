use super::{AppState, InteractiveState};
use crate::buffer::{
    Boundary, Buffer, BufferId, Direction, LinePosition, RelLine,
};
use crate::key_map::{Action, KeyMap, KeyMapLookup, KeyMapStack, Move};
use crate::key_sequence::{is_modifier, KeySequence, KeySequenceAtom};
use crate::open_file::OpenFile;
use crate::pane_tree::{Pane, PaneTree};
use crate::rope::AbsChar;
use anyhow::{anyhow, bail, Context, Error, Result};
use fs_err as fs;
use gtk4::glib::signal::Propagation;
use gtk4::prelude::*;
use gtk4::{self as gtk, gdk};
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

const PROMPT_END: &str = "prompt_end";
const COMPLETION_START: &str = "completion_start";

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
        if let Some(open_file) = &self.open_file {
            return Ok(open_file.buffer());
        }

        let pane = self.pane_tree.active();
        let buf = self
            .buffers
            .get(pane.buffer_id())
            .ok_or_else(invalid_active_buffer_error)?;
        Ok(buf)
    }

    fn active_buffer_mut(&mut self) -> Result<&mut Buffer> {
        if let Some(open_file) = &mut self.open_file {
            return Ok(open_file.buffer_mut());
        }

        let pane = self.pane_tree.active();
        let buf = self
            .buffers
            .get_mut(pane.buffer_id())
            .ok_or_else(invalid_active_buffer_error)?;
        Ok(buf)
    }

    fn active_pane_buffer_mut(&mut self) -> Result<(&Pane, &mut Buffer)> {
        if let Some(open_file) = &mut self.open_file {
            return Ok(open_file.pane_buffer_mut());
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
        if let Some(open_file) = &mut self.open_file {
            return Ok(open_file.pane_mut_buffer_mut());
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
        Ok(())
    }

    /// Insert a character into the active pane.
    fn insert_char(&mut self, c: char) -> Result<()> {
        let (pane, buf) = self.active_pane_buffer_mut()?;
        let pos = buf.cursor(pane);
        buf.insert_char(c, pos);
        Ok(())
    }

    fn move_cursor(&mut self, step: Move, dir: Direction) -> Result<()> {
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

        // Prevent the cursor from going before the prompt end or after
        // the completion start.
        //
        // This is kinda hacky and too specific. In the future we'll
        // probably want a way to mark a region of text as untouchable (can't edit
        // or even move the cursor into it).
        if let Some(prompt_end) = buf.get_marker(PROMPT_END) {
            if cursor < prompt_end {
                cursor = prompt_end;
            }
        }
        if let Some(completion_start) = buf.get_marker(COMPLETION_START) {
            if cursor > completion_start {
                cursor = completion_start;
            }
        }

        buf.set_cursor(pane, cursor);

        pane.maybe_rescroll(buf, cursor, line_height);

        Ok(())
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
        self.interactive_state = state.clone();
        self.pane_tree.set_minibuf_interactive(is_interactive);
        self.minibuf_mut().clear();
        let (prompt, default) = match state {
            InteractiveState::OpenFile(default_path) => {
                // Convert the default path to a string.
                // TODO: what about non-utf8 paths?
                let default =
                    default_path.to_str().unwrap_or_default().to_owned();
                (Some("Open file: "), default)
            }
            InteractiveState::Search => (Some("Search: "), String::new()),
            InteractiveState::Initial => (None, String::new()),
        };
        if let Some(prompt) = prompt {
            let minibuf_pane = self.pane_tree.minibuf();
            let minibuf = self
                .buffers
                .get_mut(minibuf_pane.buffer_id())
                .expect("missing minibuf buffer");
            let text = format!("{prompt}{default}");
            minibuf.set_text(&text);
            minibuf.set_cursor(minibuf_pane, AbsChar(text.len()));
            minibuf.set_marker(PROMPT_END, AbsChar(prompt.len()));
            minibuf.set_marker(COMPLETION_START, AbsChar(text.len()));
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

    fn get_interactive_text(&mut self) -> Result<String> {
        if self.interactive_state == InteractiveState::Initial {
            bail!("minibuf not in interactive mode");
        }

        let minibuf = self.minibuf();
        let start = minibuf
            .get_marker(PROMPT_END)
            .context("missing prompt end")?;
        let end = minibuf
            .get_marker(COMPLETION_START)
            .context("missing completion start")?;
        let text = minibuf.text().slice(start..end).to_string();

        Ok(text)
    }

    fn open_file(&mut self) -> Result<()> {
        // Get the path to open.
        let text = self.get_interactive_text()?;
        let path = Path::new(&text);

        // Reset the minibuf, which also reselects the previous active
        // pane.
        self.clear_interactive_state();

        self.open_file_at_path(path)
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
        if let Some(open_file) = &mut self.open_file {
            let path = open_file.path();
            self.open_file = None;
            self.open_file_at_path(&path)?;
            return Ok(());
        }

        match self.interactive_state {
            InteractiveState::Initial => {}
            InteractiveState::OpenFile(_) => {
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
        Ok(())
    }

    #[instrument(skip(self))]
    fn handle_buffer_changed(&mut self) -> Result<()> {
        if let Some(open_file) = &mut self.open_file {
            open_file.update_suggestions()?;
        }

        match self.interactive_state {
            InteractiveState::Search => {
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
            InteractiveState::OpenFile(_) => {
                // TODO: this is a very simple completion that is
                // minimally helpful.
                let mut path = self.get_interactive_text()?;
                path.push('*');
                // Arbitrarily grab a few options.
                let completions: Vec<_> = glob::glob(&path)?
                    .into_iter()
                    .take(5)
                    .map(|p| p.unwrap().to_str().unwrap().to_owned())
                    .collect();

                let minibuf = self.minibuf_mut();
                let end = minibuf
                    .get_marker(COMPLETION_START)
                    .context("missing completion start")?;
                let text = minibuf.text().slice(..end).to_string();
                let text = format!("{}   {}", text, completions.join(" | "));

                // TODO: this probably interacts poorly with undo/redo.
                minibuf.set_text(&text);
            }
            _ => {}
        }
        Ok(())
    }

    fn search_next(&mut self) -> Result<()> {
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

        Ok(())
    }

    fn handle_action(
        &mut self,
        // TODO: just optional for tests
        window: Option<gtk::ApplicationWindow>,
        action: Action,
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
                self.set_interactive_state(InteractiveState::Search);
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

                self.open_file = Some(OpenFile::new(&default_path)?);

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
                self.open_file = None;
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

        if let Some(open_file) = &self.open_file {
            keymap_stack.push(open_file.get_keymap());
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
                if let Err(err) = self.handle_action(Some(window), action) {
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
        app_state.handle_action(None, Action::OpenFile)?;
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
