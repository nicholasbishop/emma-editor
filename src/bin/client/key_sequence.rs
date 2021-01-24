use fehler::{throw, throws};
use gdk::keys::constants as keys;
use gdk::{EventKey, ModifierType};
use glib::translate::FromGlib;
use std::collections::HashMap;

#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
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

fn single_modifier_to_string(m: &ModifierType) -> &'static str {
    if *m == ModifierType::CONTROL_MASK {
        "ctrl"
    } else if *m == ModifierType::SHIFT_MASK {
        "shift"
    } else if *m == ModifierType::MOD1_MASK {
        "alt"
    } else {
        "unknown"
    }
}

#[derive(thiserror::Error, Clone, Debug, Eq, PartialEq)]
pub enum Error {
    #[error("invalid escape sequence: \"\\{0}\"")]
    InvalidEscape(char),

    #[error("invalid name: \"{0}\"")]
    InvalidName(String),

    #[error("unexpected \"+\"")]
    UnexpectedAppend,

    #[error("unexpected modifier {}", single_modifier_to_string(.0))]
    UnexpectedModifier(ModifierType),

    // TODO: improve printing of this error
    #[error("unexpected key {0:?}")]
    UnexpectedKey(gdk::keys::Key),
}

#[derive(Clone, Debug, Eq, PartialEq)]
enum ParseItem {
    Modifier(ModifierType),
    Key(gdk::keys::Key),
    Append,
}

#[throws]
fn parse_key_sequence_as_items(s: &str) -> Vec<ParseItem> {
    enum State {
        Initial,
        InName,
        InEscape,
    }

    let mut state = State::Initial;

    let mut names = HashMap::new();
    names.insert("ctrl", ParseItem::Modifier(ModifierType::CONTROL_MASK));
    names.insert("shift", ParseItem::Modifier(ModifierType::SHIFT_MASK));
    names.insert("alt", ParseItem::Modifier(ModifierType::MOD1_MASK));
    names.insert("esc", ParseItem::Key(keys::Escape));
    names.insert("space", ParseItem::Key(keys::space));

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
                    items.push(ParseItem::Append);
                } else {
                    let keyval = gdk::unicode_to_keyval(c as u32);
                    items
                        .push(ParseItem::Key(gdk::keys::Key::from_glib(keyval)))
                }
            }
            State::InEscape => {
                if c == '<' {
                    items.push(ParseItem::Key(keys::leftanglebracket));
                } else if c == '>' {
                    items.push(ParseItem::Key(keys::rightanglebracket));
                } else if c == '\\' {
                    items.push(ParseItem::Key(keys::backslash));
                } else if c == '+' {
                    items.push(ParseItem::Key(keys::plus));
                } else {
                    throw!(Error::InvalidEscape(c));
                }
                state = State::Initial;
            }
            State::InName => {
                if c == '>' {
                    if let Some(val) = names.get(name.as_str()) {
                        items.push(val.clone());
                    } else {
                        throw!(Error::InvalidName(name));
                    }
                    name.clear();
                    state = State::Initial;
                } else {
                    name.push(c);
                }
            }
        }
    }

    items
}

#[derive(Clone, Debug, Default, Eq, Ord, PartialEq, PartialOrd)]
pub struct KeySequence(pub Vec<KeySequenceAtom>);

