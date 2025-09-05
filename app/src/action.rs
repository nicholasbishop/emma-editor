use crate::buffer::BufferId;
use crate::pane_tree::Orientation;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum Direction {
    Dec,
    Inc,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum Boundary {
    Grapheme,
    LineEnd,
    BufferEnd,
    // TODO:
    // Subword,
    // Word,
    // LineEndExcludingWhitespace,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum Move {
    Boundary(Boundary),
    Line,
    Page,
}

#[derive(Clone, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum Action {
    // Insert text for a key press, e.g. pressing the 'a' key inserts
    // an 'a' character into the active buffer.
    Insert(char),

    // Insert a new line after the cursor. The cursor position is left
    // unchanged.
    InsertLineAfter,

    Exit,
    OpenFile,
    SaveFile,
    PreviousPane,
    NextPane,
    SplitPane(Orientation),
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

    /// Move the cursor in the active pane.
    Move(Move, Direction),

    /// Interactively switch to a different buffer.
    SwitchToBuffer,

    /// Cancel the current operation, e.g. opening a file.
    Cancel,

    /// Try to autocomplete something, e.g. a file path.
    Autocomplete,

    RunNonInteractiveProcess,

    // TODO: maybe not the right level of specificity
    AppendToBuffer(BufferId, String),
}
