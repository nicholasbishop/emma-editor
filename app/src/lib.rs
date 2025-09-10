#![expect(clippy::new_without_default)]

mod key_map;
mod key_sequence;
mod path_chooser;
mod process;
mod search_widget;
mod shell;
mod util;

pub mod action;
pub mod buffer;
pub mod config;
pub mod grapheme;
pub mod key;
pub mod message;
pub mod overlay;
pub mod pane_tree;
pub mod rope;
pub mod state;
pub mod theme;
pub mod widget;

// TODO: location
#[derive(Clone, Copy, Debug)]
pub struct LineHeight(pub f64);
