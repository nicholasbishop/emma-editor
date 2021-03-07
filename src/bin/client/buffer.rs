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

pub struct RestoreInfo {
    pub id: BufferId,
    pub path: PathBuf,
    pub name: String,
    pub kind: BufferKind,
    pub cursor_position: i32,
}

#[derive(Debug, PartialEq)]
pub enum BufferKind {
    File,
    Shell,
}

impl BufferKind {
    pub const fn to_str(&self) -> &'static str {
        match self {
            BufferKind::File => "file",
            BufferKind::Shell => "shell",
        }
    }
}

impl BufferKind {
    pub fn from_str(s: &str) -> Option<BufferKind> {
        if s == BufferKind::File.to_str() {
            Some(BufferKind::File)
        } else if s == BufferKind::Shell.to_str() {
            Some(BufferKind::Shell)
        } else {
            None
        }
    }
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

            let storage = embuf.storage();

            let start = storage.get_start_iter();
            let end = storage.get_end_iter();
            let text = storage.get_text(&start, &end, true);
        });
        storage.set_text(&contents);

        embuf_clone
    }

    #[throws]
    pub fn load_file(path: &Path) -> Embuf {
        Self::load_file_with_id(path, make_buffer_id())?
    }

    #[throws]
    pub fn restore(info: RestoreInfo) -> Embuf {
        match info.kind {
            BufferKind::File => {
                // TODO: lazy load file
                let embuf = Embuf::load_file_with_id(&info.path, info.id)?;
                let storage = embuf.storage();
                let iter = storage.get_iter_at_offset(info.cursor_position);
                storage.place_cursor(&iter);
                embuf
            }
            BufferKind::Shell => {
                // TODO: set directory
                todo!();
            }
        }
    }

    pub fn save(&self) {
        // TODO: report errors in some way
        if self.kind() == BufferKind::File {
            let storage = self.storage();

            let start = storage.get_start_iter();
            let end = storage.get_end_iter();
            let text = storage.get_text(&start, &end, true);

            fs::write(self.path(), text.as_str()).unwrap();
        }
    }

    pub fn name(&self) -> String {
        self.borrow().name.clone()
    }

    pub fn kind(&self) -> BufferKind {
        BufferKind::File
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
