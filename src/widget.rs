use crate::app::LineHeight;
use crate::buffer::Buffer;
use crate::pane_tree::{Pane, Rect};

pub trait Widget {
    fn buffer(&self) -> &Buffer;

    fn buffer_mut(&mut self) -> &mut Buffer;

    fn pane(&self) -> &Pane;

    fn pane_buffer_mut(&mut self) -> (&Pane, &mut Buffer);

    fn pane_mut_buffer_mut(&mut self) -> (&mut Pane, &mut Buffer);

    fn recalc_layout(&mut self, width: f64, line_height: LineHeight);

    fn rect(&self) -> &Rect;
}
