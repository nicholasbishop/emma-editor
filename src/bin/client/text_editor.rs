use {
    crate::theme,
    anyhow::Error,
    fehler::throws,
    fs_err as fs,
    gtk4::{self as gtk, cairo, prelude::*},
    ropey::Rope,
    std::{
        cell::RefCell,
        io,
        path::{Path, PathBuf},
        rc::Rc,
        sync::{Arc, RwLock},
    },
    syntect::{
        highlighting::{
            HighlightState, Highlighter, RangedHighlightIterator, Style,
        },
        parsing::{ParseState, ScopeStack, SyntaxSet},
    },
};

#[derive(Debug)]
struct StyleSpan {
    len: usize,
    style: Style,
}

#[derive(Debug)]
pub struct Buffer {
    text: Rope,
    path: PathBuf,

    // Outer vec: per line
    // Inner vec: style for a contiguous group of chars, covers the
    // whole line.
    // TODO: think about a smarter structure
    style_spans: Vec<Vec<StyleSpan>>,
}

impl Buffer {
    #[throws]
    pub fn from_path(path: &Path) -> Buffer {
        let text =
            Rope::from_reader(&mut io::BufReader::new(fs::File::open(path)?))?;
        let mut buffer = Buffer {
            text,
            path: path.into(),
            style_spans: Vec::new(),
        };

        // TODO: run in background
        buffer.recalc_style_spans();

        buffer
    }

    // TODO: simple for now
    fn recalc_style_spans(&mut self) {
        self.style_spans.clear();

        // TODO: cache
        let syntax_set = SyntaxSet::load_defaults_newlines();
        let theme = theme::load_default_theme().unwrap();

        let syntax = if let Ok(Some(syntax)) =
            syntax_set.find_syntax_for_file(&self.path)
        {
            syntax
        } else {
            return;
        };

        let mut parse_state = ParseState::new(syntax);
        let highlighter = Highlighter::new(&theme);
        let mut highlight_state =
            HighlightState::new(&highlighter, ScopeStack::new());

        let mut full_line = String::new();
        for line in self.text.lines() {
            full_line.clear();
            // TODO: any way to avoid pulling the full line in? Should
            // at least limit the length probably.
            for chunk in line.chunks() {
                full_line.push_str(chunk);
            }

            let changes = parse_state.parse_line(&full_line, &syntax_set);

            let iter = RangedHighlightIterator::new(
                &mut highlight_state,
                &changes,
                &full_line,
                &highlighter,
            );

            self.style_spans.push(
                iter.map(|(style, _text, range)| StyleSpan {
                    len: range.len(),
                    style,
                })
                .collect(),
            );
        }
    }
}

fn set_source_from_syntect_color(
    ctx: &cairo::Context,
    color: &syntect::highlighting::Color,
) {
    let r = (color.r as f64) / 255.0;
    let g = (color.g as f64) / 255.0;
    let b = (color.b as f64) / 255.0;
    let a = (color.a as f64) / 255.0;
    ctx.set_source_rgba(r, g, b, a);
}

#[derive(Debug)]
struct TextEditorInternal {
    widget: gtk::DrawingArea,
    buffer: Arc<RwLock<Buffer>>,
    top_line: usize,
}

impl TextEditorInternal {
    fn scroll(&mut self, dir: i32) {
        let num_lines = self.buffer.read().unwrap().text.lines().count();

        if dir == -1 && self.top_line > 0 {
            self.top_line -= 1;
        } else if dir == 1 && self.top_line + 1 < num_lines {
            self.top_line += 1;
        }
        self.widget.queue_draw();
    }

    fn draw(&self, ctx: &cairo::Context, width: i32, height: i32) {
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

        let guard = self.buffer.read().unwrap();

        for (line_idx, line) in guard.text.lines_at(self.top_line).enumerate() {
            let line_idx = line_idx + self.top_line;

            y += font_extents.height;

            ctx.move_to(margin, y);

            let v1 = 220.0 / 255.0;
            let v2 = 204.0 / 255.0;
            ctx.set_source_rgb(v1, v1, v2);

            let style_spans = &guard.style_spans[line_idx];

            let mut char_iter = line.chars();
            for span in style_spans {
                set_source_from_syntect_color(ctx, &span.style.foreground);

                for _ in 0..span.len {
                    let c = char_iter.next().unwrap();
                    // Chop off the trailing newline. TODO: implement this
                    // properly.
                    if c == '\n' {
                        break;
                    }
                    ctx.show_text(&c.to_string());
                }
            }

            // Stop if rendering past the bottom of the widget. TODO:
            // is this the right calculation?
            if y > (height as f64) {
                break;
            }
        }
    }
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
            top_line: 0,
        };

        let editor = TextEditor {
            internal: Rc::new(RefCell::new(internal)),
        };

        let editor_clone = editor.clone();
        widget.set_draw_func(move |_widget, ctx, width, height| {
            editor.internal.borrow().draw(ctx, width, height);
        });

        editor_clone
    }

    pub fn widget(&self) -> gtk::Widget {
        self.internal.borrow().widget.clone().upcast()
    }

    // TODO
    pub fn scroll(&self, dir: i32) {
        self.internal.borrow_mut().scroll(dir);
    }
}
