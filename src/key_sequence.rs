use iced::keyboard::key::Named;
use iced::keyboard::{Key, Modifiers};
use smol_str::SmolStr;
use std::collections::HashMap;

pub fn charkey(c: char) -> Key {
    Key::Character(SmolStr::new(c.to_string()))
}

fn name_to_key_map() -> HashMap<&'static str, Key> {
    // This map is the only place that needs to be updated to add a
    // new named key.
    let mut map = HashMap::new();
    map.insert("backspace", Key::Named(Named::Backspace));
    map.insert("esc", Key::Named(Named::Escape));
    map.insert("space", Key::Named(Named::Space));
    map.insert("ret", Key::Named(Named::Enter));
    map.insert("plus", charkey('+'));
    map.insert("less", charkey('<'));
    map.insert("greater", charkey('>'));
    map
}

pub fn is_modifier(key: &Key) -> bool {
    matches!(key, Key::Named(Named::Control | Named::Alt | Named::Shift))
}

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct KeySequenceAtom {
    pub modifiers: Modifiers,
    pub key: Key,
}

impl KeySequenceAtom {
    pub fn from_event(key: Key, state: Modifiers) -> Self {
        Self {
            modifiers: state,
            key,
        }
    }
}

fn single_modifier_to_string(m: &Modifiers) -> &'static str {
    if *m == Modifiers::CTRL {
        "ctrl"
    } else if *m == Modifiers::SHIFT {
        "shift"
    } else if *m == Modifiers::ALT {
        "alt"
    } else {
        "unknown"
    }
}

fn key_to_name_map() -> HashMap<Key, &'static str> {
    let mut map = HashMap::new();
    for (k, v) in name_to_key_map() {
        map.insert(v, k);
    }
    map
}

fn key_to_string(key: &Key) -> String {
    if let Some(name) = key_to_name_map().get(key) {
        format!("<{}>", name)
    } else if let Key::Character(c) = key {
        format!("\"{}\"", c.as_str())
    } else {
        "unknown".into()
    }
}

#[derive(thiserror::Error, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    #[error("invalid name: \"{0}\"")]
    InvalidName(String),

    #[error("unexpected \"+\"")]
    UnexpectedAppend,

    #[error("unexpected modifier {}", single_modifier_to_string(.0))]
    UnexpectedModifier(Modifiers),

    #[error("unexpected key {}", key_to_string(.0))]
    UnexpectedKey(Key),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ParseItem {
    Modifier(Modifiers),
    Key(Key),
    Append,
}

