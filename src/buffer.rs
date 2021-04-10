use {
    crate::{
        grapheme::{next_grapheme_boundary, prev_grapheme_boundary},
        pane_tree::{Pane, PaneId},
        theme::Theme,
        util,
    },
    anyhow::Error,
    fehler::throws,
    ropey::Rope,
    std::{
        collections::HashMap,
        fmt, fs, io,
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
    Grapheme(Direction),
    // TODO:
    // Subword(Direction),
    // Word(Direction),
    // LineEnd(Direction),
    // Line,
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

/// Char index within the buffer.
#[derive(Clone, Copy, Debug, Default)]
pub struct Position(pub usize);

impl Position {
    pub fn from_line_position(pos: LinePosition, buf: &Buffer) -> Position {
        Position(buf.text.line_to_char(pos.line) + pos.offset)
    }

    /// Convert the Position to a LinePosition.
    pub fn line_position(&self, buf: &Buffer) -> LinePosition {
        let text = &buf.text;

        let line_idx = text.char_to_line(self.0);
        let line_offset = self.0 - text.line_to_char(line_idx);

        LinePosition {
            line: line_idx,
            offset: line_offset,
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct LinePosition {
    /// Line index (zero-indexed).
    pub line: usize,
    /// Character offset from the start of the line.
    pub offset: usize,
}

impl LinePosition {
    /// Count the number of graphemes between the start of the line
    /// and the line offset.
    pub fn grapheme_offset(&self, buf: &Buffer) -> usize {
        let line = buf.text.line(self.line);
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
        let line = buf.text.line(self.line);
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

#[derive(Debug)]
pub struct StyleSpan {
    pub len: usize,
    pub style: Style,
}

pub struct Buffer {
    id: BufferId,

    text: Rope,
    path: Option<PathBuf>,

    // TODO: consider using a reference instead of always cloning
    // theme.
    theme: Theme,

    // Outer vec: per line
    // Inner vec: style for a contiguous group of chars, covers the
    // whole line.
    // TODO: think about a smarter structure
    // TODO: put in arc for async update
    style_spans: Vec<Vec<StyleSpan>>,

    // Each pane showing this buffer has its own cursor.
    cursors: HashMap<PaneId, Position>,
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
    pub fn create_minibuf(theme: &Theme) -> Buffer {
        let mut buf = Buffer {
            id: BufferId::minibuf(),
            text: Rope::new(),
            path: None,
            theme: theme.clone(),
            style_spans: Vec::new(),
            cursors: HashMap::new(),
        };

        // TODO, async
        buf.recalc_style_spans();

        buf
    }

    #[throws]
    pub fn from_path(path: &Path, theme: &Theme) -> Buffer {
        let text =
            Rope::from_reader(&mut io::BufReader::new(fs::File::open(path)?))?;
        let mut buf = Buffer {
            id: BufferId::new(),
            text,
            path: Some(path.into()),
            theme: theme.clone(),
            style_spans: Vec::new(),
            cursors: HashMap::new(),
        };

        // TODO, async
        buf.recalc_style_spans();

        buf
    }

    pub fn id(&self) -> &BufferId {
        &self.id
    }

    pub fn text(&self) -> &Rope {
        &self.text
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn style_spans(&self) -> &Vec<Vec<StyleSpan>> {
        &self.style_spans
    }

    pub fn cursor(&self, pane: &Pane) -> Position {
        *self.cursors.get(pane.id()).expect("no cursor for pane")
    }

    pub fn set_cursor(&mut self, pane: &Pane, cursor: Position) {
        self.cursors.insert(pane.id().clone(), cursor);
    }

    pub fn remove_cursor(&mut self, pane: &Pane) {
        self.cursors.remove(&pane.id());
    }

    /// Remove all text from the buffer.
    pub fn clear(&mut self) {
        self.text = Rope::new();

        // TODO: async style recalc
        self.recalc_style_spans();

        // Update all cursors.
        for cursor in self.cursors.values_mut() {
            cursor.0 = 0;
        }
    }

    pub fn delete_text(&mut self, pos: Position, boundary: Boundary) {
        match boundary {
            Boundary::Grapheme(dir) => {
                if dir == Direction::Dec {
                    if pos.0 > 0 {
                        let start = prev_grapheme_boundary(
                            &self.text.slice(0..self.text.len_chars()),
                            pos.0,
                        );
                        let range = start..pos.0;
                        self.text.remove(range.clone());

                        // Update all cursors in this buffer.
                        for cursor in self.cursors.values_mut() {
                            if range.contains(&cursor.0) {
                                cursor.0 = range.start;
                            } else if cursor.0 >= range.end {
                                cursor.0 -= range.len();
                            }
                        }
                    }
                }
            }
        }

        // TODO: async style recalc
        self.recalc_style_spans();
    }

    pub fn insert_char(&mut self, c: char, pos: Position) {
        self.text.insert(pos.0, &c.to_string());

        // Update the associated style span to account for the new
        // character.
        let lp = pos.line_position(self);
        if let Some(spans) = self.style_spans.get_mut(lp.line) {
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
        for cursor in self.cursors.values_mut() {
            if cursor.0 >= pos.0 {
                cursor.0 += 1;
            }
        }
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

        let mut full_line = String::new();
        for line in self.text.lines() {
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
