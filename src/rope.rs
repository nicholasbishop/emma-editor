//! Thin wrapper around `ropey`.

// TODO
#![allow(clippy::to_string_trait_impl)]

use serde::{Deserialize, Serialize};
use std::io::{self, Read};
use std::ops::{Add, AddAssign, Bound, RangeBounds};

// TODO: make `pub usize` below not `pub`.

/// Char index (zero indexed) within the rope.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Deserialize,
    Serialize,
)]
pub struct AbsChar(pub usize);

/// Relative char offset.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct RelChar(pub usize);

/// Relative line offset.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct RelLine(usize);

/// Line index (zero indexed) within the rope.
#[derive(
    Clone,
    Copy,
    Debug,
    Default,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Deserialize,
    Serialize,
)]
pub struct AbsLine(pub usize);

impl RelChar {
    pub fn zero() -> Self {
        Self(0)
    }
}

impl RelLine {
    pub fn new(val: usize) -> Self {
        Self(val)
    }
}

impl AddAssign<usize> for RelLine {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs
    }
}

impl Add<RelLine> for AbsLine {
    type Output = Self;

    fn add(self, rhs: RelLine) -> Self {
        Self(self.0 + rhs.0)
    }
}

impl AddAssign<usize> for AbsLine {
    fn add_assign(&mut self, rhs: usize) {
        self.0 += rhs
    }
}

impl AbsLine {
    pub fn zero() -> Self {
        Self(0)
    }

    pub fn offset_from(&self, val: Self) -> Option<RelLine> {
        Some(RelLine(self.0.checked_sub(val.0)?))
    }

    pub fn saturating_sub(&self, val: RelLine) -> Self {
        Self(self.0.saturating_sub(val.0))
    }
}

#[derive(Clone)]
pub struct Rope(ropey::Rope);

pub struct RopeSlice<'a>(ropey::RopeSlice<'a>);

pub struct Lines<'a> {
    iter: ropey::iter::Lines<'a>,
    index: AbsLine,
}

pub struct LinesIterItem<'a> {
    pub slice: RopeSlice<'a>,
    pub index: AbsLine,
}

impl<'a> Iterator for Lines<'a> {
    type Item = LinesIterItem<'a>;

    fn next(&mut self) -> Option<Self::Item> {
        let slice = self.iter.next()?;
        let item = Self::Item {
            slice: RopeSlice(slice),
            index: self.index,
        };
        self.index += 1;
        Some(item)
    }
}

impl ToString for Rope {
    fn to_string(&self) -> String {
        self.0.to_string()
    }
}

impl Rope {
    pub fn new() -> Self {
        Self(ropey::Rope::new())
    }

    pub fn from_reader<T: Read>(reader: T) -> io::Result<Self> {
        ropey::Rope::from_reader(reader).map(Rope)
    }

    pub fn char_to_line(&self, char_idx: AbsChar) -> AbsLine {
        AbsLine(self.0.char_to_line(char_idx.0))
    }

    pub fn from_str(text: &str) -> Self {
        Self(ropey::Rope::from_str(text))
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

    pub fn max_line_index(&self) -> AbsLine {
        AbsLine(self.0.len_lines() - 1)
    }

    pub fn line(&self, line_idx: AbsLine) -> RopeSlice {
        RopeSlice(self.0.line(line_idx.0))
    }

    pub fn lines(&self) -> Lines {
        Lines {
            iter: self.0.lines(),
            index: AbsLine::zero(),
        }
    }

    // TODO: use AbsChar
    pub fn line_to_char(&self, line_idx: AbsLine) -> usize {
        self.0.line_to_char(line_idx.0)
    }

    pub fn lines_at(&self, line_idx: AbsLine) -> Lines {
        Lines {
            iter: self.0.lines_at(line_idx.0),
            index: line_idx,
        }
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

impl ToString for RopeSlice<'_> {
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

/// Container for data associated with a contiguous range of lines.
pub struct LineDataVec<T> {
    lines: Vec<T>,
    start_line: AbsLine,
}

pub struct LineDataIterItem<'a, T> {
    pub data: &'a T,
    pub index: AbsLine,
}

pub struct LineDataIter<'a, T> {
    data: &'a LineDataVec<T>,
    offset: RelLine,
}

impl<'a, T> Iterator for LineDataIter<'a, T> {
    type Item = LineDataIterItem<'a, T>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.offset.0 >= self.data.lines.len() {
            return None;
        }

        let item = LineDataIterItem {
            data: &self.data.lines[self.offset.0],
            index: self.data.start_line + self.offset,
        };
        self.offset += 1;

        Some(item)
    }
}

impl<T: Clone + Default> LineDataVec<T> {
    pub fn with_size(start_line: AbsLine, len: usize) -> Self {
        Self {
            lines: vec![T::default(); len],
            start_line,
        }
    }
}

impl<T> LineDataVec<T> {
    pub fn new(start_line: AbsLine) -> Self {
        Self {
            lines: Vec::new(),
            start_line,
        }
    }

    pub fn start_line(&self) -> AbsLine {
        self.start_line
    }

    pub fn get(&self, abs_line: AbsLine) -> Option<&T> {
        let offset = abs_line.offset_from(self.start_line)?;
        self.lines.get(offset.0)
    }

    pub fn get_mut(&mut self, abs_line: AbsLine) -> Option<&mut T> {
        let offset = abs_line.offset_from(self.start_line)?;
        self.lines.get_mut(offset.0)
    }

    pub fn iter(&self) -> LineDataIter<T> {
        self.starting_from(self.start_line())
    }

    pub fn starting_from(&self, abs_line: AbsLine) -> LineDataIter<T> {
        LineDataIter {
            data: self,
            // If input index is less than the start line, set the offset to
            // the end of the data so that the iterator will return nothing.
            offset: abs_line
                .offset_from(self.start_line)
                .unwrap_or(RelLine(self.lines.len())),
        }
    }

    pub fn push(&mut self, elem: T) {
        self.lines.push(elem);
    }

    pub fn clear(&mut self) {
        self.lines.clear();
    }
}
