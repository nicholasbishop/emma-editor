use {
    crate::shell::Shell,
    anyhow::Error,
    fehler::throws,
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
    storage: Buffer,
    generation: BufferGeneration,
    shell: Option<Shell>,
}

#[derive(Clone, Debug)]
pub struct Embuf(Rc<RefCell<EmbufInternal>>);

impl Embuf {
    pub fn new(path: PathBuf) -> Embuf {
        let tag_table: Option<&TextTagTable> = None;

        Embuf(Rc::new(RefCell::new(EmbufInternal {
            buffer_id: make_buffer_id(),
            path,
            storage: Buffer::new(tag_table),
            generation: 0,
            shell: None,
        })))
    }

    #[throws]
    pub fn launch_shell() -> Embuf {
        dbg!("launch");
        let path = Path::new(""); // TODO
        let embuf = Embuf::new(path.into());
        let embuf_clone = embuf.clone();
        let shell = Shell::launch(Box::new(move |bytes| {
            // TODO: this conversion is not necessarily correct
            // because we might have read up to part way through a
            // character, need to think about how to do this correctly
            let s = String::from_utf8_lossy(bytes);

            let embuf = embuf.borrow();
            embuf
                .storage
                .insert(&mut embuf.storage.get_end_iter(), &s.to_string());
        }))?;
        embuf_clone.0.borrow_mut().shell = Some(shell);
        embuf_clone
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

impl PartialEq for Embuf {
    fn eq(&self, other: &Embuf) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Embuf {}
