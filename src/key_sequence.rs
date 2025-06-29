use gtk4::gdk::{self, Key, ModifierType};
use gtk4::glib::translate::FromGlib;
use std::collections::HashMap;

fn name_to_key_map() -> HashMap<&'static str, gdk::Key> {
    // This map is the only place that needs to be updated to add a
    // new named key.
    let mut map = HashMap::new();
    map.insert("backspace", Key::BackSpace);
    map.insert("esc", Key::Escape);
    map.insert("space", Key::space);
    map.insert("ret", Key::Return);
    map.insert("plus", Key::plus);
    map.insert("less", Key::less);
    map.insert("greater", Key::greater);
    map
}

pub fn is_modifier(key: &Key) -> bool {
    matches!(
        *key,
        Key::Alt_L
            | Key::Alt_R
            | Key::Control_L
            | Key::Control_R
            | Key::Shift_L
            | Key::Shift_R
    )
}

#[derive(Clone, Debug, Eq, PartialEq, Hash)]
pub struct KeySequenceAtom {
    pub modifiers: ModifierType,
    pub key: Key,
}

impl KeySequenceAtom {
    pub fn from_event(key: Key, state: ModifierType) -> Self {
        Self {
            modifiers: state,
            // Convert the key to lowercase as a way to
            // normalize. This is far from perfect, for example "?"
            // should probably be the same thing as "<shift>/", but
            // that's not handled well right now.
            key: key.to_lower(),
        }
    }
}

fn single_modifier_to_string(m: &ModifierType) -> &'static str {
    if *m == ModifierType::CONTROL_MASK {
        "ctrl"
    } else if *m == ModifierType::SHIFT_MASK {
        "shift"
    } else if *m == ModifierType::ALT_MASK {
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
        format!("<{name}>")
    } else if let Some(c) = key.to_unicode() {
        format!("\"{c}\"")
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
    UnexpectedModifier(ModifierType),

    #[error("unexpected key {}", key_to_string(.0))]
    UnexpectedKey(Key),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ParseItem {
    Modifier(ModifierType),
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
    names.insert("ctrl", ParseItem::Modifier(ModifierType::CONTROL_MASK));
    names.insert("shift", ParseItem::Modifier(ModifierType::SHIFT_MASK));
    names.insert("alt", ParseItem::Modifier(ModifierType::ALT_MASK));
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
                    let keyval = gdk::unicode_to_keyval(c as u32);
                    // TODO: any safe way to do this?
                    let key = unsafe { Key::from_glib(keyval) };
                    items.push(ParseItem::Key(key))
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

#[derive(Clone, Debug, Default, Eq, PartialEq, Hash)]
pub struct KeySequence(pub Vec<KeySequenceAtom>);

impl KeySequence {
    fn from_items(items: &[ParseItem]) -> Result<Self, Error> {
        enum State {
            ModOrKeyRequired,
            AppendRequired,
        }

        let mut state = State::ModOrKeyRequired;
        let mut seq = Vec::new();
        let mut cur_mods = ModifierType::empty();

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
                        key: *k,
                    });
                    cur_mods = ModifierType::empty();

                    match state {
                        State::ModOrKeyRequired => {
                            state = State::AppendRequired;
                        }
                        State::AppendRequired => {
                            return Err(Error::UnexpectedKey(*k));
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
            format!(
                "{}",
                Error::UnexpectedModifier(ModifierType::CONTROL_MASK)
            ),
            "unexpected modifier ctrl".to_string()
        );

        assert_eq!(
            format!("{}", Error::UnexpectedKey(Key::a)),
            "unexpected key \"a\"".to_string()
        );

        assert_eq!(
            format!("{}", Error::UnexpectedKey(Key::Escape)),
            "unexpected key <esc>".to_string()
        );

        assert_eq!(
            format!("{}", Error::UnexpectedKey(Key::BackSpace)),
            "unexpected key <backspace>".to_string()
        );
    }

    #[test]
    fn test_parse_key_sequence() {
        assert_eq!(
            parse_key_sequence_as_items("aa"),
            Ok(vec![ParseItem::Key(Key::a), ParseItem::Key(Key::a)])
        );

        assert_eq!(
            parse_key_sequence_as_items("<ctrl><shift>"),
            Ok(vec![
                ParseItem::Modifier(ModifierType::CONTROL_MASK),
                ParseItem::Modifier(ModifierType::SHIFT_MASK),
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
            KeySequence::from_items(&[ParseItem::Key(Key::a)]),
            Ok(KeySequence(vec![KeySequenceAtom {
                modifiers: ModifierType::empty(),
                key: Key::a,
            }]))
        );

        assert_eq!(
            KeySequence::from_items(&[
                ParseItem::Modifier(ModifierType::CONTROL_MASK),
                ParseItem::Key(Key::a)
            ]),
            Ok(KeySequence(vec![KeySequenceAtom {
                modifiers: ModifierType::CONTROL_MASK,
                key: Key::a,
            }]))
        );

        assert_eq!(
            KeySequence::from_items(&[
                ParseItem::Modifier(ModifierType::CONTROL_MASK),
                ParseItem::Key(Key::x),
                ParseItem::Append,
                ParseItem::Key(Key::a),
            ]),
            Ok(KeySequence(vec![
                KeySequenceAtom {
                    modifiers: ModifierType::CONTROL_MASK,
                    key: Key::x,
                },
                KeySequenceAtom {
                    modifiers: ModifierType::empty(),
                    key: Key::a,
                }
            ]))
        );

        assert_eq!(
            KeySequence::from_items(&[
                ParseItem::Modifier(ModifierType::CONTROL_MASK),
                ParseItem::Key(Key::x),
                ParseItem::Append,
                ParseItem::Modifier(ModifierType::CONTROL_MASK),
                ParseItem::Key(Key::a),
            ]),
            Ok(KeySequence(vec![
                KeySequenceAtom {
                    modifiers: ModifierType::CONTROL_MASK,
                    key: Key::x,
                },
                KeySequenceAtom {
                    modifiers: ModifierType::CONTROL_MASK,
                    key: Key::a,
                }
            ]))
        );

        // Errors

        assert_eq!(
            KeySequence::from_items(&[
                ParseItem::Key(Key::a),
                ParseItem::Modifier(ModifierType::CONTROL_MASK),
            ]),
            Err(Error::UnexpectedModifier(ModifierType::CONTROL_MASK))
        );

        assert_eq!(
            KeySequence::from_items(&[ParseItem::Append]),
            Err(Error::UnexpectedAppend)
        );

        assert_eq!(
            KeySequence::from_items(&[
                ParseItem::Key(Key::a),
                ParseItem::Key(Key::a),
            ]),
            Err(Error::UnexpectedKey(Key::a))
        );
    }
}
