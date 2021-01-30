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
    CloseView,
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
        map.insert(KeySequence::parse("<ctrl>x+0").unwrap(), Action::CloseView);
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
        // modifiers (other than shift) then just pass it along; this
        // handles things like pressing the letter 'a' where we just
        // want the default insertion action to occur.
        if seq.0.len() == 1
            && (seq.0[0].modifiers.is_empty()
                || seq.0[0].modifiers == gdk::ModifierType::SHIFT_MASK)
        {
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

#[derive(Default)]
pub struct KeyMapStack(Vec<KeyMap>);

impl KeyMapStack {
    pub fn lookup(&self, seq: &KeySequence) -> KeyMapLookup {
        // TODO rustify this loop
        for (i, map) in self.0.iter().enumerate().rev() {
            let res = map.lookup(seq);

            // At the bottom of the stack just return the result.
            if i == 0 {
                return res;
            }

            // If the sequence either is in, or might be in the
            // current map, return that.
            if matches!(res, KeyMapLookup::Action(_) | KeyMapLookup::Prefix) {
                return res;
            }

            // Otherwise, continue up the stack.
        }

        panic!("empty KeyMapStack");
    }

    pub fn push(&mut self, map: KeyMap) {
        self.push(map);
    }
}

// TODO: tests
