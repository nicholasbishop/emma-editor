use {
    crate::{key_map::Direction, theme},
    anyhow::Error,
    fehler::throws,
    fs_err as fs,
    gtk4::{self as gtk, cairo, prelude::*, MovementStep},
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

#[derive(Clone, Copy, Debug, Default)]
struct Position {
    line: usize,
    line_offset: usize,
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

    /// Editors currently showing this buffer.
    editors: Vec<TextEditor>,
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
            editors: Vec::new(),
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

    fn char_index_from_position(&self, pos: Position) -> usize {
        self.text.line_to_char(pos.line) + pos.line_offset
    }

    fn insert_char(&mut self, c: char, pos: Position) {
        self.text.insert_char(self.char_index_from_position(pos), c);
        // TODO, don't recalc everything and don't do it
        // synchronously.
        self.recalc_style_spans();
        for editor in &self.editors {
            editor.update_cursor_after_insert(pos, /*TODO*/ false);
            editor.widget().queue_draw();
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
    is_active: bool,
    cursor: Position,
}

impl TextEditorInternal {
    fn move_cursor_relative(&mut self, step: MovementStep, dir: Direction) {
        let buf = self.buffer.read().expect("bad lock");
        let cursor = &mut self.cursor;
        match step {
            MovementStep::VisualPositions => {
                if dir == Direction::Dec {
                    if cursor.line_offset == 0 {
                        if cursor.line > 0 {
                            cursor.line -= 1;
                            cursor.line_offset =
                                buf.text.line(cursor.line).len_chars() - 1;
                        }
                    } else {
                        cursor.line_offset -= 1;
                    }
                } else {
                    if cursor.line_offset
                        == buf.text.line(cursor.line).len_chars() - 1
                    {
                        if cursor.line + 1 < buf.text.len_lines() {
                            cursor.line += 1;
                            cursor.line_offset = 0;
                        }
                    } else {
                        cursor.line_offset += 1;
                    }
                }
            }
            MovementStep::DisplayLines => {
                if dir == Direction::Dec {
                    if cursor.line > 0 {
                        cursor.line -= 1;
                    }
                } else {
                    if cursor.line + 1 < buf.text.len_lines() {
                        cursor.line += 1;
                    }
                }
            }
            MovementStep::BufferEnds => {
                if dir == Direction::Dec {
                    cursor.line = 0;
                    cursor.line_offset = 0;
                } else {
                    cursor.line = buf.text.len_lines() - 1;
                    cursor.line_offset = buf
                        .text
                        .line(cursor.line)
                        .len_chars()
                        .saturating_sub(1);
                }
            }
            _ => todo!(),
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
            let mut line_offset = 0;
            for span in style_spans {
                set_source_from_syntect_color(ctx, &span.style.foreground);

                for _ in 0..span.len {
                    let c = char_iter.next().unwrap();
                    let cs = c.to_string();

                    // Set style for cursor.
                    let is_cursor = line_idx == self.cursor.line
                        && line_offset == self.cursor.line_offset;
                    if is_cursor {
                        let size = ctx.text_extents(&cs);
                        let cur_point = ctx.get_current_point();
                        // TODO: color from theme
                        let r = 237.0 / 255.0;
                        let g = 212.0 / 255.0;
                        let b = 0.0;
                        ctx.set_source_rgb(r, g, b);
                        ctx.rectangle(
                            cur_point.0,
                            cur_point.1 - font_extents.height
                                + (font_extents.height - font_extents.ascent),
                            size.x_advance,
                            font_extents.height,
                        );
                        if self.is_active {
                            ctx.fill();
                        } else {
                            ctx.stroke();
                        }
                        ctx.move_to(cur_point.0, cur_point.1);

                        if self.is_active {
                            // Set inverted text color. TODO: set from
                            // theme?
                            ctx.set_source_rgb(0.0, 0.0, 0.0);
                        }
                    }

                    // Chop off the trailing newline. TODO: implement this
                    // properly.
                    if c == '\n' {
                        break;
                    }

                    ctx.show_text(&cs);

                    if is_cursor {
                        // Reset the style to the span style.
                        set_source_from_syntect_color(
                            ctx,
                            &span.style.foreground,
                        );
                    }

                    line_offset += 1;
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
        let buffer =
            Buffer::from_path(Path::new("src/bin/client/main.rs")).unwrap();
        let buffer = Arc::new(RwLock::new(buffer));

        let widget = gtk::DrawingArea::new();

        let internal = TextEditorInternal {
            widget: widget.clone(),
            buffer: buffer.clone(),
            top_line: 0,
            is_active: false,
            cursor: Position::default(),
        };

        let editor = TextEditor {
            internal: Rc::new(RefCell::new(internal)),
        };
        buffer
            .write()
            .expect("bad lock")
            .editors
            .push(editor.clone());

        let editor_clone = editor.clone();
        widget.set_draw_func(move |_widget, ctx, width, height| {
            editor.internal.borrow().draw(ctx, width, height);
        });

        editor_clone
    }

    pub fn widget(&self) -> gtk::Widget {
        self.internal.borrow().widget.clone().upcast()
    }

    fn update_cursor_after_insert(&self, p: Position, line_added: bool) {
        let mut internal = self.internal.borrow_mut();
        if line_added {
            if internal.cursor.line >= p.line {
                internal.cursor.line += 1;
            }
        } else {
            if internal.cursor.line == p.line
                && internal.cursor.line_offset >= p.line_offset
            {
                internal.cursor.line_offset += 1;
            }
        }
    }

    pub fn move_cursor_relative(&self, step: MovementStep, dir: Direction) {
        self.internal.borrow_mut().move_cursor_relative(step, dir);
    }

    pub fn set_active(&self, is_active: bool) {
        let mut internal = self.internal.borrow_mut();
        internal.is_active = is_active;
        internal.widget.queue_draw();
    }

    pub fn insert_char(&self, c: char) {
        let pos = self.internal.borrow().cursor;
        let buf = self.internal.borrow().buffer.clone();
        buf.write().expect("bad lock").insert_char(c, pos);
    }
}
