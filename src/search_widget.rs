use crate::app::LineHeight;
use crate::buffer::Buffer;
use crate::pane_tree::{Pane, Rect};
use crate::widget::Widget;

// TODO
pub struct SearchWidget {
    buffer: Buffer,
    pane: Pane,
}

impl SearchWidget {
    pub fn new() -> Self {
        let buffer = Buffer::create_empty();
        let pane = Pane::create_for_widget(buffer.id().clone());
        Self { buffer, pane }
    }
}

impl Widget for SearchWidget {
    fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }

    fn pane_buffer_mut(&mut self) -> (&Pane, &mut Buffer) {
        (&self.pane, &mut self.buffer)
    }

    fn pane_mut_buffer_mut(&mut self) -> (&mut Pane, &mut Buffer) {
        (&mut self.pane, &mut self.buffer)
    }

    fn recalc_layout(&mut self, width: f64, line_height: LineHeight) {
        self.pane.set_rect(Rect {
            x: 0.0,
            y: 0.0,
            width,
            height: line_height.0,
        });
    }

    fn rect(&self) -> &Rect {
        self.pane.rect()
    }
}
