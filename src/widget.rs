use crate::app::LineHeight;
use crate::buffer::Buffer;

pub trait Widget {
    fn buffer(&self) -> &Buffer;

    fn buffer_mut(&mut self) -> &mut Buffer;

    fn recalc_layout(&mut self, width: f64, line_height: LineHeight);
}
