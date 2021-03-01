use crate::{
    key_map::{Action, KeyMap},
    key_sequence::KeySequence,
};

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum MinibufState {
    Inactive,
    SelectBuffer,
    // TODO this will probably become more general
    OpenFile,
}

pub fn get_minibuf_keymap(state: MinibufState) -> KeyMap {
    let mut map = KeyMap::new();
    match state {
        MinibufState::Inactive => {}
        _ => {
            map.insert(
                KeySequence::parse("<ctrl>i").unwrap(),
                Action::Autocomplete,
            );
            map.insert(KeySequence::parse("<ret>").unwrap(), Action::Confirm);
        }
    }
    map
}
