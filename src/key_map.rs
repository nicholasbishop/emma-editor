use {
    crate::{
        buffer::{Boundary, Direction},
        key_sequence::KeySequence,
        pane_tree,
    },
    anyhow::Error,
    fehler::throws,
    gtk4::gdk::{self, ModifierType},
    std::collections::BTreeMap,
    tracing::{debug, error, instrument},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Move {
    Boundary(Boundary),
    Line,
    Page,
}

#[allow(dead_code)] // TODO
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Action {
    // Insert text for a key press, e.g. pressing the 'a' key inserts
    // an 'a' character into the active buffer.
    Insert(gdk::keys::Key),

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
    pub fn new(name: &'static str) -> KeyMap {
        KeyMap {
            name,
            map: BTreeMap::new(),
        }
    }

    // TODO: use this
    #[throws]
    pub fn from_pairs<'a, I: Iterator<Item = (&'a str, Action)>>(
        name: &'static str,
        iter: I,
    ) -> KeyMap {
        let mut map = KeyMap::new(name);
        for (keys, action) in iter {
            map.parse_and_insert(keys, action)?;
        }
        map
    }

    // TODO: move this to event.rs
    pub fn base() -> KeyMap {
        let mut map = KeyMap::new("base");

        let mut insert =
            |keys, action| map.parse_and_insert(keys, action).unwrap();

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

        insert("<ctrl>s", Action::InteractiveSearch);

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
        self.map.insert(seq, action);
    }

    #[throws]
    pub fn parse_and_insert(&mut self, s: &str, action: Action) {
        self.insert(KeySequence::parse(s)?, action);
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

        // If the sequence's length is 1 and it doesn't have any
        // modifiers (other than shift) then just pass it along; this
        // handles things like pressing the letter 'a' where we just
        // want the default insertion action to occur.
        if seq.0.len() == 1 {
            let atom = &seq.0[0];
            if atom.modifiers.is_empty() {
                return KeyMapLookup::Action(Action::Insert(atom.key.clone()));
            } else if atom.modifiers == ModifierType::SHIFT_MASK {
                return KeyMapLookup::Action(Action::Insert(
                    atom.key.to_upper(),
                ));
            }
        }

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
        // TODO rustify this loop
        for (i, map) in self.0.iter().enumerate().rev() {
            debug!("map: {}", map.name);

            let res = map.lookup(seq);

            // At the bottom of the stack just return the result.
            if i == 0 {
                debug!("bottom of the stack");
                return res;
            }

            // If the sequence either is in, or might be in the
            // current map, return that.
            if matches!(res, KeyMapLookup::Action(_) | KeyMapLookup::Prefix) {
                debug!("map: {}, lookup: {:?}", map.name, res);
                return res;
            }

            // Otherwise, continue up the stack.
        }

        panic!("empty KeyMapStack");
    }

    pub fn push(&mut self, map: Result<KeyMap, Error>) {
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

    #[test]
    fn test_lookup() {
        let mut stack = KeyMapStack::default();

        stack.push(KeyMap::from_pairs(
            "base",
            vec![("<ctrl>a", Action::Exit), ("<ctrl>b", Action::OpenFile)]
                .into_iter(),
        ));

        stack.push(KeyMap::from_pairs(
            "overlay",
            vec![
                ("<ctrl>a", Action::SaveFile),
                ("<ctrl>c", Action::PreviousPane),
            ]
            .into_iter(),
        ));

        // Overlay overrides base.
        assert_eq!(
            stack.lookup(&KeySequence::parse("<ctrl>a").unwrap()),
            KeyMapLookup::Action(Action::SaveFile)
        );

        // Item only in overlay is used.
        assert_eq!(
            stack.lookup(&KeySequence::parse("<ctrl>c").unwrap()),
            KeyMapLookup::Action(Action::PreviousPane)
        );

        // Item only in base is used.
        assert_eq!(
            stack.lookup(&KeySequence::parse("<ctrl>b").unwrap()),
            KeyMapLookup::Action(Action::OpenFile)
        );
    }
}
