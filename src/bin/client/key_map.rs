use crate::key_sequence::KeySequence;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Exit,
    OpenFile,
    PreviousView,
    NextView,
    SplitHorizontal,
    SplitVertical,
}

pub enum KeyMapLookup {
    Action(Action),
    Prefix,
    NoEntry,
    BadSequence,
}

#[derive(Clone, Debug, Default)]
pub struct KeyMap(BTreeMap<KeySequence, Action>);

impl KeyMap {
    pub fn new() -> KeyMap {
        let mut map = KeyMap::default();
        // TODO: for now make it easy to quit
        map.insert(KeySequence::parse("<esc>").unwrap(), Action::Exit);
        map.insert(
            KeySequence::parse("<ctrl>x+<ctrl>f").unwrap(),
            Action::OpenFile,
        );
        map.insert(
            KeySequence::parse("<ctrl><shift>j").unwrap(),
            Action::PreviousView,
        );
        map.insert(
            KeySequence::parse("<ctrl><shift>k").unwrap(),
            Action::NextView,
        );
        map.insert(
            KeySequence::parse("<ctrl>x+2").unwrap(),
            Action::SplitVertical,
        );
        map.insert(
            KeySequence::parse("<ctrl>x+3").unwrap(),
            Action::SplitHorizontal,
        );
        map
    }

    fn insert(&mut self, seq: KeySequence, action: Action) {
        self.0.insert(seq, action);
    }

    pub fn lookup(&self, seq: &KeySequence) -> KeyMapLookup {
        // First check for the exact sequence
        if let Some(action) = self.0.get(seq) {
            return KeyMapLookup::Action(*action);
        }

        // Then check if the sequence could be a prefix for something
        // in the map.
        if self.contains_prefix(seq) {
            return KeyMapLookup::Prefix;
        }

        // At this point we know the sequence is not in the map.

        // If the sequence's length is 1 and it doesn't have any
        // modifiers then just pass it along; this handles things like
        // pressing the letter 'a' where we just want the default
        // insertion action to occur.
        if seq.0.len() == 1 && seq.0[0].modifiers.is_empty() {
            return KeyMapLookup::NoEntry;
        }

        // TODO: special "<ctrl>g" type thing to kill any sequence

        KeyMapLookup::BadSequence
    }

    /// Check if `seq` matches a prefix.
    fn contains_prefix(&self, seq: &KeySequence) -> bool {
        // TODO: should be able to make this more efficient by
        // starting the search at the appropriate place.
        for k in self.0.keys() {
            if k.starts_with(&seq) {
                return true;
            }
        }
        false
    }
}

// TODO: tests
