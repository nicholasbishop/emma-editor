use {
    crate::{
        buffer::{Boundary, Direction},
        key_sequence::KeySequence,
        pane_tree,
    },
    gtk4::gdk::ModifierType,
    std::collections::BTreeMap,
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Move {
    Boundary(Boundary),
    Line,
    Page,
}

#[allow(dead_code)] // TODO
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    Exit,
    OpenFile,
    SaveFile,
    PreviousPane,
    NextPane,
    SplitPane(pane_tree::Orientation),
    ClosePane,
    Confirm,
    OpenShell,

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
            KeySequence::parse("<ctrl>b").unwrap(),
            Action::Move(Move::Boundary(Boundary::Grapheme), Direction::Dec),
        );
        map.insert(
            KeySequence::parse("<ctrl>f").unwrap(),
            Action::Move(Move::Boundary(Boundary::Grapheme), Direction::Inc),
        );
        map.insert(
            KeySequence::parse("<ctrl>p").unwrap(),
            Action::Move(Move::Line, Direction::Dec),
        );
        map.insert(
            KeySequence::parse("<ctrl>n").unwrap(),
            Action::Move(Move::Line, Direction::Inc),
        );
        map.insert(
            KeySequence::parse("<ctrl>a").unwrap(),
            Action::Move(Move::Boundary(Boundary::LineEnd), Direction::Dec),
        );
        map.insert(
            KeySequence::parse("<ctrl>e").unwrap(),
            Action::Move(Move::Boundary(Boundary::LineEnd), Direction::Inc),
        );
        map.insert(
            KeySequence::parse("<alt>v").unwrap(),
            Action::Move(Move::Page, Direction::Dec),
        );
        map.insert(
            KeySequence::parse("<ctrl>v").unwrap(),
            Action::Move(Move::Page, Direction::Inc),
        );
        map.insert(
            KeySequence::parse("<alt><shift><less>").unwrap(),
            Action::Move(Move::Boundary(Boundary::BufferEnd), Direction::Dec),
        );
        map.insert(
            KeySequence::parse("<alt><shift><greater>").unwrap(),
            Action::Move(Move::Boundary(Boundary::BufferEnd), Direction::Inc),
        );

        map.insert(
            KeySequence::parse("<backspace>").unwrap(),
            Action::Delete(Boundary::Grapheme, Direction::Dec),
        );
        map.insert(
            KeySequence::parse("<ctrl>d").unwrap(),
            Action::Delete(Boundary::Grapheme, Direction::Inc),
        );

        map.insert(
            KeySequence::parse("<ctrl>x+k").unwrap(),
            Action::DeleteBuffer,
        );
        map.insert(
            KeySequence::parse("<ctrl>x+<ctrl>f").unwrap(),
            Action::OpenFile,
        );
        map.insert(
            KeySequence::parse("<ctrl>x+<ctrl>s").unwrap(),
            Action::SaveFile,
        );
        map.insert(
            KeySequence::parse("<ctrl><shift>j").unwrap(),
            Action::PreviousPane,
        );
        map.insert(
            KeySequence::parse("<ctrl><shift>k").unwrap(),
            Action::NextPane,
        );
        map.insert(
            KeySequence::parse("<ctrl>x+2").unwrap(),
            Action::SplitPane(pane_tree::Orientation::Vertical),
        );
        map.insert(
            KeySequence::parse("<ctrl>x+3").unwrap(),
            Action::SplitPane(pane_tree::Orientation::Horizontal),
        );
        map.insert(KeySequence::parse("<ctrl>x+0").unwrap(), Action::ClosePane);
        map.insert(
            KeySequence::parse("<ctrl>c+<ctrl>s").unwrap(),
            Action::OpenShell,
        );
        map.insert(
            KeySequence::parse("<ctrl>x+b").unwrap(),
            Action::SwitchToBuffer,
        );
        // TODO: make this generic so that any key sequence can be
        // canceled with ctrl+g.
        map.insert(KeySequence::parse("<ctrl>g").unwrap(), Action::Cancel);
        map
    }

    pub fn insert(&mut self, seq: KeySequence, action: Action) {
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
                || seq.0[0].modifiers == ModifierType::SHIFT_MASK)
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
        self.0.push(map);
    }
}

// TODO: tests
