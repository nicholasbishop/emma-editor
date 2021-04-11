use {
    super::App,
    crate::{
        buffer::{Buffer, LinePosition},
        grapheme::next_grapheme_boundary,
        pane_tree::Pane,
        theme::Theme,
    },
    gtk4::{
        self as gtk, cairo,
        pango::{self, Layout},
        prelude::*,
    },
    ropey::RopeSlice,
    std::{fmt, ops::Range},
    syntect::highlighting::Style,
    tracing::{debug, instrument},
};

fn set_source_rgba_from_u8(ctx: &cairo::Context, r: u8, g: u8, b: u8, a: u8) {
    let r = (r as f64) / 255.0;
    let g = (g as f64) / 255.0;
    let b = (b as f64) / 255.0;
    let a = (a as f64) / 255.0;
    ctx.set_source_rgba(r, g, b, a);
}

fn set_source_rgb_from_u8(ctx: &cairo::Context, r: u8, g: u8, b: u8) {
    set_source_rgba_from_u8(ctx, r, g, b, 255);
}

fn set_source_from_syntect_color(
    ctx: &cairo::Context,
    color: &syntect::highlighting::Color,
) {
    set_source_rgba_from_u8(ctx, color.r, color.g, color.b, color.a);
}

fn pango_unscale(i: i32) -> f64 {
    i as f64 / pango::SCALE as f64
}

// TODO: remove or rename this
#[derive(Debug)]
pub struct Font {
    line_height: f64,
}

impl Font {
    pub fn new(pctx: pango::Context) -> Font {
        let font_desc = pctx.get_font_description();

        let language = None;
        let metrics = pctx.get_metrics(font_desc.as_ref(), language).unwrap();
        let line_height = pango_unscale(metrics.get_height());

        Font { line_height }
    }

    pub fn line_height(&self) -> f64 {
        self.line_height
    }
}

struct StyledLayout<'a> {
    layout: Layout,
    style: &'a Style,
    is_cursor: bool,
}

struct DrawPane<'a> {
    ctx: &'a cairo::Context,
    widget: &'a gtk::DrawingArea,
    pane: &'a Pane,
    buf: &'a Buffer,
    font: &'a Font,
    theme: &'a Theme,
    span_buf: String,
    margin: f64,
    cursor: LinePosition,
    len_lines: usize,
    x: f64,
    y: f64,
    empty_style: &'a Style,
}

impl<'a> fmt::Debug for DrawPane<'a> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "DrawPane({}, {}, pos={},{})",
            self.pane.id(),
            self.buf.id(),
            self.x,
            self.y
        )
    }
}

impl<'a> DrawPane<'a> {
    fn create_layout(&self, text: &str) -> Layout {
        self.widget.create_pango_layout(Some(text))
    }

    fn layout_line_range(
        &mut self,
        line: &RopeSlice,
        range: Range<usize>,
    ) -> Layout {
        self.span_buf.clear();
        for chunk in line.slice(range).chunks() {
            self.span_buf.push_str(chunk);
        }

        self.create_layout(&self.span_buf)
    }

    fn draw_layout(&mut self, layout: &Layout) {
        self.ctx.move_to(self.x, self.y);
        pangocairo::show_layout(self.ctx, layout);
        self.x += pango_unscale(layout.get_size().0);
    }

    fn styled_layouts_from_line(
        &mut self,
        line: &RopeSlice,
        line_idx: usize,
    ) -> Vec<StyledLayout<'a>> {
        let mut output = Vec::new();

        let style_spans = &self.buf.style_spans()[line_idx];

        let mut span_offset = 0;
        for span in style_spans {
            debug!("span: {} chars", span.len);
            let mut push =
                |me: &mut DrawPane, range: Range<usize>, is_cursor| {
                    if !range.is_empty() {
                        output.push(StyledLayout {
                            layout: me.layout_line_range(&line, range),
                            style: &span.style,
                            is_cursor,
                        });
                    }
                };

            let span_range = span_offset..span_offset + span.len;
            span_offset += span.len;

            if line_idx == self.cursor.line
                && span_range.contains(&self.cursor.offset)
            {
                debug!("span contains cursor");
                push(self, span_range.start..self.cursor.offset, false);

                let cursor_end_char =
                    next_grapheme_boundary(&line, self.cursor.offset);

                push(self, self.cursor.offset..cursor_end_char, true);
                push(self, cursor_end_char..span_range.end, false);
            } else {
                push(self, span_range, false);
            }
        }

        // The last line of the buffer by definition doesn't end in a
        // new line. (If the last character in a file is a newline,
        // ropey's iterator produces an empty line at the end.) We
        // still need to draw the cursor in that case though, so
        // append it here.
        if self.cursor.line == line_idx
            && line_idx + 1 == self.len_lines
            && self.cursor.offset == line.len_chars()
        {
            debug!("eof cursor");
            output.push(StyledLayout {
                layout: self.create_layout(""),
                style: self.empty_style,
                is_cursor: true,
            });
            return output;
        }