fn parse_key_sequence_as_items(s: &str) -> Result<Vec<ParseItem>, Error> {
    enum State {
        Initial,
        InName,
    }

    let mut state = State::Initial;

    let mut names = HashMap::new();
    names.insert("ctrl", ParseItem::Modifier(Modifiers::CTRL));
    names.insert("shift", ParseItem::Modifier(Modifiers::SHIFT));
    names.insert("alt", ParseItem::Modifier(Modifiers::ALT));
    for (k, v) in name_to_key_map() {
        names.insert(k, ParseItem::Key(v));
    }

    let mut items = Vec::new();
    let mut name = String::new();
    for c in s.chars() {
        match state {
            State::Initial => {
                if c == '<' {
                    state = State::InName;
                } else if c == '+' {
                    items.push(ParseItem::Append);
                } else {
                    items.push(ParseItem::Key(charkey(c)))
                }
            }
            State::InName => {
                if c == '>' {
                    if let Some(val) = names.get(name.as_str()) {
                        items.push(val.clone());
                    } else {
                        return Err(Error::InvalidName(name));
                    }
                    name.clear();
                    state = State::Initial;
                } else {
                    name.push(c);
                }
            }
        }
    }

    Ok(items)
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct KeySequence(pub Vec<KeySequenceAtom>);

impl KeySequence {
    fn from_items(items: &[ParseItem]) -> Result<Self, Error> {
        enum State {
            ModOrKeyRequired,
            AppendRequired,
        }

        let mut state = State::ModOrKeyRequired;
        let mut seq = Vec::new();
        let mut cur_mods = Modifiers::empty();

        for item in items {
            match item {
                ParseItem::Modifier(m) => {
                    cur_mods |= *m;

                    match state {
                        State::ModOrKeyRequired => {
                            state = State::ModOrKeyRequired;
                        }
                        State::AppendRequired => {
                            return Err(Error::UnexpectedModifier(*m));
                        }
                    }
                }
                ParseItem::Key(k) => {
                    seq.push(KeySequenceAtom {
                        modifiers: cur_mods,
                        key: k.clone(),
                    });
                    cur_mods = Modifiers::empty();

                    match state {
                        State::ModOrKeyRequired => {
                            state = State::AppendRequired;
                        }
                        State::AppendRequired => {
                            return Err(Error::UnexpectedKey(k.clone()));
                        }
                    }
                }
                ParseItem::Append => match state {
                    State::ModOrKeyRequired => {
                        return Err(Error::UnexpectedAppend);
                    }
                    State::AppendRequired => {
                        state = State::ModOrKeyRequired;
                    }
                },
            }
        }

        Ok(Self(seq))
    }

    pub fn parse(s: &str) -> Result<Self, Error> {
        let items = parse_key_sequence_as_items(s)?;
        Self::from_items(&items)
    }

    pub fn starts_with(&self, other: &Self) -> bool {
        self.0.starts_with(&other.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_error_display() {
        assert_eq!(
            format!("{}", Error::UnexpectedModifier(Modifiers::CTRL)),
            "unexpected modifier ctrl".to_string()
        );

        assert_eq!(
            format!(
                "{}",
                Error::UnexpectedKey(Key::Character(SmolStr::new(
                    'a'.to_string()
                )))
            ),
            "unexpected key \"a\"".to_string()
        );

        assert_eq!(
            format!("{}", Error::UnexpectedKey(Key::Named(Named::Escape))),
            "unexpected key <esc>".to_string()
        );

        assert_eq!(
            format!("{}", Error::UnexpectedKey(Key::Named(Named::Backspace))),
            "unexpected key <backspace>".to_string()
        );
    }

    #[test]
    fn test_parse_key_sequence() {
        assert_eq!(
            parse_key_sequence_as_items("aa"),
            Ok(vec![
                ParseItem::Key(charkey('a')),
                ParseItem::Key(charkey('a'))
            ])
        );

        assert_eq!(
            parse_key_sequence_as_items("<ctrl><shift>"),
            Ok(vec![
                ParseItem::Modifier(Modifiers::CTRL),
                ParseItem::Modifier(Modifiers::SHIFT),
            ])
        );

        // Error

        assert_eq!(
            parse_key_sequence_as_items("<invalid>"),
            Err(Error::InvalidName("invalid".into()))
        );
    }

    #[test]
    fn test_sequence_from_items() {
        assert_eq!(
            KeySequence::from_items(&[ParseItem::Key(charkey('a'))]),
            Ok(KeySequence(vec![KeySequenceAtom {
                modifiers: Modifiers::empty(),
                key: charkey('a'),
            }]))
        );

        assert_eq!(
            KeySequence::from_items(&[
                ParseItem::Modifier(Modifiers::CTRL),
                ParseItem::Key(charkey('a'))
            ]),
            Ok(KeySequence(vec![KeySequenceAtom {
                modifiers: Modifiers::CTRL,
                key: charkey('a'),
            }]))
        );

        assert_eq!(
            KeySequence::from_items(&[
                ParseItem::Modifier(Modifiers::CTRL),
                ParseItem::Key(charkey('x')),
                ParseItem::Append,
                ParseItem::Key(charkey('a')),
            ]),
            Ok(KeySequence(vec![
                KeySequenceAtom {
                    modifiers: Modifiers::CTRL,
                    key: charkey('x'),
                },
                KeySequenceAtom {
                    modifiers: Modifiers::empty(),
                    key: charkey('a'),
                }
            ]))
        );

        assert_eq!(
            KeySequence::from_items(&[
                ParseItem::Modifier(Modifiers::CTRL),
                ParseItem::Key(charkey('x')),
                ParseItem::Append,
                ParseItem::Modifier(Modifiers::CTRL),
                ParseItem::Key(charkey('a')),
            ]),
            Ok(KeySequence(vec![
                KeySequenceAtom {
                    modifiers: Modifiers::CTRL,
                    key: charkey('x'),
                },
                KeySequenceAtom {
                    modifiers: Modifiers::CTRL,
                    key: charkey('a'),
                }
            ]))
        );

        // Errors

        assert_eq!(
            KeySequence::from_items(&[
                ParseItem::Key(charkey('a')),
                ParseItem::Modifier(Modifiers::CTRL),
            ]),
            Err(Error::UnexpectedModifier(Modifiers::CTRL))
        );

        assert_eq!(
            KeySequence::from_items(&[ParseItem::Append]),
            Err(Error::UnexpectedAppend)
        );

        assert_eq!(
            KeySequence::from_items(&[
                ParseItem::Key(charkey('a')),
                ParseItem::Key(charkey('a')),
            ]),
            Err(Error::UnexpectedKey(charkey('a')))
        );
    }
}
