use {
    gtk4::{TextBuffer, TextTagTable},
    rand::{distributions::Alphanumeric, thread_rng, Rng},
    std::{cell::RefCell, path::PathBuf, rc::Rc},
};

pub type Buffer = TextBuffer;
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

#[derive(Debug, Eq, PartialEq)]
struct EmbufInternal {
    buffer_id: BufferId,
    path: PathBuf,
    storage: Buffer,
    generation: BufferGeneration,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Embuf(Rc<RefCell<EmbufInternal>>);

impl Embuf {
    pub fn new(path: PathBuf) -> Embuf {
        let tag_table: Option<&TextTagTable> = None;

        Embuf(Rc::new(RefCell::new(EmbufInternal {
            buffer_id: make_buffer_id(),
            path,
            storage: Buffer::new(tag_table),
            generation: 0,
        })))
    }

    fn borrow(&self) -> std::cell::Ref<EmbufInternal> {
        self.0.borrow()
    }

    pub fn buffer_id(&self) -> BufferId {
        self.borrow().buffer_id.clone()
    }

    pub fn path(&self) -> PathBuf {
        self.borrow().path.clone()
    }

    pub fn storage(&self) -> Buffer {
        self.borrow().storage.clone()
    }

    pub fn generation(&self) -> BufferGeneration {
        self.borrow().generation
    }

    pub fn increment_generation(&self) {
        self.0.borrow_mut().generation += 1;
    }
}