impl KeySequence {
    #[throws]
    fn from_items(items: &[ParseItem]) -> KeySequence {
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
                            throw!(Error::UnexpectedModifier(*m));
                        }
                    }
                }
                ParseItem::Key(k) => {
                    seq.push(KeySequenceAtom {
                        modifiers: cur_mods,
                        key: k.clone(),
                    });
                    cur_mods = ModifierType::empty();

                    match state {
                        State::ModOrKeyRequired => {
                            state = State::AppendRequired;
                        }
                        State::AppendRequired => {
                            throw!(Error::UnexpectedKey(k.clone()));
                        }
                    }
                }
                ParseItem::Append => match state {
                    State::ModOrKeyRequired => {
                        throw!(Error::UnexpectedAppend);
                    }
                    State::AppendRequired => {
                        state = State::ModOrKeyRequired;
                    }
                },
            }
        }

        KeySequence(seq)
    }

    #[throws]
    pub fn parse(s: &str) -> KeySequence {
        let items = parse_key_sequence_as_items(s)?;
        Self::from_items(&items)?
    }

    pub fn starts_with(&self, _other: &KeySequence) -> bool {
        todo!();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Once;

    // Used to ensure `gdk::init` is called only once. We run the
    // tests single threaded so the `sync` aspect feels a little
    // silly but seems necessary.
    static INIT_SYNC: Once = Once::new();

    fn init() {
        INIT_SYNC.call_once(|| {
            gdk::init();
        });
    }

    #[test]
    fn test_error_display() {
        init();

        assert_eq!(
            format!(
                "{}",
                Error::UnexpectedModifier(ModifierType::CONTROL_MASK)
            ),
            "unexpected modifier ctrl".to_string()
        );
    }

    #[test]
    fn test_parse_key_sequence() {
        init();

        assert_eq!(
            parse_key_sequence_as_items("aa"),
            Ok(vec![ParseItem::Key(keys::a), ParseItem::Key(keys::a)])
        );

        assert_eq!(
            parse_key_sequence_as_items("<ctrl><shift>"),
            Ok(vec![
                ParseItem::Modifier(ModifierType::CONTROL_MASK),
                ParseItem::Modifier(ModifierType::SHIFT_MASK),
            ])
        );

        // Errors

        assert_eq!(
            parse_key_sequence_as_items("\\a"),
            Err(Error::InvalidEscape('a'))
        );

        assert_eq!(
            parse_key_sequence_as_items("<invalid>"),
            Err(Error::InvalidName("invalid".into()))
        );
    }

    #[test]
    fn test_sequence_from_items() {
        init();

        assert_eq!(
            KeySequence::from_items(&[ParseItem::Key(keys::a)]),
            Ok(KeySequence(vec![KeySequenceAtom {
                modifiers: ModifierType::empty(),
                key: keys::a,
            }]))
        );

        assert_eq!(
            KeySequence::from_items(&[
                ParseItem::Modifier(ModifierType::CONTROL_MASK),
                ParseItem::Key(keys::a)
            ]),
            Ok(KeySequence(vec![KeySequenceAtom {
                modifiers: ModifierType::CONTROL_MASK,
                key: keys::a,
            }]))
        );

        assert_eq!(
            KeySequence::from_items(&[
                ParseItem::Modifier(ModifierType::CONTROL_MASK),
                ParseItem::Key(keys::x),
                ParseItem::Append,
                ParseItem::Key(keys::a),
            ]),
            Ok(KeySequence(vec![
                KeySequenceAtom {
                    modifiers: ModifierType::CONTROL_MASK,
                    key: keys::x,
                },
                KeySequenceAtom {
                    modifiers: ModifierType::empty(),
                    key: keys::a,
                }
            ]))
        );

        assert_eq!(
            KeySequence::from_items(&[
                ParseItem::Modifier(ModifierType::CONTROL_MASK),
                ParseItem::Key(keys::x),
                ParseItem::Append,
                ParseItem::Modifier(ModifierType::CONTROL_MASK),
                ParseItem::Key(keys::a),
            ]),
            Ok(KeySequence(vec![
                KeySequenceAtom {
                    modifiers: ModifierType::CONTROL_MASK,
                    key: keys::x,
                },
                KeySequenceAtom {
                    modifiers: ModifierType::CONTROL_MASK,
                    key: keys::a,
                }
            ]))
        );

        // Errors

        assert_eq!(
            KeySequence::from_items(&[
                ParseItem::Key(keys::a),
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
                ParseItem::Key(keys::a),
                ParseItem::Key(keys::a),
            ]),
            Err(Error::UnexpectedKey(keys::a))
        );
    }
}
