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
