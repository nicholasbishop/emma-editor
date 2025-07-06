use crate::buffer::Buffer;

pub trait Widget {
    fn buffer(&self) -> &Buffer;

    fn buffer_mut(&mut self) -> &mut Buffer;
}
