use emma_app::key::{Key, Modifier, Modifiers};
use gtk4::gdk::{Key as GKey, ModifierType};

pub fn key_from_gdk(key: gtk4::gdk::Key) -> Key {
    match key {
        GKey::BackSpace => Key::Backspace,
        GKey::Escape => Key::Escape,
        GKey::greater => Key::Greater,
        GKey::less => Key::Less,
        GKey::plus => Key::Plus,
        GKey::Return => Key::Return,
        GKey::space => Key::Space,

        GKey::Alt_L | GKey::Alt_R => Key::Modifier(Modifier::Alt),
        GKey::Control_L | GKey::Control_R => Key::Modifier(Modifier::Control),
        GKey::Shift_L | GKey::Shift_R => Key::Modifier(Modifier::Shift),

        _ => {
            if let Some(c) = key.to_unicode() {
                Key::Char(c)
            } else {
                todo!("unhandled key: {key}")
            }
        }
    }
}

pub fn modifiers_from_gdk(modifiers: ModifierType) -> Modifiers {
    Modifiers {
        alt: modifiers.contains(ModifierType::ALT_MASK),
        control: modifiers.contains(ModifierType::CONTROL_MASK),
        shift: modifiers.contains(ModifierType::SHIFT_MASK),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_key_conversion() {
        assert_eq!(key_from_gdk(GKey::a), Key::Char('a'));
        assert_eq!(key_from_gdk(GKey::Escape), Key::Escape);
        assert_eq!(key_from_gdk(GKey::Alt_L), Key::Modifier(Modifier::Alt));
        assert_eq!(
            key_from_gdk(GKey::Control_L),
            Key::Modifier(Modifier::Control)
        );
        assert_eq!(key_from_gdk(GKey::Shift_L), Key::Modifier(Modifier::Shift));
    }

    #[test]
    fn test_modifier_conversion() {
        assert_eq!(modifiers_from_gdk(ModifierType::empty()), Modifiers::new());
        assert_eq!(
            modifiers_from_gdk(
                ModifierType::ALT_MASK
                    | ModifierType::CONTROL_MASK
                    | ModifierType::SHIFT_MASK
            ),
            Modifiers {
                alt: true,
                control: true,
                shift: true
            }
        );
    }
}
