use {
    rand::distributions::Alphanumeric,
    rand::{thread_rng, Rng},
    std::path::PathBuf,
};

pub type Buffer = gtk::TextBuffer;
pub type BufferId = String;
pub type BufferGeneration = u64;

fn make_buffer_id() -> BufferId {
    let r: String = thread_rng()
        .sample_iter(&Alphanumeric)
        .take(8)
        .map(char::from)
        .collect();
    format!("buffer-{}", r)
}

pub struct EmBuf {
    pub buffer_id: String,
    pub path: PathBuf,
    pub storage: Buffer,
    pub generation: BufferGeneration,
}

impl EmBuf {
    pub fn new(path: PathBuf) -> EmBuf {
        let tag_table: Option<&gtk::TextTagTable> = None;

        EmBuf {
            buffer_id: make_buffer_id(),
            path,
            storage: Buffer::new(tag_table),
            generation: 0,
        }
    }
}
