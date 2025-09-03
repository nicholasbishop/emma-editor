// TODO
pub use gtk4::gdk::Key;

use std::fmt::{self, Display, Formatter};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Modifier {
    Alt,
    Control,
    Shift,
}

impl Display for Modifier {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{}",
            match self {
                Self::Control => "ctrl",
                Self::Shift => "shift",
                Self::Alt => "alt",
            }
        )
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub struct Modifiers {
    pub alt: bool,
    pub control: bool,
    pub shift: bool,
}

impl Modifiers {
    pub fn new() -> Self {
        Self {
            alt: false,
            control: false,
            shift: false,
        }
    }

    // TODO: naming
    pub fn is_empty(&self) -> bool {
        !self.alt && !self.control && !self.shift
    }

    pub fn enable_modifier(&mut self, modifier: Modifier) {
        match modifier {
            Modifier::Alt => self.alt = true,
            Modifier::Control => self.control = true,
            Modifier::Shift => self.shift = true,
        }
    }
}

impl PartialEq<Modifier> for Modifiers {
    fn eq(&self, modifier: &Modifier) -> bool {
        match modifier {
            Modifier::Alt => self.alt && !self.control && !self.shift,
            Modifier::Control => !self.alt && self.control && !self.shift,
            Modifier::Shift => !self.alt && !self.control && self.shift,
        }
    }
}
