use {
    crate::{
        key_map::{Action, KeyMap},
        key_sequence::KeySequence,
    },
    gtk4::{self as gtk, prelude::*},
    std::{ffi::OsString, fs, path::Path},
};

const INPUT_START: &str = "input-start";

/// Get the names of the children of `dir`. All errors are silently
/// ignored.
fn list_dir_no_error(dir: &Path) -> Vec<OsString> {
    if let Ok(iter) = fs::read_dir(dir) {
        iter.filter_map(|entry| entry.ok().map(|entry| entry.file_name()))
            .collect()
    } else {
        dbg!("err");
        Vec::new()
    }
}

fn longest_shared_prefix(inputs: &[&str]) -> String {
    // TODO: I'm sure there's a much more efficient way to do this,
    // maybe even a pre-existing crate we can use.
    let mut longest_prefix = String::new();
    for s in inputs {
        for i in 0..s.len() {
            let prefix = &s[..i];
            // Only interested in this prefix if it's longer than the
            // current longest prefix.
            if prefix.len() > longest_prefix.len() {
                // Check if this prefix is in all inputs.
                if inputs.iter().all(|s| s.starts_with(prefix)) {
                    longest_prefix = prefix.into();
                }
            }
        }
    }
    longest_prefix
}

#[derive(Clone, Copy, Eq, PartialEq)]
pub enum MinibufState {
    Inactive,
    SelectBuffer,
    // TODO this will probably become more general
    OpenFile,
}

pub struct Minibuf {
    view: gtk::TextView,
    state: MinibufState,
}

impl Minibuf {
    pub fn new() -> Minibuf {
        let view = gtk::TextView::new();
        view.set_size_request(-1, 26); // TODO

        Minibuf {
            view,
            state: MinibufState::Inactive,
        }
    }

    pub fn widget(&self) -> gtk::Widget {
        self.view.clone().upcast()
    }

    pub fn set_state(&mut self, state: MinibufState) {
        self.state = state;
    }

    pub fn state(&self) -> MinibufState {
        self.state
    }

    // TODO: think about whether widget focus is the right thing to
    // use here.
    pub fn has_focus(&self) -> bool {
        self.view.has_focus()
    }

    pub fn grab_focus(&self) {
        self.view.grab_focus();
    }

    pub fn start_input(&self, prompt: &str, def: &str) {
        self.grab_focus();

        let buf = self.view.get_buffer();

        // Get or create prompt tag.
        let tag_name = "prompt";
        let tag = buf.get_tag_table().lookup(tag_name);
        let tag = if let Some(tag) = tag {
            tag
        } else {
            let tag = gtk::TextTag::new(Some("prompt"));
            tag.set_property_editable(false);
            tag.set_property_foreground(Some("#edd400"));
            buf.get_tag_table().add(&tag);
            tag
        };

        // Add prompt text and apply tag.
        buf.set_text(prompt);
        let start = buf.get_start_iter();
        let mut prompt_end = buf.get_iter_at_offset(prompt.len() as i32);
        buf.apply_tag(&tag, &start, &prompt_end);

        // Insert mark to indicate the beginning of the user
        // input.
        if let Some(mark) = buf.get_mark(INPUT_START) {
            buf.delete_mark(&mark);
        }
        let left_gravity = true;
        buf.create_mark(Some(INPUT_START), &prompt_end, left_gravity);

        buf.insert(&mut prompt_end, def);
    }

    pub fn cancel(&mut self) {
        match self.state {
            MinibufState::Inactive => {}
            _ => {
                let buf = self.view.get_buffer();

                buf.set_text("");

                self.state = MinibufState::Inactive;
            }
        }
    }

    pub fn get_minibuf_input(&self) -> String {
        let buf = self.view.get_buffer();

        // TODO: dedup
        let mark = buf.get_mark(INPUT_START).unwrap();
        let start = buf.get_iter_at_mark(&mark);
        let end = buf.get_end_iter();

        let text = buf.get_text(&start, &end, false);
        text.to_string()
    }

    pub fn take_input(&self) -> String {
        let buf = self.view.get_buffer();

        // TODO: dedup
        let mark = buf.get_mark(INPUT_START).unwrap();
        let start = buf.get_iter_at_mark(&mark);
        let end = buf.get_end_iter();

        let text = buf.get_text(&start, &end, false);

        buf.set_text("");

        text.to_string()
    }

    /// Replace the text after the prompt.
    pub fn set_input(&self, text: &str) {
        let buf = self.view.get_buffer();

        // TODO: dedup
        let mark = buf.get_mark(INPUT_START).unwrap();
        let mut start = buf.get_iter_at_mark(&mark);
        let mut end = buf.get_end_iter();

        buf.delete(&mut start, &mut end);
        buf.insert(&mut start, text);
    }

    pub fn get_keymap(&self) -> KeyMap {
        let mut map = KeyMap::new();
        match self.state {
            MinibufState::Inactive => {}
            _ => {
                map.insert(
                    KeySequence::parse("<ctrl>i").unwrap(),
                    Action::Autocomplete,
                );
                map.insert(
                    KeySequence::parse("<ret>").unwrap(),
                    Action::Confirm,
                );
            }
        }
        map
    }

    pub fn autocomplete(&self) {
        match self.state {
            MinibufState::Inactive => {}
            MinibufState::SelectBuffer => {
                // TODO
            }
            MinibufState::OpenFile => {
                let text = self.get_minibuf_input();
                let cur_path = Path::new(&text);

                // Get the parent directory (the contents of which
                // should be listed), as well as the prefix (the
                // portion of the name within the parent directory
                // that has already been written).
                let prefix;
                let dir;
                if text.ends_with('/') {
                    prefix = None;
                    dir = cur_path;
                } else {
                    prefix = cur_path.file_name();
                    dir = cur_path.parent().unwrap_or(cur_path);
                };

                // Get the names of the children of `dir`.
                let children = list_dir_no_error(dir);

                // Convert to UTF-8. These names end up typed in a
                // TextBuffer, so we don't have a good way to handle
                // non-UTF-8 paths right now.
                let mut children: Vec<&str> =
                    children.iter().filter_map(|path| path.to_str()).collect();
                let prefix: Option<&str> =
                    prefix.and_then(|prefix| prefix.to_str());

                // Filter out the children that don't start with `prefix`.
                if let Some(prefix) = prefix {
                    children.retain(|name| name.starts_with(prefix));
                }

                children.sort_unstable();

                dbg!(&children);

                // TODO: look into that path library that assumes utf8

                // If there's just one completion, fill it in. If that
                // path is a directory, add a '/' to the end.
                if children.len() == 1 {
                    let new_path = dir.join(&children[0]);
                    let mut new_path_str =
                        new_path.to_str().unwrap().to_string();
                    if new_path.is_dir() {
                        new_path_str.push('/');
                    }
                    self.set_input(&new_path_str);
                } else if children.len() >= 2 {
                    // If all completions have a shared prefix, fill
                    // it in.
                    let longest_prefix = longest_shared_prefix(&children);
                    let new_path = dir.join(longest_prefix);
                    let new_path_str = new_path.to_str().unwrap().to_string();
                    self.set_input(&new_path_str);
                }
            }
        }
    }
}
