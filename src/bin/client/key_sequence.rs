use gdk::keys::constants as keys;
use gdk::{EventKey, ModifierType};
use glib::translate::FromGlib;
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KeySequenceAtom {
    pub modifiers: ModifierType,
    pub key: gdk::keys::Key,
}

impl KeySequenceAtom {
    pub fn from_event(e: &EventKey) -> KeySequenceAtom {
        KeySequenceAtom {
            modifiers: e.get_state(),
            key: e.get_keyval(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum KeySequenceParseError {
    InvalidEscape(char),
    InvalidName(String),
}

#[derive(Clone)]
enum KeySequenceParseItem {
    Modifier(ModifierType),
    Key(gdk::keys::Key),
    Append,
}

fn parse_key_sequence_as_items(
    s: &str,
) -> Result<Vec<KeySequenceParseItem>, KeySequenceParseError> {
    enum State {
        Initial,
        InName,
        InEscape,
    }

    let mut state = State::Initial;

    let mut names = HashMap::new();
    names.insert(
        "ctrl",
        KeySequenceParseItem::Modifier(ModifierType::CONTROL_MASK),
    );
    names.insert(
        "shift",
        KeySequenceParseItem::Modifier(ModifierType::SHIFT_MASK),
    );
    names.insert(
        "alt",
        KeySequenceParseItem::Modifier(ModifierType::MOD1_MASK),
    );
    names.insert("esc", KeySequenceParseItem::Key(keys::Escape));
    names.insert("space", KeySequenceParseItem::Key(keys::space));

    let mut items = Vec::new();
    let mut name = String::new();
    for c in s.chars() {
        match state {
            State::Initial => {
                if c == '\\' {
                    state = State::InEscape;
                } else if c == '<' {
                    state = State::InName;
                } else if c == '+' {
                    items.push(KeySequenceParseItem::Append);
                } else {
                    let keyval = gdk::unicode_to_keyval(c as u32);
                    items.push(KeySequenceParseItem::Key(
                        gdk::keys::Key::from_glib(keyval),
                    ))
                }
            }
            State::InEscape => {
                if c == '<' {
                    items.push(KeySequenceParseItem::Key(
                        keys::leftanglebracket,
                    ));
                } else if c == '>' {
                    items.push(KeySequenceParseItem::Key(
                        keys::rightanglebracket,
                    ));
                } else if c == '\\' {
                    items.push(KeySequenceParseItem::Key(keys::backslash));
                } else if c == '+' {
                    items.push(KeySequenceParseItem::Key(keys::plus));
                } else {
                    return Err(KeySequenceParseError::InvalidEscape(c));
                }
                state = State::Initial;
            }
            State::InName => {
                if c == '>' {
                    if let Some(val) = names.get(name.as_str()) {
                        items.push(val.clone());
                    } else {
                        return Err(KeySequenceParseError::InvalidName(name));
                    }
                    state = State::Initial;
                } else {
                    name.push(c);
                }
            }
        }
    }

    Ok(items)
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct KeySequence(Vec<KeySequenceAtom>);

impl KeySequence {
    // TODO: change to a Result
    fn parse(s: &str) -> Result<KeySequence, KeySequenceParseError> {
        let mut seq = Vec::new();
        let mut cur_mods = ModifierType::empty();

        let items = parse_key_sequence_as_items(s)?;

        for item in items {
            match item {
                KeySequenceParseItem::Modifier(m) => cur_mods |= m,
                KeySequenceParseItem::Key(k) => {
                    seq.push(KeySequenceAtom {
                        modifiers: cur_mods,
                        key: k,
                    });
                    cur_mods = ModifierType::empty();
                }
                KeySequenceParseItem::Append => {
                    // TODO
                }
            }
        }

        Ok(KeySequence(seq))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_sequence_parse() {
        gdk::init();

        assert_eq!(
            KeySequence::parse("f"),
            Ok(KeySequence(vec![KeySequenceAtom {
                modifiers: ModifierType::empty(),
                key: keys::f,
            }]))
        );

        assert_eq!(
            KeySequence::parse("<ctrl>f"),
            Ok(KeySequence(vec![KeySequenceAtom {
                modifiers: ModifierType::CONTROL_MASK,
                key: keys::f,
            }]))
        );

        assert_eq!(
            KeySequence::parse("<ctrl>x+f"),
            Ok(KeySequence(vec![
                KeySequenceAtom {
                    modifiers: ModifierType::CONTROL_MASK,
                    key: keys::x,
                },
                KeySequenceAtom {
                    modifiers: ModifierType::empty(),
                    key: keys::f,
                }
            ]))
        );
    }
}
