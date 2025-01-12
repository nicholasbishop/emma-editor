use crate::buffer::{Boundary, Direction};
use crate::key_sequence::KeySequence;
use crate::pane_tree;
use anyhow::Result;
use iced::keyboard::{Key, Modifiers};
use std::collections::BTreeMap;
use tracing::{debug, error, instrument};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Move {
    Boundary(Boundary),
    Line,
    Page,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    // Used in tests.
    #[cfg(test)]
    Test(&'static str),

    // Insert text for a key press, e.g. pressing the 'a' key inserts
    // an 'a' character into the active buffer.
    Insert(String),

    // Insert a new line after the cursor. The cursor position is left
    // unchanged.
    InsertLineAfter,

    Exit,
    OpenFile,
    SaveFile,
    PreviousPane,
    NextPane,
    SplitPane(pane_tree::Orientation),
    ClosePane,
    Confirm,
    OpenShell,
    InteractiveSearch,
    SearchNext,

    Undo,
    Redo,

    /// Delete text in the active pane.
    Delete(Boundary, Direction),

    /// Delete the buffer in the active pane.
    DeleteBuffer,

    /// Move the cursor in the active pane (or minibuf).
    Move(Move, Direction),

    /// Interactively switch to a different buffer.
    SwitchToBuffer,

    /// Cancel the current operation, e.g. opening a file from the
    /// minibuf.
    Cancel,

    /// Try to autocomplete something in the minibuf, e.g. a file
    /// path.
    Autocomplete,
}

#[derive(Debug, Eq, PartialEq)]
pub enum KeyMapLookup {
    Action(Action),
    Prefix,
    BadSequence,
}

#[derive(Clone, Debug)]
pub struct KeyMap {
    name: &'static str,
    map: BTreeMap<KeySequence, Action>,
}

impl KeyMap {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            map: BTreeMap::new(),
        }
    }

    pub fn from_pairs<'a, I: Iterator<Item = (&'a str, Action)>>(
        name: &'static str,
        iter: I,
    ) -> Result<Self> {
        let mut map = Self::new(name);
        for (keys, action) in iter {
            map.parse_and_insert(keys, action)?;
        }
        Ok(map)
    }

    // TODO: move this to event.rs
    pub fn base() -> Result<Self> {
        Self::from_pairs(
            "base",
            vec![
                // TODO: for now make it easy to quit
                ("<esc>", Action::Exit),
                ("<ctrl>o", Action::InsertLineAfter),
                (
                    "<ctrl>b",
                    Action::Move(
                        Move::Boundary(Boundary::Grapheme),
                        Direction::Dec,
                    ),
                ),
                (
                    "<ctrl>f",
                    Action::Move(
                        Move::Boundary(Boundary::Grapheme),
                        Direction::Inc,
                    ),
                ),
                ("<ctrl>p", Action::Move(Move::Line, Direction::Dec)),
                ("<ctrl>n", Action::Move(Move::Line, Direction::Inc)),
                (
                    "<ctrl>a",
                    Action::Move(
                        Move::Boundary(Boundary::LineEnd),
                        Direction::Dec,
                    ),
                ),
                (
                    "<ctrl>e",
                    Action::Move(
                        Move::Boundary(Boundary::LineEnd),
                        Direction::Inc,
                    ),
                ),
                ("<alt>v", Action::Move(Move::Page, Direction::Dec)),
                ("<ctrl>v", Action::Move(Move::Page, Direction::Inc)),
                (
                    "<alt><shift><less>",
                    Action::Move(
                        Move::Boundary(Boundary::BufferEnd),
                        Direction::Dec,
                    ),
                ),
                (
                    "<alt><shift><greater>",
                    Action::Move(
                        Move::Boundary(Boundary::BufferEnd),
                        Direction::Inc,
                    ),
                ),
                (
                    "<backspace>",
                    Action::Delete(Boundary::Grapheme, Direction::Dec),
                ),
                (
                    "<ctrl>d",
                    Action::Delete(Boundary::Grapheme, Direction::Inc),
                ),
                ("<ctrl>s", Action::InteractiveSearch),
                ("<ctrl>/", Action::Undo),
                ("<ctrl><shift>?", Action::Redo),
                ("<ctrl>x+k", Action::DeleteBuffer),
                ("<ctrl>x+<ctrl>f", Action::OpenFile),
                ("<ctrl>x+<ctrl>s", Action::SaveFile),
                ("<ctrl><shift>j", Action::PreviousPane),
                ("<ctrl><shift>k", Action::NextPane),
                (
                    "<ctrl>x+2",
                    Action::SplitPane(pane_tree::Orientation::Vertical),
                ),
                (
                    "<ctrl>x+3",
                    Action::SplitPane(pane_tree::Orientation::Horizontal),
                ),
                ("<ctrl>x+0", Action::ClosePane),
                ("<ctrl>c+<ctrl>s", Action::OpenShell),
                ("<ctrl>x+b", Action::SwitchToBuffer),
                // TODO: make this generic so that any key sequence can be
                // canceled with ctrl+g.
                ("<ctrl>g", Action::Cancel),
            ]
            .into_iter(),
        )
    }

    pub fn insert(&mut self, seq: KeySequence, action: Action) {
        self.map.insert(seq, action);
    }

    pub fn parse_and_insert(&mut self, s: &str, action: Action) -> Result<()> {
        self.insert(KeySequence::parse(s)?, action);
        Ok(())
    }

    pub fn lookup(&self, seq: &KeySequence) -> KeyMapLookup {
        // First check for the exact sequence
        if let Some(action) = self.map.get(seq) {
            return KeyMapLookup::Action(action.clone());
        }

        // Then check if the sequence could be a prefix for something
        // in the map.
        if self.contains_prefix(seq) {
            return KeyMapLookup::Prefix;
        }

        // At this point we know the sequence is not in the map.

        // TODO: special "<ctrl>g" type thing to kill any sequence

        KeyMapLookup::BadSequence
    }

    /// Check if `seq` matches a prefix.
    fn contains_prefix(&self, seq: &KeySequence) -> bool {
        // TODO: should be able to make this more efficient by
        // starting the search at the appropriate place.
        for k in self.map.keys() {
            if k.starts_with(seq) {
                return true;
            }
        }
        false
    }
}

