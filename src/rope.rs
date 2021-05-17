//! Thin wrapper around `ropey`.

use std::{
    io::{self, Read},
    ops::{Bound, RangeBounds},
};

// TODO: make `pub usize` below not `pub`.

/// Char index (zero indexed) within the rope.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct AbsChar(pub usize);

/// Relative char offset.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct RelChar(pub usize);

/// Relative line offset.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct RelLine(pub usize);

/// Line index (zero indexed) within the rope.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct AbsLine(pub usize);

impl AbsLine {
    pub fn offset_from(&self, val: usize) -> Option<RelLine> {
        Some(RelLine(self.0.checked_sub(val)?))
    }

    pub fn saturating_sub(&self, val: usize) -> AbsLine {
        AbsLine(self.0.saturating_sub(val))
    }
}

#[derive(Clone)]
pub struct Rope(ropey::Rope);

pub struct RopeSlice<'a>(ropey::RopeSlice<'a>);

pub struct Lines<'a>(ropey::iter::Lines<'a>);

impl<'a> Iterator for Lines<'a> {
    type Item = RopeSlice<'a>;

    fn next(&mut self) -> Option<RopeSlice<'a>> {
        self.0.next().map(RopeSlice)
    }
}

impl ToString for Rope {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl Rope {
    pub fn new() -> Rope {
        Rope(ropey::Rope::new())
    }

    pub fn from_reader<T: Read>(reader: T) -> io::Result<Self> {
        ropey::Rope::from_reader(reader).map(Rope)
    }

    pub fn char_to_line(&self, char_idx: AbsChar) -> AbsLine {
        AbsLine(self.0.char_to_line(char_idx.0))
    }

    pub fn from_str(text: &str) -> Self {
        Rope(ropey::Rope::from_str(text))
    }

    pub fn insert(&mut self, char_idx: AbsChar, text: &str) {
        self.0.insert(char_idx.0, text);
    }

    pub fn len_chars(&self) -> usize {
        self.0.len_chars()
    }

    pub fn len_lines(&self) -> usize {
        self.0.len_lines()
    }

    pub fn line(&self, line_idx: AbsLine) -> RopeSlice {
        RopeSlice(self.0.line(line_idx.0))
    }

    pub fn lines(&self) -> Lines {
        Lines(self.0.lines())
    }

    // TODO: use AbsChar
    pub fn line_to_char(&self, line_idx: AbsLine) -> usize {
        self.0.line_to_char(line_idx.0)
    }

    pub fn lines_at(&self, line_idx: AbsLine) -> Lines {
        Lines(self.0.lines_at(line_idx.0))
    }

    pub fn remove<R>(&mut self, char_range: R)
    where
        R: RangeBounds<AbsChar>,
    {
        self.0.remove(convert_abs_char_range_bounds(char_range));
    }

    pub fn slice<R>(&self, char_range: R) -> RopeSlice
    where
        R: RangeBounds<AbsChar>,
    {
        RopeSlice(self.0.slice(convert_abs_char_range_bounds(char_range)))
    }
}

impl<'a> ToString for RopeSlice<'a> {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl<'a> RopeSlice<'a> {
    // TODO: stricter type
    pub fn byte_to_char(&self, byte_idx: usize) -> usize {
        self.0.byte_to_char(byte_idx)
    }

    // TODO: stricter type
    pub fn char_to_byte(&self, char_idx: usize) -> usize {
        self.0.char_to_byte(char_idx)
    }

    // TODO: stricter type
    pub fn chunk_at_byte(
        &self,
        byte_idx: usize,
    ) -> (&str, usize, usize, usize) {
        self.0.chunk_at_byte(byte_idx)
    }

    pub fn chunks(&self) -> ropey::iter::Chunks<'a> {
        self.0.chunks()
    }

    pub fn len_bytes(&self) -> usize {
        self.0.len_bytes()
    }

    pub fn len_chars(&self) -> usize {
        self.0.len_chars()
    }

    // TODO: stricter type
    pub fn slice<R>(&self, char_range: R) -> Self
    where
        R: RangeBounds<usize>,
    {
        RopeSlice(self.0.slice(char_range))
    }
}

fn convert_abs_char_bound(b: Bound<&AbsChar>) -> Bound<usize> {
    match b {
        Bound::Included(v) => Bound::Included(v.0),
        Bound::Excluded(v) => Bound::Excluded(v.0),
        Bound::Unbounded => Bound::Unbounded,
    }
}

fn convert_abs_char_range_bounds<R: RangeBounds<AbsChar>>(
    char_range: R,
) -> (Bound<usize>, Bound<usize>) {
    (
        convert_abs_char_bound(char_range.start_bound()),
        convert_abs_char_bound(char_range.end_bound()),
    )
}
