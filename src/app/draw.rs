use {
    super::App,
    crate::{
        buffer::{Buffer, LinePosition},
        grapheme::next_grapheme_boundary,
        pane_tree::Pane,
    },
    gtk4::{
        cairo,
        pango::{self, FontDescription, Layout},
    },
    ropey::RopeSlice,
    std::ops::Range,
    syntect::highlighting::Style,
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

pub struct Font {
    description: FontDescription,
    line_height: f64,
}

impl Font {
    pub fn new(ctx: &cairo::Context) -> Font {
        // TODO: prints out the list of font families
        // let font_map = pangocairo::FontMap::get_default().unwrap();
        // use gtk4::prelude::*;
        // let families = font_map.list_families();
        // for fam in families {
        //     println!("{}", fam.get_name().unwrap());
        // }

        let mut font_desc = FontDescription::new();
        font_desc.set_family("Monospace");
        // TODO
        font_desc.set_absolute_size(18.0 * pango::SCALE as f64);

        let pctx = pangocairo::create_context(ctx).unwrap();
        let language = None;
        let metrics = pctx.get_metrics(Some(&font_desc), language).unwrap();
        let line_height = pango_unscale(metrics.get_height());

        Font {
            description: font_desc,
            line_height,
        }
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
    pane: &'a Pane,
    buf: &'a Buffer,
    font: &'a Font,
    span_buf: String,
    margin: f64,
    cursor: LinePosition,
    x: f64,
    y: f64,
}

impl<'a> DrawPane<'a> {
    fn new(
        ctx: &'a cairo::Context,
        pane: &'a Pane,
        buf: &'a Buffer,
        font: &'a Font,
    ) -> DrawPane<'a> {
        DrawPane {
            ctx,
            pane,
            buf,
            font,
            span_buf: String::new(),
            margin: 2.0,
            cursor: LinePosition::default(),
            x: 0.0,
            y: 0.0,
        }
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

        let layout = pangocairo::create_layout(self.ctx).unwrap();
        layout.set_font_description(Some(&self.font.description));
        layout.set_text(&self.span_buf);
        layout
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

        let line_idx = line_idx + self.pane.top_line();

        let style_spans = &self.buf.style_spans()[line_idx];

        let mut span_offset = 0;
        for span in style_spans {
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
                push(self, span_range.start..self.cursor.offset, false);

                let cursor_end_char =
                    next_grapheme_boundary(&line, self.cursor.offset);

                push(self, self.cursor.offset..cursor_end_char, true);
                push(self, cursor_end_char..span_range.end, false);
            } else {
                push(self, span_range, false);
            }
        }

        output
    }

    fn draw_cursor(&mut self, styled_layout: &StyledLayout) {
        // TODO: color from theme
        set_source_rgb_from_u8(self.ctx, 237, 212, 0);
        let mut cursor_width = pango_unscale(styled_layout.layout.get_size().0);
        if cursor_width == 0.0 {
            // TODO: this is needed for at least newlines,
            // which give (0, double-line-height), but
            // might need to think about other kinds of
            // not-really-rendered characters as well.
            cursor_width = self.font.line_height / 2.0;
        }
        self.ctx
            .rectangle(self.x, self.y, cursor_width, self.font.line_height);
        if self.pane.is_active() {
            self.ctx.fill();
        } else {
            self.ctx.stroke();
        }
    }

    fn draw_line(&mut self, line: &RopeSlice, line_idx: usize) {
        let line_idx = line_idx + self.pane.top_line();

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

    fn draw_info_bar(&self) {
        // TODO: color from theme
        if self.pane.is_active() {
            set_source_rgb_from_u8(self.ctx, 30, 35, 32);
        } else {
            set_source_rgb_from_u8(self.ctx, 46, 51, 48);
        }
        let rect = self.pane.rect();
        self.ctx.rectangle(
            rect.x,
            rect.y + rect.height - self.font.line_height,
            rect.width,
            self.font.line_height,
        );
        self.ctx.fill();
    }

    fn draw(&mut self) {
        // Fill in the background. Subtract small amount from bottom
        // and right edges to give a border.
        let rect = self.pane.rect();
        let border = 0.5;
        self.ctx.rectangle(
            rect.x,
            rect.y,
            rect.width - border,
            rect.height - border,
        );
        set_source_rgb_from_u8(self.ctx, 63, 63, 63);
        self.ctx.fill();

        self.cursor = self.pane.cursor().line_position(self.buf);

        self.y = rect.y + self.margin;

        for (line_idx, line) in
            self.buf.text().lines_at(self.pane.top_line()).enumerate()
        {
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
    ) {
        // Fill in the background. This acts as the border color
        // between panes. Don't go all the way to the right/bottom
        // edges to avoid unwanted borders there.
        let border = 1.0;
        ctx.rectangle(0.0, 0.0, width - border, height - border);
        set_source_rgb_from_u8(ctx, 220, 220, 204);
        ctx.fill();

        let mut panes = self.pane_tree.panes();
        panes.push(self.pane_tree.minibuf());

        for pane in panes {
            let buf = self.buffers.get(pane.buffer_id()).unwrap();

            let mut dp = DrawPane::new(ctx, pane, buf, font);
            dp.draw();
        }
    }
}
