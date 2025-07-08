use crate::app::LineHeight;
use crate::buffer::Buffer;
use crate::key_map::{Action, KeyMap};
use crate::pane_tree::{Pane, Rect};
use crate::rope::AbsChar;
use crate::widget::Widget;
use anyhow::Result;

// TODO
pub struct SearchWidget {
    buffer: Buffer,
    pane: Pane,
    rect: Rect,
}

impl SearchWidget {
    pub fn new() -> Self {
        // TODO: dedup with PathChooser, make harder to get wrong (if
        // cursor is not set, errors occur).
        let mut buffer = Buffer::create_empty();
        let pane = Pane::create_for_widget(buffer.id().clone());
        buffer.set_cursor(pane.id(), AbsChar::default());
        Self {
            buffer,
            pane,
            rect: Rect::default(),
        }
    }

    pub fn text(&self) -> String {
        self.buffer.text().to_string()
    }
}

impl Widget for SearchWidget {
    fn get_keymap(&self) -> Result<KeyMap> {
        KeyMap::from_pairs(
            "search",
            vec![
                ("<ret>", Action::Confirm),
                ("<ctrl>m", Action::Confirm),
                ("<ctrl>s", Action::SearchNext),
            ]
            .into_iter(),
        )
    }

    fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    fn pane(&self) -> &Pane {
        &self.pane
    }

    fn pane_buffer_mut(&mut self) -> (&Pane, &mut Buffer) {
        (&self.pane, &mut self.buffer)
    }

    fn pane_mut_buffer_mut(&mut self) -> (&mut Pane, &mut Buffer) {
        (&mut self.pane, &mut self.buffer)
    }

    fn recalc_layout(&mut self, width: f64, line_height: LineHeight) {
        self.rect = Rect {
            x: 0.0,
            y: 0.0,
            width,
            height: line_height.0 * 2.0,
        };
        self.pane.set_rect(Rect {
            x: 0.0,
            y: line_height.0,
            width,
            height: line_height.0,
        });
    }

    fn rect(&self) -> &Rect {
        &self.rect
    }
}
