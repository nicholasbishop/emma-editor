/// Relative line offset.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd)]
pub struct RelLine(pub usize);

/// Line index (zero indexed) within the buffer.
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
