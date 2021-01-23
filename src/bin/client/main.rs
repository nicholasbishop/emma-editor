use gdk::keys::constants as keys;
use gdk::{EventKey, ModifierType};
use gio::prelude::*;
use glib::translate::FromGlib;
use gtk::prelude::*;
use std::collections::HashMap;
use std::env;

#[derive(Clone, Debug, Eq, PartialEq)]
struct KeySequenceAtom {
    modifiers: ModifierType,
    key: gdk::keys::Key,
}

impl KeySequenceAtom {
    fn from_event(e: &EventKey) -> KeySequenceAtom {
        KeySequenceAtom {
            modifiers: e.get_state(),
            key: e.get_keyval(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct KeySequence(Vec<KeySequenceAtom>);

#[derive(Clone, Debug, Eq, PartialEq)]
enum KeySequenceParseError {
    InvalidEscape(char),
    InvalidName(String),
}

impl KeySequence {
    // TODO: change to a Result
    fn parse(s: &str) -> Result<KeySequence, KeySequenceParseError> {
        enum State {
            Initial,
            InName,
            InEscape,
        }

        #[derive(Clone)]
        enum Item {
            Modifier(ModifierType),
            Key(gdk::keys::Key),
        }

        let mut state = State::Initial;

        let mut names = HashMap::new();
        names.insert("ctrl", Item::Modifier(ModifierType::CONTROL_MASK));
        names.insert("shift", Item::Modifier(ModifierType::SHIFT_MASK));
        names.insert("alt", Item::Modifier(ModifierType::MOD1_MASK));
        names.insert("esc", Item::Key(keys::Escape));
        names.insert("space", Item::Key(keys::space));

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
                        // Nothing to do, this is just an explicit way
                        // to break up the key sequence
                    } else {
                        let keyval = gdk::unicode_to_keyval(c as u32);
                        items.push(Item::Key(gdk::keys::Key::from_glib(keyval)))
                    }
                }
                State::InEscape => {
                    if c == '<' {
                        items.push(Item::Key(keys::leftanglebracket));
                    } else if c == '>' {
                        items.push(Item::Key(keys::rightanglebracket));
                    } else if c == '\\' {
                        items.push(Item::Key(keys::backslash));
                    } else if c == '+' {
                        items.push(Item::Key(keys::plus));
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
                            return Err(KeySequenceParseError::InvalidName(
                                name,
                            ));
                        }
                        state = State::Initial;
                    } else {
                        name.push(c);
                    }
                }
            }
        }

        let mut seq = Vec::new();
        let mut cur_mods = ModifierType::empty();

        for item in items {
            match item {
                Item::Modifier(m) => cur_mods |= m,
                Item::Key(k) => {
                    seq.push(KeySequenceAtom {
                        modifiers: cur_mods,
                        key: k,
                    });
                    cur_mods = ModifierType::empty();
                }
            }
        }

        Ok(KeySequence(seq))
    }
}

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("emma");
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(350, 70);

    let text = gtk::TextView::new();

    window.add(&text);

    window.add_events(gdk::EventMask::KEY_PRESS_MASK);
    window.connect_key_press_event(|_, e| {
        // Ignore lone modifier presses.
        if e.get_is_modifier() {
            return Inhibit(false);
        }

        let atom = KeySequenceAtom::from_event(e);

        if atom.key == keys::Escape {
            // TODO: for now make it easy to quit
            std::process::exit(0);
        } else if atom.modifiers == gdk::ModifierType::CONTROL_MASK
            && atom.key == keys::f
        {
            dbg!("C-f");
            Inhibit(true)
        } else {
            Inhibit(false)
        }
    });

    window.show_all();
}

fn main() {
    let application = gtk::Application::new(
        Some("com.github.gtk-rs.examples.basic"),
        Default::default(),
    )
    .expect("Initialization failed...");

    application.connect_activate(|app| {
        build_ui(app);
    });

    application.run(&env::args().collect::<Vec<_>>());
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
    }
}
