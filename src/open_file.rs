use crate::app::LineHeight;
use crate::buffer::Buffer;
use crate::key_map::{Action, KeyMap};
use crate::pane_tree::{Pane, Rect};
use crate::rope::AbsChar;
use anyhow::Result;
use std::path::{Path, PathBuf};

pub struct OpenFile {
    buffer: Buffer,
    pane: Pane,
    rect: Rect,
    // TODO: this type will probably eventually become more interesting.
    suggestions: String,
}

impl OpenFile {
    pub fn new(default_path: &Path) -> Result<Self> {
        let mut buffer = Buffer::create_empty();
        // TODO: what about non-utf8 paths?
        let mut default_path = default_path.to_str().unwrap().to_owned();
        // That pesky default path doesn't end in a slash.
        default_path += "/";
        buffer.set_text(&default_path);

        let pane = Pane::create_for_widget(buffer.id().clone());
        buffer.set_cursor(&pane, AbsChar(default_path.len()));

        let mut s = Self {
            buffer,
            pane,
            rect: Rect::default(),
            suggestions: String::new(),
        };
        s.update_suggestions()?;
        Ok(s)
    }

    pub fn rect(&self) -> &Rect {
        &self.rect
    }

    pub fn path(&self) -> PathBuf {
        PathBuf::from(self.buffer.text().to_string())
    }

    pub fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    pub fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    pub fn pane(&self) -> &Pane {
        &self.pane
    }

    pub fn pane_buffer_mut(&mut self) -> (&Pane, &mut Buffer) {
        (&self.pane, &mut self.buffer)
    }

    pub fn pane_mut_buffer_mut(&mut self) -> (&mut Pane, &mut Buffer) {
        (&mut self.pane, &mut self.buffer)
    }

    pub fn suggestions(&self) -> &str {
        &self.suggestions
    }

    pub fn recalc_layout(
        &mut self,
        width: f64,
        _height: f64,
        line_height: LineHeight,
    ) {
        self.rect = Rect {
            x: 0.0,
            y: 0.0,
            width,
            height: line_height.0 * 3.0,
        };
        self.pane.set_rect(Rect {
            x: 0.0,
            y: line_height.0,
            width,
            height: line_height.0,
        });
    }

    // TODO: add a trait to generically get keymap for a widget?
    pub fn get_keymap(&self) -> Result<KeyMap> {
        KeyMap::from_pairs(
            "open_file",
            vec![
                ("<ctrl>i", Action::Autocomplete),
                ("<ret>", Action::Confirm),
                ("<ctrl>m", Action::Confirm),
            ]
            .into_iter(),
        )
    }

    pub fn update_suggestions(&mut self) -> Result<()> {
        // TODO: this is a very simple completion that is
        // minimally helpful.
        let mut path = self.path().to_str().unwrap().to_owned();
        path.push('*');
        // Arbitrarily grab a few options.
        let completions: Vec<_> = glob::glob(&path)?
            .into_iter()
            .take(100)
            .map(|p| {
                p.unwrap().file_name().unwrap().to_str().unwrap().to_owned()
            })
            .collect();

        self.suggestions = completions.join(" | ");

        Ok(())
    }
}
