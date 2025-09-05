use crate::buffer::BufferId;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub enum ToGtkMsg {
    /// Exit the whole application.
    Close,

    // TODO: not sure if we want something more specific. This is used
    // for appending subprocess output to a buffer.
    AppendToBuffer(BufferId, String),
}
