pub mod buffer;
pub mod config;
pub mod grapheme;
pub mod key;
pub mod key_map;
pub mod key_sequence;
pub mod overlay;
pub mod pane_tree;
pub mod path_chooser;
pub mod process;
pub mod rope;
pub mod search_widget;
pub mod shell;
pub mod theme;
pub mod util;
pub mod widget;

// TODO: location
#[derive(Clone, Copy, Debug)]
pub struct LineHeight(pub f64);
