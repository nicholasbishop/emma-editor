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
        sync::{Arc, RwLock},
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
    buffer: Arc<RwLock<Buffer>>,
}

#[derive(Clone, Debug)]
pub struct TextEditor {
    internal: Rc<RefCell<TextEditorInternal>>,
}

impl TextEditor {
    pub fn new() -> TextEditor {
        // TODO
        let buffer = Arc::new(RwLock::new(
            Buffer::from_path(Path::new("src/bin/client/main.rs")).unwrap(),
        ));

        let widget = gtk::DrawingArea::new();

        let internal = TextEditorInternal {
            widget: widget.clone(),
            buffer,
        };

        let editor = TextEditor {
            internal: Rc::new(RefCell::new(internal)),
        };

        let editor_clone = editor.clone();
        widget.set_draw_func(move |_widget, ctx, width, height| {
            TextEditor::draw(editor.clone(), ctx, width, height);
        });

        editor_clone
    }

    pub fn widget(&self) -> gtk::Widget {
        self.internal.borrow().widget.clone().upcast()
    }

    pub fn buffer(&self) -> Arc<RwLock<Buffer>> {
        self.internal.borrow().buffer.clone()
    }

    fn draw(editor: TextEditor, ctx: &cairo::Context, width: i32, height: i32) {
        // Fill in the background.
        ctx.rectangle(0.0, 0.0, width as f64, height as f64);
        let v = 63.0 / 255.0;
        ctx.set_source_rgb(v, v, v);
        ctx.fill();

        ctx.select_font_face(
            "DejaVu Sans Mono",
            cairo::FontSlant::Normal,
            cairo::FontWeight::Normal,
        );
        ctx.set_font_size(18.0);
        let font_extents = ctx.font_extents();

        let margin = 2.0;
        let mut y = margin;

        let buffer = editor.buffer();
        let guard = buffer.read().unwrap();

        for line in guard.text.lines() {
            y += font_extents.height;
            ctx.move_to(margin, y);

            let v1 = 220.0 / 255.0;
            let v2 = 204.0 / 255.0;
            ctx.set_source_rgb(v1, v1, v2);

            for c in line.chars() {
                // Chop off the trailing newline. TODO: implement this
                // properly.
                if c == '\n' {
                    break;
                }
                ctx.show_text(&c.to_string());
            }
        }
    }
}
