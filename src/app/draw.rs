use {
    super::App,
    crate::{
        buffer::Buffer, grapheme::next_grapheme_boundary, pane_tree::Pane,
    },
    gtk4::{
        cairo,
        pango::{self, FontDescription},
    },
    ropey::RopeSlice,
    std::ops::Range,
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

struct DrawPane {
    font_desc: FontDescription,
    span_buf: String,
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
            x: 0.0,
            y: 0.0,
        }
    }

    fn draw_line_range(
        &mut self,
        ctx: &cairo::Context,
        line: &RopeSlice,
        range: Range<usize>,
    ) {
        self.span_buf.clear();
        for chunk in line.slice(range.clone()).chunks() {
            self.span_buf.push_str(chunk);
        }

        let layout = pangocairo::create_layout(ctx).unwrap();
        layout.set_font_description(Some(&self.font_desc));
        layout.set_text(&self.span_buf);
        ctx.move_to(self.x, self.y);
        pangocairo::show_layout(ctx, &layout);
        self.x += layout.get_size().0 as f64 / pango::SCALE as f64;
    }

    fn draw(&mut self, ctx: &cairo::Context, pane: &Pane, buf: &Buffer) {
        // Fill in the background.
        let rect = pane.rect();
        ctx.rectangle(rect.x, rect.y, rect.width, rect.height);
        set_source_rgb_from_u8(ctx, 63, 63, 63);
        ctx.fill();

        let cursor_line_pos = pane.cursor().line_position(buf);

        let font_extents = ctx.font_extents();

        let margin = 2.0;
        self.y = margin;

        for (line_idx, line) in buf.text().lines_at(pane.top_line()).enumerate()
        {
            let line_idx = line_idx + pane.top_line();

            self.x = 0.0;
            self.y += font_extents.height;

            ctx.move_to(margin, self.y);

            set_source_rgb_from_u8(ctx, 220, 220, 204);

            let style_spans = &buf.style_spans()[line_idx];

            let mut span_offset = 0;
            for span in style_spans {
                set_source_from_syntect_color(ctx, &span.style.foreground);

                let span_range = span_offset..span_offset + span.len;
                span_offset += span.len;

                let first_range;
                let cursor_ranges;
                if line_idx == cursor_line_pos.line
                    && span_range.contains(&cursor_line_pos.offset)
                {
                    first_range = span_range.start..cursor_line_pos.offset;

                    let cursor_end_char =
                        next_grapheme_boundary(&line, cursor_line_pos.offset);

                    cursor_ranges = Some((
                        cursor_line_pos.offset..cursor_end_char,
                        cursor_end_char..span_range.end,
                    ));
                } else {
                    first_range = span_range;
                    cursor_ranges = None;
                }

                self.draw_line_range(ctx, &line, first_range);

                if let Some((second_range, third_range)) = cursor_ranges {
                    // Draw cursor
                    // TODO: color from theme
                    set_source_rgb_from_u8(ctx, 237, 212, 0);
                    ctx.rectangle(self.x, self.y, 20.0, 20.0);
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
                    self.draw_line_range(ctx, &line, second_range);

                    // Restore text color and draw the rest of the span.
                    set_source_from_syntect_color(ctx, &span.style.foreground);
                    self.draw_line_range(ctx, &line, third_range);
                }
            }

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
