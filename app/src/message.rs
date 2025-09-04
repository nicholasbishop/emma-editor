use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Eq, PartialEq, Deserialize, Serialize)]
pub enum ToGtkMsg {
    /// Exit the whole application.
    Close,
}
