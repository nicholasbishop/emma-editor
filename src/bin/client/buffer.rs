use {
    anyhow::Error,
    fehler::throws,
    fs_err as fs,
    gtk4::{prelude::*, TextBuffer, TextTagTable},
    rand::{distributions::Alphanumeric, thread_rng, Rng},
    std::{
        cell::RefCell,
        path::{Path, PathBuf},
        rc::Rc,
    },
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

#[derive(Debug)]
struct EmbufInternal {
    buffer_id: BufferId,
    path: PathBuf,
    name: String,
    storage: Buffer,
    generation: BufferGeneration,
}

#[derive(Clone, Debug)]
pub struct Embuf(Rc<RefCell<EmbufInternal>>);

impl Embuf {
    fn new_with_id(path: PathBuf, buffer_id: BufferId) -> Embuf {
        let tag_table: Option<&TextTagTable> = None;

        Embuf(Rc::new(RefCell::new(EmbufInternal {
            buffer_id,
            name: path
                .file_name()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_else(|| "?".to_string()),
            path,
            storage: Buffer::new(tag_table),
            generation: 0,
        })))
    }

    pub fn new(path: PathBuf) -> Embuf {
        Self::new_with_id(path, make_buffer_id())
    }

    #[throws]
    fn load_file_with_id(path: &Path, buffer_id: BufferId) -> Embuf {
        let contents = fs::read_to_string(path)?;

        let embuf = Embuf::new_with_id(path.into(), buffer_id);

        let storage = embuf.storage();
        let embuf_clone = embuf.clone();

        storage.connect_changed(move |_| {
            embuf.increment_generation();
        });
        storage.set_text(&contents);

        embuf_clone
    }

    #[throws]
    pub fn load_file(path: &Path) -> Embuf {
        Self::load_file_with_id(path, make_buffer_id())?
    }

    pub fn name(&self) -> String {
        self.borrow().name.clone()
    }

    fn borrow(&self) -> std::cell::Ref<EmbufInternal> {
        self.0.borrow()
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

    pub fn has_shell(&self) -> bool {
        false
    }
}

impl PartialEq for Embuf {
    fn eq(&self, other: &Embuf) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Embuf {}
