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

        let mut insert = |keys, action| {
            map.insert(KeySequence::parse(keys).unwrap(), action)
        };

        // TODO: for now make it easy to quit
        insert("<esc>", Action::Exit);

        insert(
            "<ctrl>b",
            Action::Move(Move::Boundary(Boundary::Grapheme), Direction::Dec),
        );
        insert(
            "<ctrl>f",
            Action::Move(Move::Boundary(Boundary::Grapheme), Direction::Inc),
        );
        insert("<ctrl>p", Action::Move(Move::Line, Direction::Dec));
        insert("<ctrl>n", Action::Move(Move::Line, Direction::Inc));
        insert(
            "<ctrl>a",
            Action::Move(Move::Boundary(Boundary::LineEnd), Direction::Dec),
        );
        insert(
            "<ctrl>e",
            Action::Move(Move::Boundary(Boundary::LineEnd), Direction::Inc),
        );
        insert("<alt>v", Action::Move(Move::Page, Direction::Dec));
        insert("<ctrl>v", Action::Move(Move::Page, Direction::Inc));
        insert(
            "<alt><shift><less>",
            Action::Move(Move::Boundary(Boundary::BufferEnd), Direction::Dec),
        );
        insert(
            "<alt><shift><greater>",
            Action::Move(Move::Boundary(Boundary::BufferEnd), Direction::Inc),
        );

        insert(
            "<backspace>",
            Action::Delete(Boundary::Grapheme, Direction::Dec),
        );
        insert(
            "<ctrl>d",
            Action::Delete(Boundary::Grapheme, Direction::Inc),
        );

        insert("<ctrl>/", Action::Undo);
        insert("<ctrl><shift>?", Action::Redo);

        insert("<ctrl>x+k", Action::DeleteBuffer);
        insert("<ctrl>x+<ctrl>f", Action::OpenFile);
        insert("<ctrl>x+<ctrl>s", Action::SaveFile);
        insert("<ctrl><shift>j", Action::PreviousPane);
        insert("<ctrl><shift>k", Action::NextPane);
        insert(
            "<ctrl>x+2",
            Action::SplitPane(pane_tree::Orientation::Vertical),
        );
        insert(
            "<ctrl>x+3",
            Action::SplitPane(pane_tree::Orientation::Horizontal),
        );
        insert("<ctrl>x+0", Action::ClosePane);
        insert("<ctrl>c+<ctrl>s", Action::OpenShell);
        insert("<ctrl>x+b", Action::SwitchToBuffer);
        // TODO: make this generic so that any key sequence can be
        // canceled with ctrl+g.
        insert("<ctrl>g", Action::Cancel);
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