        output
    }

    fn draw_cursor(&mut self, styled_layout: &StyledLayout) {
        if !self.pane.is_cursor_visible() {
            debug!("cursor not visible");
            return;
        }

        set_source_from_syntect_color(
            self.ctx,
            self.theme
                .syntect
                .settings
                .caret
                .as_ref()
                .expect("caret color not set in theme"),
        );
        let mut cursor_width = pango_unscale(styled_layout.layout.get_size().0);
        if cursor_width == 0.0 {
            // TODO: this is needed for at least newlines,
            // which give (0, double-line-height), but
            // might need to think about other kinds of
            // not-really-rendered characters as well.
            cursor_width = self.font.line_height / 2.0;
        }
        debug!(
            "drawing cursor: size={}x{}",
            cursor_width, self.font.line_height
        );
        self.ctx
            .rectangle(self.x, self.y, cursor_width, self.font.line_height);
        if self.pane.is_active() {
            self.ctx.fill();
        } else {
            self.ctx.stroke();
        }
    }

    fn draw_line(&mut self, line: &RopeSlice, line_idx: usize) {
        self.x = self.pane.rect().x;

        self.ctx.move_to(self.margin, self.y);

        set_source_rgb_from_u8(self.ctx, 220, 220, 204);

        let styled_layouts = self.styled_layouts_from_line(line, line_idx);

        for styled_layout in styled_layouts {
            if styled_layout.is_cursor {
                self.draw_cursor(&styled_layout);

                if self.pane.is_active() {
                    // Set inverted text color. TODO: set from
                    // theme?
                    self.ctx.set_source_rgb(0.0, 0.0, 0.0);
                }
            } else {
                set_source_from_syntect_color(
                    self.ctx,
                    &styled_layout.style.foreground,
                );
            }
            self.draw_layout(&styled_layout.layout);
        }

        self.y += self.font.line_height;
    }

    fn draw_info_bar(&mut self) {
        if self.pane.is_active() {
            set_source_from_syntect_color(
                self.ctx,
                &self.theme.info_bar_active.background,
            );
        } else {
            set_source_from_syntect_color(
                self.ctx,
                &self.theme.info_bar_inactive.background,
            );
        }
        let rect = self.pane.rect();
        self.ctx.rectangle(
            rect.x,
            rect.y + rect.height - self.font.line_height,
            rect.width,
            self.font.line_height,
        );
        self.ctx.fill();

        if let Some(path) = self.buf.path() {
            let name = path.file_name().expect("path has no file name");

            if self.pane.is_active() {
                set_source_from_syntect_color(
                    self.ctx,
                    &self.theme.info_bar_active.foreground,
                );
            } else {
                set_source_from_syntect_color(
                    self.ctx,
                    &self.theme.info_bar_inactive.foreground,
                );
            }

            let layout = self.create_layout(&name.to_string_lossy());

            self.x = rect.x;
            self.y = rect.y + rect.height - self.font.line_height;
            self.draw_layout(&layout);
        }
    }

    #[instrument]
    fn draw(&mut self) {
        debug!("drawing pane with {} lines", self.buf.text().len_lines());

        // Fill in the background. Subtract small amount from the
        // right edge to give a border.
        let rect = self.pane.rect();
        let border = 0.5;
        self.ctx
            .rectangle(rect.x, rect.y, rect.width - border, rect.height);
        set_source_rgb_from_u8(self.ctx, 63, 63, 63);
        self.ctx.fill();

        self.cursor = self.buf.cursor(self.pane).line_position(self.buf);

        self.y = rect.y + self.margin;

        for (line_idx, line) in
            self.buf.text().lines_at(self.pane.top_line()).enumerate()
        {
            let line_idx = self.pane.top_line() + line_idx;
            self.draw_line(&line, line_idx);

            // Stop if rendering past the bottom of the widget. TODO:
            // is this the right calculation?
            if self.y > (rect.y + rect.height as f64) {
                break;
            }
        }

        if self.pane.show_info_bar() {
            self.draw_info_bar();
        }
    }
}

impl App {
    pub(super) fn draw(
        &self,
        ctx: &cairo::Context,
        width: f64,
        height: f64,
        font: &Font,
        theme: &Theme,
    ) {
        // Fill in the background. This acts as the border color
        // between panes. Don't go all the way to the right
        // edge to avoid an unwanted border there.
        let border = 1.0;
        ctx.rectangle(0.0, 0.0, width - border, height);
        set_source_rgb_from_u8(ctx, 220, 220, 204);
        ctx.fill();

        let mut panes = self.pane_tree.panes();
        panes.push(self.pane_tree.minibuf());

        let empty_style = Style::default();

        for pane in panes {
            let buf = self.buffers.get(pane.buffer_id()).unwrap();

            let mut dp = DrawPane {
                ctx,
                widget: &self.widget,
                pane,
                buf,
                font,
                theme,
                span_buf: String::new(),
                margin: 2.0,
                cursor: LinePosition::default(),
                len_lines: buf.text().len_lines(),
                x: 0.0,
                y: 0.0,
                empty_style: &empty_style,
            };
            dp.draw();
        }
    }
}
