use {
    crate::{shell::Shell, HighlightRequest},
    anyhow::Error,
    crossbeam_channel::Sender,
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

    shell: Option<Shell>,
}

impl EmbufInternal {
    #[throws]
    fn send_to_shell(&mut self) {
        if let Some(shell) = &mut self.shell {
            let mark = self.storage.get_mark("output_end").unwrap();
            let mut start_iter = self.storage.get_iter_at_mark(&mark);
            let mut end_iter = self.storage.get_end_iter();
            let mut input: String = self
                .storage
                .get_text(
                    &start_iter,
                    &end_iter,
                    /*include_hidden_chars=*/ false,
                )
                .to_string();
            input.push('\n');

            // Clear the input text since the shell itself will echo
            // the input.
            self.storage.delete(&mut start_iter, &mut end_iter);

            shell.send(input.as_bytes())?;
        }
    }
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
            shell: None,
        })))
    }

    pub fn new(path: PathBuf) -> Embuf {
        Self::new_with_id(path, make_buffer_id())
    }

    #[throws]
    fn load_file_with_id(
        path: &Path,
        highlight_request_sender: Sender<HighlightRequest>,
        buffer_id: BufferId,
    ) -> Embuf {
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

            let req = HighlightRequest {
                buffer_id: embuf.buffer_id(),
                text: text.to_string(),
                generation: embuf.generation(),
                path: embuf.path(),
            };
            highlight_request_sender.send(req).unwrap();
        });
        storage.set_text(&contents);

        embuf_clone
    }

    #[throws]
    pub fn load_file(
        path: &Path,
        highlight_request_sender: Sender<HighlightRequest>,
    ) -> Embuf {
        Self::load_file_with_id(
            path,
            highlight_request_sender,
            make_buffer_id(),
        )?
    }

    #[throws]
    pub fn restore(
        info: RestoreInfo,
        highlight_request_sender: Sender<HighlightRequest>,
    ) -> Embuf {
        match info.kind {
            BufferKind::File => {
                // TODO: lazy load file
                let embuf = Embuf::load_file_with_id(
                    &info.path,
                    highlight_request_sender,
                    info.id,
                )?;
                let storage = embuf.storage();
                let iter = storage.get_iter_at_offset(info.cursor_position);
                storage.place_cursor(&iter);
                embuf
            }
            BufferKind::Shell => {
                // TODO: set directory
                Embuf::launch_shell(&info.name)?
            }
        }
    }

    #[throws]
    pub fn launch_shell(name: &str) -> Embuf {
        let path = Path::new(""); // TODO
        let embuf = Embuf::new(path.into());
        let embuf_clone = embuf.clone();
        let shell = Shell::launch(Box::new(move |bytes| {
            // TODO: this conversion is not necessarily correct
            // because we might have read up to part way through a
            // character, need to think about how to do this correctly
            let s = String::from_utf8_lossy(bytes);

            let embuf = embuf.borrow();
            let storage = &embuf.storage;
            // TODO shared const for this string or keep TextMark?
            let mark = storage.get_mark("output_end").unwrap();
            storage
                .insert(&mut storage.get_iter_at_mark(&mark), &s.to_string());

            // The output_end mark floats left so that user input goes
            // after the mark, that means we have to manually move the
            // mark after the newly inserted shell output.
            let mut iter = storage.get_iter_at_mark(&mark);
            // TODO: it's not clear whether forward_chars measures in
            // bytes or unicode characters or something else.
            iter.forward_chars(s.len() as i32);
            storage.move_mark(&mark, &iter);
        }))?;

        {
            let mut internal = embuf_clone.0.borrow_mut();
            internal.name = name.into();
            internal.storage.create_mark(
                Some("output_end"),
                &internal.storage.get_end_iter(),
                // We kind of want both, not sure how best to
                // represent this. We want the mark to keep to the
                // right of shell output, but to the left of user
                // input. For now set gravity to keep it to the left
                // of user input, and manually move the mark past
                // shell output in the callback.
                /*left_gravity=*/
                true,
            );
            internal.shell = Some(shell);
        }
        embuf_clone
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
        if self.borrow().shell.is_some() {
            BufferKind::Shell
        } else {
            BufferKind::File
        }
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
        self.borrow().shell.is_some()
    }

    #[throws]
    pub fn send_to_shell(&self) {
        self.0.borrow_mut().send_to_shell()?;
    }
}

impl PartialEq for Embuf {
    fn eq(&self, other: &Embuf) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Embuf {}
