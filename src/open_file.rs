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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::theme::Theme;
    use anyhow::Result;
    use std::fs;

    fn path_to_str(p: &Path) -> String {
        p.to_str().unwrap().to_owned()
    }

    #[test]
    fn test_open_file() -> Result<()> {
        // TODO: ick
        Theme::set_current(
            Theme::load_default().expect("failed to load built-in theme"),
        );

        // Create test files.
        let tmp_dir = tempfile::tempdir()?;
        let tmp_dir = tmp_dir.path();
        let tmp_path1 = tmp_dir.join("testfile1");
        fs::write(&tmp_path1, "test data 1\n")?;
        let tmp_path2 = tmp_dir.join("testfile2");
        fs::write(&tmp_path2, "test data 2\n")?;

        let mut open_file = OpenFile::new(&tmp_dir)?;

        // Check the default path.
        assert_eq!(path_to_str(&open_file.path()), path_to_str(&tmp_dir) + "/");

        // Check the initial suggestions.
        assert_eq!(open_file.suggestions(), "testfile1 | testfile2");

        // Modify the path and check suggestions again.
        open_file.buffer_mut().set_text(&path_to_str(&tmp_path1));
        open_file.update_suggestions()?;
        assert_eq!(open_file.suggestions(), "testfile1");

        Ok(())
    }
}
