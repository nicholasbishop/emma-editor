use crate::LineHeight;
use crate::buffer::Buffer;
use crate::key_map::KeyMap;
use crate::pane_tree::{Pane, Rect};
use crate::path_chooser::PathChooser;
use crate::search_widget::SearchWidget;
use crate::widget::Widget;
use anyhow::Result;

pub enum Overlay {
    OpenFile(PathChooser),
    Search(SearchWidget),
}

impl Overlay {
    pub fn prompt(&self) -> &'static str {
        match self {
            Self::OpenFile(_) => "Open file:",
            Self::Search(_) => "Search:",
        }
    }

    fn widget(&self) -> &dyn Widget {
        match self {
            Self::OpenFile(w) => w,
            Self::Search(w) => w,
        }
    }

    fn widget_mut(&mut self) -> &mut dyn Widget {
        match self {
            Self::OpenFile(w) => w,
            Self::Search(w) => w,
        }
    }
}

impl Widget for Overlay {
    fn get_keymap(&self) -> Result<KeyMap> {
        self.widget().get_keymap()
    }

    fn buffer(&self) -> &Buffer {
        self.widget().buffer()
    }

    fn buffer_mut(&mut self) -> &mut Buffer {
        self.widget_mut().buffer_mut()
    }

    fn pane_buffer_mut(&mut self) -> (&Pane, &mut Buffer) {
        self.widget_mut().pane_buffer_mut()
    }

    fn pane(&self) -> &Pane {
        self.widget().pane()
    }

    fn pane_mut_buffer_mut(&mut self) -> (&mut Pane, &mut Buffer) {
        self.widget_mut().pane_mut_buffer_mut()
    }

    fn recalc_layout(&mut self, width: f64, line_height: LineHeight) {
        self.widget_mut().recalc_layout(width, line_height);
    }

    fn rect(&self) -> &Rect {
        self.widget().rect()
    }
}
