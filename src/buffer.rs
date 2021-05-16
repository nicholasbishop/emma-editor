pub use crate::rope::{AbsLine, RelLine};

use {
    crate::{
        grapheme::{next_grapheme_boundary, prev_grapheme_boundary},
        pane_tree::{Pane, PaneId},
        rope::Rope,
        theme::Theme,
        util,
    },
    aho_corasick::AhoCorasick,
    anyhow::Error,
    fehler::throws,
    std::{
        collections::HashMap,
        fmt, fs, io,
        ops::Range,
        path::{Path, PathBuf},
    },
    syntect::{
        highlighting::{
            HighlightState, Highlighter, RangedHighlightIterator, Style,
        },
        parsing::{ParseState, ScopeStack, SyntaxReference, SyntaxSet},
    },
};

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

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct BufferId(String);

impl BufferId {
    fn new() -> BufferId {
        BufferId(util::make_id("buffer"))
    }

    fn minibuf() -> BufferId {
        BufferId("buffer-minibuf".into())
    }
}

impl fmt::Display for BufferId {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.0)
    }
}

/// Char index (zero indexed) within the buffer.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct AbsChar(pub usize);

impl AbsChar {
    /// Convert the AbsChar to a LinePosition.
    pub fn line_position(&self, buf: &Buffer) -> LinePosition {
        let text = &buf.text();

        let line = text.char_to_line(self.0);
        let line_offset = self.0 - text.line_to_char(line);

        LinePosition {
            line,
            offset: line_offset,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LinePosition {
    /// Line index.
    pub line: AbsLine,
    /// Character offset from the start of the line.
    pub offset: usize,
}

impl LinePosition {
    pub fn to_abs_char(&self, buf: &Buffer) -> AbsChar {
        AbsChar(buf.text().line_to_char(self.line) + self.offset)
    }

    /// Count the number of graphemes between the start of the line
    /// and the line offset.
    pub fn grapheme_offset(&self, buf: &Buffer) -> usize {
        let line = buf.text().line(self.line);
        let mut num_graphemes = 0;
        let mut cur_offset = 0;
        while cur_offset < self.offset {
            let new_offset = next_grapheme_boundary(&line, cur_offset);
            if cur_offset == new_offset {
                break;
            } else {
                num_graphemes += 1;
                cur_offset = new_offset;
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
        self.offset = 0;
        while num_graphemes > 0 {
            self.offset = next_grapheme_boundary(&line, self.offset);
            num_graphemes -= 1;
            if self.offset >= num_chars {
                self.offset = num_chars;
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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ActionType {
    None,
    Clear,
    InsertChar,
    Deletion,
}

type CursorMap = HashMap<PaneId, AbsChar>;

#[derive(Clone)]
struct HistoryItem {
    text: Rope,
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
    start_line_index: AbsLine,
    matches: Vec<LineMatches>,
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

        if let Some(offset) = line_index.offset_from(pane.top_line().0) {
            self.matches.get(offset.0)
        } else {
            None
        }
    }

    pub fn next_match(&self, line_pos: LinePosition) -> Option<LinePosition> {
        let lm_base = line_pos.line.offset_from(self.start_line_index.0)?;
        for (lm_offset, lm) in self.matches.iter().skip(lm_base.0).enumerate() {
            let line = AbsLine(self.start_line_index.0 + lm_base.0 + lm_offset);

            for span in &lm.spans {
                // Ignore matches on line_pos's line that are before
                // the char offset.
                if line == line_pos.line && span.start < line_pos.offset {
                    continue;
                }

                return Some(LinePosition {
                    line,
                    offset: span.start,
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

    // TODO: consider using a reference instead of always cloning
    // theme.
    theme: Theme,

    // Outer vec: per line
    // Inner vec: style for a contiguous group of chars, covers the
    // whole line.
    // TODO: think about a smarter structure
    // TODO: put in arc for async update
    style_spans: Vec<Vec<StyleSpan>>,

    search: Option<SearchState>,
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
    fn new(
        id: BufferId,
        text: Rope,
        path: Option<PathBuf>,
        theme: &Theme,
    ) -> Buffer {
        let mut buf = Buffer {
            id,
            history: vec![HistoryItem {
                text,
                cursors: CursorMap::new(),
            }],
            active_history_index: 0,
            last_action_type: ActionType::None,
            path,
            theme: theme.clone(),
            style_spans: Vec::new(),
            search: None,
        };

        // TODO, async
        buf.recalc_style_spans();

        buf
    }

    pub fn create_minibuf(theme: &Theme) -> Buffer {
        Buffer::new(BufferId::minibuf(), Rope::new(), None, theme)
    }

    #[throws]
    pub fn from_path(path: &Path, theme: &Theme) -> Buffer {
        let text =
            Rope::from_reader(&mut io::BufReader::new(fs::File::open(path)?))?;
        Buffer::new(BufferId::new(), text, Some(path.into()), theme)
    }

    pub fn id(&self) -> &BufferId {
        &self.id
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

    pub fn style_spans(&self) -> &Vec<Vec<StyleSpan>> {
        &self.style_spans
    }

    pub fn search_state(&self) -> &Option<SearchState> {
        &self.search
    }

    pub fn cursor(&self, pane: &Pane) -> AbsChar {
        *self
            .active_history_item()
            .cursors
            .get(pane.id())
            .expect("no cursor for pane")
    }

    pub fn set_cursor(&mut self, pane: &Pane, cursor: AbsChar) {
        // This isn't an undoable action, but should prevent history
        // (e.g. press 'a', move cursor, press 'b' should be two
        // history items, not one).
        self.last_action_type = ActionType::None;

        self.cursors_mut().insert(pane.id().clone(), cursor);

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
            (Boundary::Grapheme, Direction::Dec) => AbsChar(
                prev_grapheme_boundary(&text.slice(0..text.len_chars()), pos.0),
            ),
            (Boundary::Grapheme, Direction::Inc) => AbsChar(
                next_grapheme_boundary(&text.slice(0..text.len_chars()), pos.0),
            ),
            (Boundary::LineEnd, direction) => {
                let mut lp = pos.line_position(self);
                if direction == Direction::Dec {
                    // TODO: add logic to initially move to
                    // first-non-whitespace char.
                    lp.offset = 0;
                } else {
                    lp.offset = text.line(lp.line).len_chars() - 1;
                }
                lp.to_abs_char(self)
            }
            (Boundary::BufferEnd, Direction::Dec) => AbsChar(0),
            (Boundary::BufferEnd, Direction::Inc) => AbsChar(text.len_chars()),
        }
    }

    pub fn delete_text(&mut self, range: Range<AbsChar>) {
        self.maybe_store_history_item(ActionType::Deletion);

        self.text_mut().unwrap().remove(range.start.0..range.end.0);

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

        self.text_mut().unwrap().insert(pos.0, &c.to_string());

        // Update the associated style span to account for the new
        // character.
        let lp = pos.line_position(self);
        if let Some(spans) = self.style_spans.get_mut(lp.line.0) {
            let offset = 0;
            for span in spans {
                if lp.offset >= offset && lp.offset < offset + span.len {
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

        // TODO: update all cursors
    }

    pub fn search(&mut self, text: &str, pane: &Pane, num_lines: usize) {
        if text.is_empty() {
            return;
        }

        let mut state = SearchState {
            pane_id: pane.id().clone(),
            start_line_index: pane.top_line(),
            matches: Vec::with_capacity(num_lines),
        };
        state.matches.resize(num_lines, LineMatches::default());

        let ac = AhoCorasick::new(&[text]);
        for (offset, line) in
            self.text().lines_at(state.start_line_index).enumerate()
        {
            if offset > num_lines {
                break;
            }

            let line_str = line.to_string();
            for m in ac.find_iter(&line_str) {
                state.matches[offset].spans.push(m.start()..m.end());
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
        let highlighter = Highlighter::new(&self.theme.syntect);
        let mut highlight_state =
            HighlightState::new(&highlighter, ScopeStack::new());

        // Duplicate text() method to avoid borrowing issue.
        let text = &self.history[self.active_history_index].text;

        let mut full_line = String::new();
        for line in text.lines() {
            full_line.clear();
            // TODO: any way to avoid pulling the full line in? Should
            // at least limit the length probably.
            for chunk in line.chunks() {
                full_line.push_str(chunk);
            }

            let changes = parse_state.parse_line(&full_line, &syntax_set);

            let iter = RangedHighlightIterator::new(
                &mut highlight_state,
                &changes,
                &full_line,
                &highlighter,
            );

            self.style_spans.push(
                iter.map(|(style, _text, range)| {
                    // Convert from byte range to char range.
                    let start = line.byte_to_char(range.start);
                    let end = line.byte_to_char(range.end);
                    StyleSpan {
                        len: end - start,
                        style,
                    }
                })
                .collect(),
            );
        }
    }
}
