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

fn layout_scaled_size(layout: &Layout) -> (f64, f64) {
    (
        layout.get_size().0 as f64 / pango::SCALE as f64,
        layout.get_size().1 as f64 / pango::SCALE as f64,
    )
}

struct StyledLayout<'a> {
    layout: Layout,
    style: &'a Style,
    is_cursor: bool,
}

struct DrawPane {
    font_desc: FontDescription,
    span_buf: String,
    margin: f64,
    cursor: LinePosition,
    x: f64,
    y: f64,
}

impl DrawPane {
    fn new() -> DrawPane {
        // TODO: prints out the list of font families
        // let font_map = pangocairo::FontMap::get_default().unwrap();
        // use gtk4::prelude::*;
        // let families = font_map.list_families();
        // for fam in families {
        //     println!("{}", fam.get_name().unwrap());
        // }

        let mut font_desc = FontDescription::new();
        font_desc.set_family("Monospace");
        font_desc.set_absolute_size(18.0 * pango::SCALE as f64);

        DrawPane {
            font_desc,
            span_buf: String::new(),
            margin: 2.0,
            cursor: LinePosition::default(),
            x: 0.0,
            y: 0.0,
        }
    }

    fn layout_line_range(
        &mut self,
        ctx: &cairo::Context,
        line: &RopeSlice,
        range: Range<usize>,
    ) -> Layout {
        self.span_buf.clear();
        for chunk in line.slice(range.clone()).chunks() {
            self.span_buf.push_str(chunk);
        }

        let layout = pangocairo::create_layout(ctx).unwrap();
        layout.set_font_description(Some(&self.font_desc));
        layout.set_text(&self.span_buf);
        layout
    }

    fn draw_layout(&mut self, ctx: &cairo::Context, layout: &Layout) {
        ctx.move_to(self.x, self.y);
        pangocairo::show_layout(ctx, layout);
        self.x += layout_scaled_size(layout).0;
    }

    fn styled_layouts_from_line<'a>(
        &mut self,
        ctx: &cairo::Context,
        pane: &Pane,
        buf: &'a Buffer,
        line: &RopeSlice,
        line_idx: usize,
    ) -> Vec<StyledLayout<'a>> {
        let mut output = Vec::new();

        let line_idx = line_idx + pane.top_line();

        let style_spans = &buf.style_spans()[line_idx];

        let mut span_offset = 0;
        for span in style_spans {
            let mut push =
                |me: &mut DrawPane, range: Range<usize>, is_cursor| {
                    if !range.is_empty() {
                        output.push(StyledLayout {
                            layout: me.layout_line_range(ctx, &line, range),
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

    fn draw_line(
        &mut self,
        ctx: &cairo::Context,
        pane: &Pane,
        buf: &Buffer,
        line: &RopeSlice,
        line_idx: usize,
        line_height: f64,
    ) {
        let line_idx = line_idx + pane.top_line();

        self.x = 0.0;
        self.y += line_height;

        ctx.move_to(self.margin, self.y);

        set_source_rgb_from_u8(ctx, 220, 220, 204);

        let styled_layouts =
            self.styled_layouts_from_line(ctx, pane, buf, line, line_idx);

        for styled_layout in styled_layouts {
            if styled_layout.is_cursor {
                // TODO: color from theme
                set_source_rgb_from_u8(ctx, 237, 212, 0);
                // TODO: rework this, don't need scaled height any more
                let mut layout_size = layout_scaled_size(&styled_layout.layout);
                if layout_size.0 == 0.0 {
                    // TODO: this is needed for at least newlines,
                    // which give (0, double-line-height), but
                    // might need to think about other kinds of
                    // not-really-rendered characters as well.
                    layout_size.0 = line_height / 2.0;
                }
                ctx.rectangle(self.x, self.y, layout_size.0, line_height);
                if pane.is_active() {
                    ctx.fill();
                } else {
                    ctx.stroke();
                }

                if pane.is_active() {
                    // Set inverted text color. TODO: set from
                    // theme?
                    ctx.set_source_rgb(0.0, 0.0, 0.0);
                }
            } else {
                set_source_from_syntect_color(
                    ctx,
                    &styled_layout.style.foreground,
                );
            }
            self.draw_layout(ctx, &styled_layout.layout);
        }
    }

    fn draw(&mut self, ctx: &cairo::Context, pane: &Pane, buf: &Buffer) {
        // Fill in the background.
        let rect = pane.rect();
        ctx.rectangle(rect.x, rect.y, rect.width, rect.height);
        set_source_rgb_from_u8(ctx, 63, 63, 63);
        ctx.fill();

        self.cursor = pane.cursor().line_position(buf);

        let pctx = pangocairo::create_context(ctx).unwrap();
        let language = None;
        let metrics =
            pctx.get_metrics(Some(&self.font_desc), language).unwrap();
        let line_height = metrics.get_height() as f64 / pango::SCALE as f64;

        self.y = self.margin;

        for (line_idx, line) in buf.text().lines_at(pane.top_line()).enumerate()
        {
            self.draw_line(ctx, pane, buf, &line, line_idx, line_height);

            // Stop if rendering past the bottom of the widget. TODO:
            // is this the right calculation?
            if self.y > (pane.rect().height as f64) {
                break;
            }
        }
    }
}

impl App {
    pub(super) fn draw(&self, ctx: &cairo::Context) {
        for pane in self.pane_tree.panes() {
            let buf = self.buffers.get(pane.buffer_id()).unwrap();

            let mut dp = DrawPane::new();
            dp.draw(ctx, pane, buf);
        }
    }
}
