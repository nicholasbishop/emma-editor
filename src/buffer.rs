pub use crate::rope::{AbsChar, AbsLine, LinesIterItem, RelChar, RelLine};

use crate::grapheme::{next_grapheme_boundary, prev_grapheme_boundary};
use crate::pane_tree::{Pane, PaneId};
use crate::process::NonInteractiveProcess;
use crate::rope::{LineDataVec, Rope};
use crate::shell::Shell;
use crate::theme::Theme;
use crate::util;
use aho_corasick::AhoCorasick;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::{fmt, fs, io};
use syntect::highlighting::{
    HighlightState, Highlighter, RangedHighlightIterator, Style,
};
use syntect::parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet};

// TODO: move this into this file?
use crate::key_map::Move;

// TODO: not sure where we want these.
pub const PROMPT_END: &str = "prompt_end";
pub const COMPLETION_START: &str = "completion_start";

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Direction {
    Dec,
    Inc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Boundary {
    Grapheme,
    LineEnd,
    BufferEnd,
    // TODO:
    // Subword,
    // Word,
    // LineEndExcludingWhitespace,
}

#[derive(Debug, Clone, Eq, PartialEq, Hash, Deserialize, Serialize)]
pub struct BufferId(String);

impl BufferId {
    fn new() -> Self {
        Self(util::make_id("buffer"))
    }

    fn minibuf() -> Self {
        Self("buffer-minibuf".into())
    }

    pub fn is_minibuf(&self) -> bool {
        *self == Self::minibuf()
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }

    pub fn from_string(s: String) -> Self {
        Self(s)
    }
}

impl fmt::Display for BufferId {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.0)
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LinePosition {
    /// Line index.
    pub line: AbsLine,
    /// Character offset from the start of the line.
    pub offset: RelChar,
}

impl LinePosition {
    /// Convert the AbsChar to a LinePosition.
    pub fn from_abs_char(pos: AbsChar, buf: &Buffer) -> Self {
        let text = &buf.text();

        let line = text.char_to_line(pos);
        let line_offset = pos.0 - text.line_to_char(line);

        Self {
            line,
            offset: RelChar(line_offset),
        }
    }

    pub fn to_abs_char(self, buf: &Buffer) -> AbsChar {
        AbsChar(buf.text().line_to_char(self.line) + self.offset.0)
    }

    /// Count the number of graphemes between the start of the line
    /// and the line offset.
    pub fn grapheme_offset(&self, buf: &Buffer) -> usize {
        let line = buf.text().line(self.line);
        let mut num_graphemes = 0;
        let mut cur_offset = 0;
        while cur_offset < self.offset.0 {
            let new_offset = next_grapheme_boundary(&line, cur_offset);
            if cur_offset == new_offset.0 {
                break;
            } else {
                num_graphemes += 1;
                cur_offset = new_offset.0;
            }
        }
        num_graphemes
    }

    /// Set the offset to point after the specified number of
    /// graphemes. This is truncated to the end of the line in case
    /// there are fewer graphemes in the line than requested.
    pub fn set_offset_in_graphemes(
        &mut self,
        buf: &Buffer,
        mut num_graphemes: usize,
    ) {
        let line = buf.text().line(self.line);
        let num_chars = line.len_chars();
        self.offset = RelChar::zero();
        while num_graphemes > 0 {
            self.offset = next_grapheme_boundary(&line, self.offset.0);
            num_graphemes -= 1;
            if self.offset.0 >= num_chars {
                self.offset = RelChar(num_chars);
                break;
            }
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct StyleSpan {
    pub len: usize,
    pub style: Style,
}

/// Style for a contiguous group of chars, covers the whole line.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct StyledLine(pub Vec<StyleSpan>);

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActionType {
    None,
    Clear,
    InsertChar,
    Deletion,
}

pub type CursorMap = HashMap<PaneId, AbsChar>;

#[derive(Clone)]
struct HistoryItem {
    text: Rope,

    markers: HashMap<String, AbsChar>,

    // TODO: style_spans?

    // Each pane showing this buffer has its own cursor.
    cursors: CursorMap,
}

/// Matching spans within a line.
#[derive(Clone, Debug, Default)]
pub struct LineMatches {
    pub spans: Vec<Range<usize>>,
}

pub struct SearchState {
    pane_id: PaneId,
    matches: LineDataVec<LineMatches>,
}

impl SearchState {
    pub fn line_matches(
        &self,
        pane: &Pane,
        line_index: AbsLine,
    ) -> Option<&LineMatches> {
        if pane.id() != &self.pane_id {
            return None;
        }

        self.matches.get(line_index)
    }

    pub fn next_match(&self, line_pos: LinePosition) -> Option<LinePosition> {
        for lm in self.matches.starting_from(line_pos.line) {
            for span in &lm.data.spans {
                // Ignore matches on line_pos's line that are before
                // the char offset.
                if lm.index == line_pos.line && span.start < line_pos.offset.0 {
                    continue;
                }

                return Some(LinePosition {
                    line: lm.index,
                    offset: RelChar(span.start),
                });
            }
        }
        None
    }
}

pub struct Buffer {
    id: BufferId,

    path: Option<PathBuf>,

    history: Vec<HistoryItem>,
    active_history_index: usize,
    last_action_type: ActionType,

    // TODO: think about a smarter structure
    // TODO: put in arc for async update
    style_spans: LineDataVec<StyledLine>,

    search: Option<SearchState>,

    _shell: Option<Shell>,
    non_interactive_process: Option<NonInteractiveProcess>,
}

impl fmt::Debug for Buffer {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        // Might put additional fields in here like path, but
        // definitely want to exclude the text, them, and style_spans
        // fields.
        write!(f, "Buffer({})", self.id.0)
    }
}

impl Buffer {
    fn new(id: BufferId, text: Rope, path: Option<PathBuf>) -> Self {
        let mut buf = Self {
            id,
            history: vec![HistoryItem {
                text,
                markers: HashMap::new(),
                cursors: CursorMap::new(),
            }],
            active_history_index: 0,
            last_action_type: ActionType::None,
            path,
            style_spans: LineDataVec::new(AbsLine::zero()),
            search: None,
            _shell: None,
            non_interactive_process: None,
        };

        // TODO, async
        buf.recalc_style_spans();

        buf
    }

    /// Create an empty buffer with no associated path.
    pub fn create_empty() -> Self {
        Self::new(BufferId::new(), Rope::new(), None)
    }

    pub fn create_minibuf() -> Self {
        Self::new(BufferId::minibuf(), Rope::new(), None)
    }

    pub fn create_for_non_interactive_process() -> Self {
        // TODO: set path and process info.
        let mut buf = Self::new(BufferId::new(), Rope::new(), None);
        buf.non_interactive_process = Some(NonInteractiveProcess::new());
        buf
    }

    pub fn from_path(path: &Path) -> Result<Self> {
        let text =
            Rope::from_reader(&mut io::BufReader::new(fs::File::open(path)?))?;
        Ok(Self::new(BufferId::new(), text, Some(path.into())))
    }

    pub fn id(&self) -> &BufferId {
        &self.id
    }

    pub fn run_non_interactive_process(&mut self) -> Result<()> {
        let proc = self.non_interactive_process.as_mut().unwrap();
        proc.run()
    }

    pub fn non_interactive_process(&self) -> Option<&NonInteractiveProcess> {
        self.non_interactive_process.as_ref()
    }

    pub fn non_interactive_process_mut(
        &mut self,
    ) -> Option<&mut NonInteractiveProcess> {
        self.non_interactive_process.as_mut()
    }

    pub fn text(&self) -> &Rope {
        &self.history[self.active_history_index].text
    }

    /// Get a mutable reference to the rope. This is only valid if the
    /// active history item is the newest one -- editing earlier
    /// entries in the history stack is not allowed.
    pub fn text_mut(&mut self) -> Option<&mut Rope> {
        if self.active_history_index == self.history.len() - 1 {
            Some(&mut self.history[self.active_history_index].text)
        } else {
            None
        }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn style_spans(&self) -> &LineDataVec<StyledLine> {
        &self.style_spans
    }

    pub fn search_state(&self) -> &Option<SearchState> {
        &self.search
    }

    pub fn get_marker(&self, name: &str) -> Option<AbsChar> {
        self.active_history_item().markers.get(name).copied()
    }

    pub fn set_marker<S: Into<String>>(&mut self, name: S, pos: AbsChar) {
        self.active_history_item_mut()
            .markers
            .insert(name.into(), pos);
    }

    pub fn cursor(&self, pane_id: &PaneId) -> AbsChar {
        *self
            .active_history_item()
            .cursors
            .get(pane_id)
            .unwrap_or_else(|| panic!("no cursor for {pane_id}"))
    }

    pub fn move_cursor(
        &mut self,
        pane_id: &PaneId,
        step: Move,
        dir: Direction,
    ) -> Result<()> {
        let mut cursor = self.cursor(pane_id);

        match step {
            Move::Boundary(boundary) => {
                cursor = self.find_boundary(cursor, boundary, dir);
            }
            Move::Line | Move::Page => {
                let offset =
                    RelLine::new(if step == Move::Line { 1 } else { 20 });

                let mut lp = LinePosition::from_abs_char(cursor, self);

                // When moving between lines, use grapheme offset
                // rather than char offset to keep the cursor more or
                // less visually horizontally aligned. Probably would
                // need to be more sophisticated for non-monospace
                // fonts though.
                let num_graphemes = lp.grapheme_offset(self);

                if dir == Direction::Dec {
                    lp.line = lp.line.saturating_sub(offset);
                } else {
                    lp.line = std::cmp::min(
                        lp.line + offset,
                        self.text().max_line_index(),
                    );
                }
                lp.set_offset_in_graphemes(self, num_graphemes);
                cursor = lp.to_abs_char(self);
            }
        }

        // Prevent the cursor from going before the prompt end or after
        // the completion start.
        //
        // This is kinda hacky and too specific. In the future we'll
        // probably want a way to mark a region of text as untouchable (can't edit
        // or even move the cursor into it).
        if let Some(prompt_end) = self.get_marker(PROMPT_END) {
            if cursor < prompt_end {
                cursor = prompt_end;
            }
        }
        if let Some(completion_start) = self.get_marker(COMPLETION_START) {
            if cursor > completion_start {
                cursor = completion_start;
            }
        }

        self.set_cursor(pane_id, cursor);

        Ok(())
    }

    pub fn set_cursor(&mut self, pane_id: &PaneId, cursor: AbsChar) {
        // This isn't an undoable action, but should prevent history
        // (e.g. press 'a', move cursor, press 'b' should be two
        // history items, not one).
        self.last_action_type = ActionType::None;

        self.cursors_mut().insert(pane_id.clone(), cursor);

        // TODO: set_cursor is used for two cases: moving a cursor and
        // adding a new cursor to represent a new pane showing the
        // buffer. Need to think about handling the second case across
        // history items better.
    }

    pub fn remove_cursor(&mut self, pane: &Pane) {
        // Remove the cursor from all history items.
        for item in &mut self.history {
            item.cursors.remove(pane.id());
        }
    }

    pub fn cursors(&self) -> &CursorMap {
        &self.active_history_item().cursors
    }

    /// Remove all text from the buffer.
    pub fn clear(&mut self) {
        self.maybe_store_history_item(ActionType::Clear);

        *self.text_mut().unwrap() = Rope::new();

        // TODO: async style recalc
        self.recalc_style_spans();

        // Update all cursors.
        for cursor in self.cursors_mut().values_mut() {
            cursor.0 = 0;
        }
    }

    fn active_history_item(&self) -> &HistoryItem {
        &self.history[self.active_history_index]
    }

    fn active_history_item_mut(&mut self) -> &mut HistoryItem {
        &mut self.history[self.active_history_index]
    }

    fn cursors_mut(&mut self) -> &mut CursorMap {
        &mut self.history[self.active_history_index].cursors
    }

    fn maybe_store_history_item(&mut self, action_type: ActionType) {
        // Check if the active history item is not most recent history
        // item. That means the user has run undo one or more times,
        // and is now making edits.
        if self.active_history_index != self.history.len() - 1 {
            // Chop off all newer history items.
            self.history.truncate(self.active_history_index + 1);

            // Reset the last_action_type; whatever action is occuring
            // now should not be merged into the top history item.
            self.last_action_type = ActionType::None;
        }

        // If the action type is unchanged then we don't store a new
        // item. The idea here is that if a number of keys are typed
        // to insert characters we don't want to individually undo
        // each one -- they should be grouped together. Same goes for
        // most other edit actions such as deleting characters.
        //
        // ActionType::None is special -- this never merges into the
        // existing history item.
        //
        // TODO: we'll probably need to make this a bit smarter. For
        // example, if the user types a whole paragraph it shouldn't
        // be a single undo entry. Maybe it should limit it by time or
        // by length of typed text.
        if self.last_action_type != action_type
            || action_type == ActionType::None
        {
            self.history.push(self.history.last().unwrap().clone());
            self.active_history_index = self.history.len() - 1;
            self.last_action_type = action_type;
        }
    }

    pub fn undo(&mut self) {
        if self.active_history_index > 0 {
            self.active_history_index -= 1;
        }

        // TODO: async style recalc
        self.recalc_style_spans();
    }

    pub fn redo(&mut self) {
        if self.active_history_index + 1 < self.history.len() {
            self.active_history_index += 1;
        }

        // TODO: async style recalc
        self.recalc_style_spans();
    }

    pub fn find_boundary(
        &mut self,
        pos: AbsChar,
        boundary: Boundary,
        direction: Direction,
    ) -> AbsChar {
        let text = self.text();
        match (boundary, direction) {
            (Boundary::Grapheme, Direction::Dec) => {
                AbsChar(prev_grapheme_boundary(&text.slice(..), pos.0).0)
            }
            (Boundary::Grapheme, Direction::Inc) => {
                AbsChar(next_grapheme_boundary(&text.slice(..), pos.0).0)
            }
            (Boundary::LineEnd, direction) => {
                let mut lp = LinePosition::from_abs_char(pos, self);
                if direction == Direction::Dec {
                    // TODO: add logic to initially move to
                    // first-non-whitespace char.
                    lp.offset = RelChar::zero();
                } else {
                    let line = text.line(lp.line);
                    // The last line in a buffer may or may not end in a
                    // newline character; this will affect the desired
                    // offset of the cursor.
                    //
                    // TODO: to_string is overkill.
                    let offset = if line.to_string().ends_with('\n') {
                        1
                    } else {
                        0
                    };
                    lp.offset = RelChar(line.len_chars() - offset);
                }
                lp.to_abs_char(self)
            }
            (Boundary::BufferEnd, Direction::Dec) => AbsChar(0),
            (Boundary::BufferEnd, Direction::Inc) => AbsChar(text.len_chars()),
        }
    }

    pub fn delete_text(&mut self, range: Range<AbsChar>) {
        self.maybe_store_history_item(ActionType::Deletion);

        self.text_mut().unwrap().remove(range.clone());

        // Update all cursors in this buffer.
        for cursor in self.cursors_mut().values_mut() {
            if range.contains(cursor) {
                *cursor = range.start;
            } else if *cursor >= range.end {
                // TODO any way to impl len?
                cursor.0 -= range.end.0 - range.start.0;
            }
        }

        // TODO: async style recalc
        self.recalc_style_spans();
    }

    pub fn insert_char(&mut self, c: char, pos: AbsChar) {
        self.maybe_store_history_item(ActionType::InsertChar);

        self.text_mut().unwrap().insert(pos, &c.to_string());

        // Update the associated style span to account for the new
        // character.
        let lp = LinePosition::from_abs_char(pos, self);
        if let Some(spans) = self.style_spans.get_mut(lp.line) {
            let offset = 0;
            for span in &mut spans.0 {
                if lp.offset.0 >= offset && lp.offset.0 < offset + span.len {
                    span.len += 1;
                    break;
                }
            }
        }

        // TODO: async style recalc
        self.recalc_style_spans();

        // Update all cursors in this buffer.
        for cursor in self.cursors_mut().values_mut() {
            if cursor.0 >= pos.0 {
                cursor.0 += 1;
            }
        }
    }

    /// Replace the entire contents of the buffer with `text`.
    pub fn set_text(&mut self, text: &str) {
        self.maybe_store_history_item(ActionType::None);

        *self.text_mut().unwrap() = Rope::from_str(text);

        // TODO: async style recalc
        self.recalc_style_spans();

        // Update all cursors in this buffer.
        let len_chars = self.text().len_chars();
        for cursor in self.cursors_mut().values_mut() {
            if cursor.0 > len_chars {
                cursor.0 = len_chars;
            }
        }
    }

    pub fn search(&mut self, text: &str, pane: &Pane, num_lines: usize) {
        if text.is_empty() {
            return;
        }

        let mut state = SearchState {
            pane_id: pane.id().clone(),
            matches: LineDataVec::with_size(pane.top_line(), num_lines),
        };

        // TODO: unwrap
        let ac = AhoCorasick::new([text]).unwrap();
        for line in self.text().lines_at(state.matches.start_line()) {
            let lm = if let Some(lm) = state.matches.get_mut(line.index) {
                lm
            } else {
                break;
            };

            let line_str = line.slice.to_string();
            for m in ac.find_iter(&line_str) {
                lm.spans.push(m.start()..m.end());
            }
        }

        self.search = Some(state);
    }

    pub fn clear_search(&mut self) {
        self.search = None;
    }

    fn get_syntax<'a>(&self, syntax_set: &'a SyntaxSet) -> &'a SyntaxReference {
        if let Some(path) = &self.path {
            if let Ok(Some(syntax)) = syntax_set.find_syntax_for_file(path) {
                return syntax;
            }
        }

        // Fall back to plain text.
        syntax_set
            .find_syntax_by_name("Plain Text")
            .expect("missing plain text syntax")
    }

    // TODO: simple for now
    fn recalc_style_spans(&mut self) {
        self.style_spans.clear();

        // TODO: cache
        let syntax_set = SyntaxSet::load_defaults_newlines();

        let syntax = self.get_syntax(&syntax_set);

        let mut parse_state = ParseState::new(syntax);
        let theme = Theme::current();
        let highlighter = Highlighter::new(&theme.syntect);
        let mut highlight_state =
            HighlightState::new(&highlighter, ScopeStack::new());

        // Duplicate text() method to avoid borrowing issue.
        let text = &self.history[self.active_history_index].text;

        let mut full_line = String::new();
        for line in text.lines() {
            full_line.clear();
            // TODO: any way to avoid pulling the full line in? Should
            // at least limit the length probably.
            for chunk in line.slice.chunks() {
                full_line.push_str(chunk);
            }

            let changes =
                parse_state.parse_line(&full_line, &syntax_set).unwrap();

            let iter = RangedHighlightIterator::new(
                &mut highlight_state,
                &changes,
                &full_line,
                &highlighter,
            );

            self.style_spans.push(StyledLine(
                iter.map(|(style, _text, range)| {
                    // Convert from byte range to char range.
                    let start = line.slice.byte_to_char(range.start);
                    let end = line.slice.byte_to_char(range.end);
                    StyleSpan {
                        len: end - start,
                        style,
                    }
                })
                .collect(),
            ));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_buf(text: &str) -> (Buffer, PaneId) {
        let mut buf = Buffer::create_empty();
        buf.set_text(text);
        let pane_id = PaneId::new();
        buf.set_cursor(&pane_id, AbsChar(0));
        (buf, pane_id)
    }

    #[test]
    fn test_move_cursor_line_end() {
        let (mut buf, pane_id) = create_buf("abc\n");

        buf.move_cursor(
            &pane_id,
            Move::Boundary(Boundary::LineEnd),
            Direction::Inc,
        )
        .unwrap();
        assert_eq!(buf.cursor(&pane_id), AbsChar(3));
    }

    #[test]
    fn test_move_cursor_line_end_no_newline() {
        let (mut buf, pane_id) = create_buf("abc");

        buf.move_cursor(
            &pane_id,
            Move::Boundary(Boundary::LineEnd),
            Direction::Inc,
        )
        .unwrap();
        assert_eq!(buf.cursor(&pane_id), AbsChar(3));
    }
}
