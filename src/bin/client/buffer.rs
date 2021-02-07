use std::path::PathBuf;

pub type Buffer = gtk::TextBuffer;

pub struct EmBuf {
    pub buffer_id: String,
    pub path: PathBuf,
    pub storage: Buffer,
    pub generation: u64,
}
