use std::fmt::{self, Display, Formatter};
use tracing::error;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Hash)]
pub enum Key {
    Modifier(Modifier),
    Char(char),

    // TODO: some of these could be represented with Char, is there a
    // good reason not to?
    Backspace,
    Escape,
    Greater,
    Less,
    Plus,
    Return,
    Space,
}

impl Key {
    pub fn is_modifier(self) -> bool {
        matches!(self, Self::Modifier(_))
    }

    pub fn from_char(c: char) -> Self {
        Self::Char(c)
    }

    pub fn to_lower(self) -> Self {
        if let Self::Char(c) = self {
            let s = c.to_lowercase().to_string();
            let mut chars = s.chars();
            if let Some(c) = chars.next() {
                if chars.next().is_none() {
                    Self::Char(c)
                } else {
                    // TODO: for now just return the original key.
                    error!("lowercased character is no longer a single char");
                    self
                }
            } else {
                unreachable!("lowercased character is an empty string");
            }
        } else {
            self
        }
    }

    pub fn to_upper(self) -> Self {
        if let Self::Char(c) = self {
            let s = c.to_uppercase().to_string();
            let mut chars = s.chars();
            if let Some(c) = chars.next() {
                if chars.next().is_none() {
                    Self::Char(c)
                } else {
                    // TODO: for now just return the original key.
                    error!("uppercased character is no longer a single char");
                    self
                }
            } else {
                unreachable!("uppercased character is an empty string");
            }
        } else {
            self
        }
    }

    pub fn to_char(self) -> Option<char> {
        match self {
            Self::Char(c) => Some(c),
            Self::Less => Some('<'),
            Self::Greater => Some('>'),
            Self::Plus => Some('+'),
            Self::Space => Some(' '),
            Self::Return => Some('\n'),
            _ => None,
        }
    }
}

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

impl From<Modifier> for Modifiers {
    fn from(modifier: Modifier) -> Self {
        match modifier {
            Modifier::Alt => Self {
                alt: true,
                control: false,
                shift: false,
            },
            Modifier::Control => Self {
                alt: false,
                control: true,
                shift: false,
            },
            Modifier::Shift => Self {
                alt: false,
                control: false,
                shift: true,
            },
        }
    }
}
