use {
    anyhow::Error,
    fehler::throws,
    fs_err as fs,
    gtk4::{self as gtk, cairo, prelude::*},
    ropey::Rope,
    std::{
        cell::RefCell,
        io,
        path::Path,
        rc::Rc,
        sync::{Arc, Mutex},
    },
};

#[derive(Debug)]
pub struct Buffer {
    text: Rope,
}

impl Buffer {
    #[throws]
    pub fn from_path(path: &Path) -> Buffer {
        let text =
            Rope::from_reader(&mut io::BufReader::new(fs::File::open(path)?))?;
        Buffer { text }
    }
}

#[derive(Debug)]
struct TextEditorInternal {
    widget: gtk::DrawingArea,
    buffer: Arc<Mutex<Buffer>>,
}

#[derive(Clone, Debug)]
pub struct TextEditor {
    internal: Rc<RefCell<TextEditorInternal>>,
}

impl TextEditor {
    pub fn new() -> TextEditor {
        let widget = gtk::DrawingArea::new();

        // TODO
        let buffer = Arc::new(Mutex::new(
            Buffer::from_path(Path::new("src/bin/client/main.rs")).unwrap(),
        ));

        let internal = TextEditorInternal { widget, buffer };

        TextEditor {
            internal: Rc::new(RefCell::new(internal)),
        }
    }

    pub fn widget(&self) -> gtk::Widget {
        self.internal.borrow().widget.clone().upcast()
    }
}