#[derive(Debug, Default)]
pub struct KeyMapStack(Vec<KeyMap>);

impl KeyMapStack {
    #[instrument(skip(self))]
    pub fn lookup(&self, seq: &KeySequence) -> KeyMapLookup {
        for map in self.0.iter().rev() {
            debug!("map: {}", map.name);

            let res = map.lookup(seq);

            // If the sequence either is in, or might be in the
            // current map, return that.
            if matches!(res, KeyMapLookup::Action(_) | KeyMapLookup::Prefix) {
                debug!("map: {}, lookup: {:?}", map.name, res);
                return res;
            }

            // Otherwise, continue up the stack.
        }

        // None of the keymaps had an explicit match.

        // If the sequence's length is 1 and it doesn't have any
        // modifiers (other than shift) then just pass it along; this
        // handles things like pressing the letter 'a' where we just
        // want the default insertion action to occur.
        if seq.0.len() == 1 {
            let atom = &seq.0[0];
            if let Key::Character(c) = &atom.key {
                let mut c = c.to_string();
                if atom.modifiers.is_empty() {
                    return KeyMapLookup::Action(Action::Insert(c));
                } else if atom.modifiers == Modifiers::SHIFT {
                    return KeyMapLookup::Action(Action::Insert(
                        c.to_uppercase(),
                    ));
                }
            }
        }

        KeyMapLookup::BadSequence
    }

    pub fn push(&mut self, map: Result<KeyMap>) {
        match map {
            Ok(map) => self.0.push(map),
            Err(err) => {
                // TODO: display in UI
                error!("invalid map: {}", err)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_sequence::charkey;

    #[test]
    fn test_lookup() {
        let mut stack = KeyMapStack::default();

        stack.push(KeyMap::from_pairs(
            "base",
            vec![
                ("<ctrl>a", Action::Test("a base")),
                ("<ctrl>b", Action::Test("b base")),
                ("x", Action::Test("x base")),
            ]
            .into_iter(),
        ));

        stack.push(KeyMap::from_pairs(
            "overlay",
            vec![
                ("<ctrl>a", Action::Test("a overlay")),
                ("<ctrl>c", Action::Test("c overlay")),
            ]
            .into_iter(),
        ));

        // Overlay overrides base.
        assert_eq!(
            stack.lookup(&KeySequence::parse("<ctrl>a").unwrap()),
            KeyMapLookup::Action(Action::Test("a overlay"))
        );

        // Item only in overlay is used.
        assert_eq!(
            stack.lookup(&KeySequence::parse("<ctrl>c").unwrap()),
            KeyMapLookup::Action(Action::Test("c overlay"))
        );

        // Item only in base is used.
        assert_eq!(
            stack.lookup(&KeySequence::parse("<ctrl>b").unwrap()),
            KeyMapLookup::Action(Action::Test("b base"))
        );

        // Single-character sequence properly falls through the overlay
        // keymap and is found in the base keymap.
        assert_eq!(
            stack.lookup(&KeySequence::parse("x").unwrap()),
            KeyMapLookup::Action(Action::Test("x base"))
        );

        // Simple sequence not in any keymap.
        assert_eq!(
            stack.lookup(&KeySequence::parse("y").unwrap()),
            KeyMapLookup::Action(Action::Insert("y".to_owned()))
        );

        // Sequence not in any keymap.
        assert_eq!(
            stack.lookup(&KeySequence::parse("<ctrl>x").unwrap()),
            KeyMapLookup::BadSequence,
        );
    }
}
