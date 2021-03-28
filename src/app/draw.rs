use {super::App, crate::pane_tree::Pane, gtk4::cairo};

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

fn draw_pane(app: &App, ctx: &cairo::Context, pane: &Pane) {
    let buf = app.buffers.get(pane.buffer_id()).unwrap();

    ctx.select_font_face(
        "DejaVu Sans Mono",
        cairo::FontSlant::Normal,
        cairo::FontWeight::Normal,
    );
    ctx.set_font_size(18.0);
    let font_extents = ctx.font_extents();

    let margin = 2.0;
    let mut y = margin;

    for (line_idx, line) in buf.text().lines_at(pane.top_line()).enumerate() {
        let line_idx = line_idx + pane.top_line();

        y += font_extents.height;

        ctx.move_to(margin, y);

        let v1 = 220.0 / 255.0;
        let v2 = 204.0 / 255.0;
        ctx.set_source_rgb(v1, v1, v2);

        let style_spans = &buf.style_spans()[line_idx];

        let mut char_iter = line.chars();
        let mut line_offset = 0;
        for span in style_spans {
            set_source_from_syntect_color(ctx, &span.style.foreground);

            for _ in 0..span.len {
                let c = char_iter.next().unwrap();
                let cs = c.to_string();

                // Set style for cursor.
                let is_cursor = line_idx == pane.cursor().line
                    && line_offset == pane.cursor().line_offset;
                if is_cursor {
                    let size = ctx.text_extents(&cs);
                    let cur_point = ctx.get_current_point();
                    // TODO: color from theme
                    set_source_rgb_from_u8(ctx, 237, 212, 0);
                    ctx.rectangle(
                        cur_point.0,
                        cur_point.1 - font_extents.ascent,
                        size.x_advance,
                        font_extents.height,
                    );
                    if pane.is_active() {
                        ctx.fill();
                    } else {
                        ctx.stroke();
                    }
                    ctx.move_to(cur_point.0, cur_point.1);

                    if pane.is_active() {
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
                    set_source_from_syntect_color(ctx, &span.style.foreground);
                }

                line_offset += 1;
            }
        }

        // Stop if rendering past the bottom of the widget. TODO:
        // is this the right calculation?
        if y > (pane.rect().height as f64) {
            break;
        }
    }
}

impl App {
    pub(super) fn draw(&self, ctx: &cairo::Context, width: i32, height: i32) {
        // Fill in the background.
        ctx.rectangle(0.0, 0.0, width as f64, height as f64);
        set_source_rgb_from_u8(ctx, 63, 63, 255);
        ctx.fill();

        // TODO
        for pane in self.pane_tree.panes() {
            let rect = pane.rect();
            ctx.rectangle(rect.x, rect.y, rect.width, rect.height);
            set_source_rgb_from_u8(ctx, 63, 63, 63);
            ctx.fill();

            draw_pane(self, ctx, pane);
        }
    }
}
