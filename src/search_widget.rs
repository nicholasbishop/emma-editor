use crate::buffer::Buffer;
use crate::pane_tree::{Pane, Rect};
use crate::widget::Widget;

// TODO
#[expect(unused)]
pub struct SearchWidget {
    buffer: Buffer,
    pane: Pane,
    rect: Rect,
}

impl Widget for SearchWidget {
    fn buffer(&self) -> &Buffer {
        &self.buffer
    }

    fn buffer_mut(&mut self) -> &mut Buffer {
        &mut self.buffer
    }
}
