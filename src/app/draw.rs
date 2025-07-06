use crate::app::{AppState, Overlay};
use crate::buffer::{
    Buffer, LineMatches, LinePosition, LinesIterItem, StyleSpan, StyledLine,
};
use crate::grapheme::next_grapheme_boundary;
use crate::pane_tree::Pane;
use crate::rope::{LineDataVec, RopeSlice};
use crate::theme::Theme;
use anyhow::Result;
use gtk4::pango::{self, Layout};
use gtk4::prelude::*;
use gtk4::{self as gtk, cairo};
use std::fmt;
use std::ops::Range;
use syntect::highlighting::Style;
use tracing::{debug, error, instrument};

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

#[derive(Clone, Copy, Debug)]
pub struct LineHeight(pub f64);

#[derive(Default)]
struct Point {
    x: f64,
    y: f64,
}

impl fmt::Display for Point {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "({}, {})", self.x, self.y)
    }
}

impl LineHeight {
    pub fn calculate(widget: &gtk::DrawingArea) -> Self {
        let pctx = widget.pango_context();
        let font_desc = pctx.font_description();

        let language = None;
        let metrics = pctx.metrics(font_desc.as_ref(), language);

        Self(pango_unscale(metrics.height()))
    }
}

struct StyledLayout {
    layout: Layout,
    // TODO: this should be a reference but then things get *really*
    // complicated with the borrow checker.
    style: Style,
    is_cursor: bool,
}

fn apply_match_style(
    base_spans: &StyledLine,
    matches: &LineMatches,
    match_style: &Style,
) -> StyledLine {
    // TODO: the way this is implemented is almost certainly not the
    // best way to do it, but seems reasonably easy to verify.

    // TODO: share between outer iterations
    let mut output = Vec::with_capacity(base_spans.0.len());

    // TODO: share between outer iterations
    let mut scratch =
        Vec::with_capacity(base_spans.0.iter().map(|s| s.len).sum());

    // Fill in the base indices.
    for (base_index, base_span) in base_spans.0.iter().enumerate() {
        for _ in 0..base_span.len {
            scratch.push(base_index);
        }
    }

    // Override with match markers.
    let match_marker = usize::MAX;
    for match_span in &matches.spans {
        // TODO
        #[allow(clippy::needless_range_loop)]
        for index in match_span.start..match_span.end {
            scratch[index] = match_marker;
        }
    }

    // Go through the scratch vec and convert back to spans.
    let mut span_len = 0;
    for scratch_index in 0..scratch.len() {
        let cur = scratch[scratch_index];
        let next = scratch.get(scratch_index + 1);
        span_len += 1;

        if Some(&cur) != next {
            let style = if cur == match_marker {
                match_style
            } else {
                &base_spans.0[cur].style
            };
            output.push(StyleSpan {
                len: span_len,
                style: *style,
            });
            span_len = 0;
        }
    }

    StyledLine(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn match_style() -> Style {
        Style::default()
    }

    fn style1() -> Style {
        let mut style = Style::default();
        style.foreground.r = 1;
        style
    }

    #[test]
    fn test_apply_match_style() {
        let base_spans = StyledLine(vec![StyleSpan {
            len: 5,
            style: style1(),
        }]);
        let mut matches = LineMatches { spans: vec![] };

        fn label(styled_line: &StyledLine) -> Vec<(&'static str, usize)> {
            styled_line
                .0
                .iter()
                .map(|span| {
                    let name = if span.style == match_style() {
                        "match"
                    } else if span.style == style1() {
                        "style1"
                    } else {
                        "unknown"
                    };
                    (name, span.len)
                })
                .collect()
        }

        fn check(
            base_spans: &StyledLine,
            matches: &LineMatches,
            expected: &[(&str, usize)],
        ) {
            let mods = apply_match_style(&base_spans, matches, &match_style());
            assert_eq!(label(&mods), expected);
        }

        // No matches
        check(&base_spans, &matches, &[("style1", 5)]);

        // One match, replaces the base span
        matches.spans = vec![0..5];
        check(&base_spans, &matches, &[("match", 5)]);

        // One match at the start of the base span
        matches.spans = vec![0..3];
        check(&base_spans, &matches, &[("match", 3), ("style1", 2)]);

        // One match at the end of the base span
        matches.spans = vec![3..5];
        check(&base_spans, &matches, &[("style1", 3), ("match", 2)]);
    }
}

struct DrawPane<'a> {
    ctx: &'a cairo::Context,
    widget: &'a gtk::DrawingArea,
    pane: &'a Pane,
    buf: &'a Buffer,
    line_height: LineHeight,
    theme: &'a Theme,
    span_buf: String,
    margin: f64,
    cursor: LinePosition,
    len_lines: usize,
    pos: Point,
}

impl fmt::Debug for DrawPane<'_> {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(
            f,
            "DrawPane({}, {}, pos={})",
            self.pane.id(),
            self.buf.id(),
            self.pos,
        )
    }
}

impl DrawPane<'_> {
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
        self.ctx.move_to(self.pos.x, self.pos.y);
        pangocairo::functions::show_layout(self.ctx, layout);
        self.pos.x += pango_unscale(layout.size().0);
    }

    fn styled_layouts_from_line(
        &mut self,
        line: &LinesIterItem,
    ) -> LineDataVec<StyledLayout> {
        let mut output = LineDataVec::new(line.index);

        let match_style = Style {
            background: self.theme.search_match.background,
            foreground: self.theme.search_match.foreground,
            ..Style::default()
        };

        let base_style_spans = self.buf.style_spans().get(line.index).unwrap();
        let mut style_spans = base_style_spans;
        // TODO: share across iterations
        let modified_style_spans;
        if let Some(search) = self.buf.search_state() {
            if let Some(matches) = search.line_matches(self.pane, line.index) {
                modified_style_spans =
                    apply_match_style(base_style_spans, matches, &match_style);

                style_spans = &modified_style_spans;
            }
        }

        let mut span_offset = 0;
        for span in &style_spans.0 {
            debug!("span: {} chars", span.len);
            let mut push =
                |me: &mut DrawPane, range: Range<usize>, is_cursor| {
                    if !range.is_empty() {
                        output.push(StyledLayout {
                            layout: me.layout_line_range(&line.slice, range),
                            style: span.style,
                            is_cursor,
                        });
                    }
                };

            let span_range = span_offset..span_offset + span.len;
            span_offset += span.len;

            if line.index == self.cursor.line
                && span_range.contains(&self.cursor.offset.0)
            {
                debug!("span contains cursor");
                push(self, span_range.start..self.cursor.offset.0, false);

                let cursor_end_char =
                    next_grapheme_boundary(&line.slice, self.cursor.offset.0);

                push(self, self.cursor.offset.0..cursor_end_char.0, true);
                push(self, cursor_end_char.0..span_range.end, false);
            } else {
                push(self, span_range, false);
            }
        }

        // The last line of the buffer by definition doesn't end in a
        // new line. (If the last character in a file is a newline,
        // ropey's iterator produces an empty line at the end.) We
        // still need to draw the cursor in that case though, so
        // append it here.
        if self.cursor.line == line.index
            && line.index.0 + 1 == self.len_lines
            && self.cursor.offset.0 == line.slice.len_chars()
        {
            debug!("eof cursor");
            output.push(StyledLayout {
                layout: self.create_layout(""),
                style: Style::default(),
                is_cursor: true,
            });
            return output;
        }

        output
    }

    fn draw_cursor(&mut self, styled_layout: &StyledLayout) -> Result<()> {
        if !self.pane.is_cursor_visible() {
            debug!("cursor not visible");
            return Ok(());
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
        let mut cursor_width = pango_unscale(styled_layout.layout.size().0);
        if cursor_width == 0.0 {
            // TODO: this is needed for at least newlines,
            // which give (0, double-line-height), but
            // might need to think about other kinds of
            // not-really-rendered characters as well.
            cursor_width = self.line_height.0 / 2.0;
        }
        debug!(
            "drawing cursor: size={}x{}",
            cursor_width, self.line_height.0
        );
        self.ctx.rectangle(
            self.pos.x,
            self.pos.y,
            cursor_width,
            self.line_height.0,
        );
        if self.pane.is_active() {
            self.ctx.fill()?;
        } else {
            self.ctx.stroke()?;
        }

        Ok(())
    }

    fn draw_line(&mut self, line: &LinesIterItem) -> Result<()> {
        self.pos.x = self.pane.rect().x;

        self.ctx.move_to(self.margin, self.pos.y);

        set_source_rgb_from_u8(self.ctx, 220, 220, 204);

        let styled_layouts = self.styled_layouts_from_line(line);

        for styled_layout in styled_layouts.iter() {
            let styled_layout = &styled_layout.data;

            // Draw background
            set_source_from_syntect_color(
                self.ctx,
                &styled_layout.style.background,
            );
            let size = styled_layout.layout.size();
            self.ctx.rectangle(
                self.pos.x,
                self.pos.y,
                pango_unscale(size.0),
                pango_unscale(size.1),
            );
            self.ctx.fill()?;

            if styled_layout.is_cursor {
                self.draw_cursor(styled_layout)?;

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

        self.pos.y += self.line_height.0;
        Ok(())
    }

    fn draw_info_bar(&mut self) -> Result<()> {
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
            rect.y + rect.height - self.line_height.0,
            rect.width,
            self.line_height.0,
        );
        self.ctx.fill()?;

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

            self.pos.x = rect.x;
            self.pos.y = rect.y + rect.height - self.line_height.0;
            self.draw_layout(&layout);
        }

        Ok(())
    }

    #[instrument]
    fn draw(&mut self) -> Result<()> {
        debug!("drawing pane with {} lines", self.buf.text().len_lines());

        // Fill in the background. Subtract small amount from the
        // right edge to give a border.
        let rect = self.pane.rect();
        let border = 0.5;
        self.ctx
            .rectangle(rect.x, rect.y, rect.width - border, rect.height);
        set_source_rgb_from_u8(self.ctx, 63, 63, 63);
        self.ctx.fill()?;

        self.cursor = LinePosition::from_abs_char(
            self.buf.cursor(self.pane.id()),
            self.buf,
        );

        self.pos.y = rect.y + self.margin;

        for line in self.buf.text().lines_at(self.pane.top_line()) {
            self.draw_line(&line)?;

            // Stop if rendering past the bottom of the widget. TODO:
            // is this the right calculation?
            if self.pos.y > (rect.y + rect.height) {
                break;
            }
        }

        if self.pane.show_info_bar() {
            self.draw_info_bar()?;
        }

        Ok(())
    }
}

impl AppState {
    // Errors here are logged but otherwise swallowed.
    pub(super) fn draw(
        &self,
        widget: &gtk::DrawingArea,
        ctx: &cairo::Context,
        width: f64,
        height: f64,
        line_height: LineHeight,
        theme: &Theme,
    ) {
        // Fill in the background. This acts as the border color
        // between panes. Don't go all the way to the right
        // edge to avoid an unwanted border there.
        let border = 1.0;
        ctx.rectangle(0.0, 0.0, width - border, height);
        set_source_rgb_from_u8(ctx, 220, 220, 204);
        if let Err(err) = ctx.fill() {
            error!("fill failed: {}", err);
        }

        let mut panes = self.pane_tree.panes();
        panes.push(self.pane_tree.minibuf());

        for pane in panes {
            let buf = self.buffers.get(pane.buffer_id()).unwrap();

            let mut dp = DrawPane {
                ctx,
                widget,
                pane,
                buf,
                line_height,
                theme,
                span_buf: String::new(),
                margin: 2.0,
                cursor: LinePosition::default(),
                len_lines: buf.text().len_lines(),
                pos: Point::default(),
            };
            if let Err(err) = dp.draw() {
                error!("failed to draw pane: {}", err);
            }
        }

        self.draw_interactive_widget(widget, ctx, line_height, theme);
    }

    fn draw_interactive_widget(
        &self,
        widget: &gtk::DrawingArea,
        ctx: &cairo::Context,
        line_height: LineHeight,
        theme: &Theme,
    ) {
        // TODO: handle other widgets

        let Some(Overlay::OpenFile(open_file)) = &self.overlay else {
            return;
        };

        // Fill in the background.
        let r = open_file.rect();
        ctx.rectangle(0.0, 0.0, r.width, r.height);
        set_source_rgb_from_u8(ctx, 63, 63, 100);
        if let Err(err) = ctx.fill() {
            error!("fill failed: {}", err);
        }

        // Vertical drop shadow at the bottom of the widget.
        let shadow_height = 20.0;
        let gradient = cairo::LinearGradient::new(
            0.0,
            r.bottom(),
            0.0,
            r.bottom() + shadow_height,
        );
        gradient.add_color_stop_rgba(0.0, 0.0, 0.0, 0.0, 1.0);
        gradient.add_color_stop_rgba(1.0, 0.0, 0.0, 0.0, 0.0);
        ctx.set_source(gradient).unwrap();
        ctx.rectangle(0.0, r.bottom(), r.width, shadow_height);
        if let Err(err) = ctx.fill() {
            error!("fill failed: {}", err);
        }

        // Prompt.
        let layout = widget.create_pango_layout(Some("Open file:"));
        set_source_rgb_from_u8(ctx, 200, 200, 200);
        ctx.move_to(r.x, r.y);
        pangocairo::functions::show_layout(ctx, &layout);

        let buf = open_file.buffer();
        // TODO: dedup?
        let mut dp = DrawPane {
            ctx,
            widget,
            pane: open_file.pane(),
            buf,
            line_height,
            theme,
            span_buf: String::new(),
            margin: 2.0,
            cursor: LinePosition::default(),
            len_lines: buf.text().len_lines(),
            pos: Point::default(),
        };
        if let Err(err) = dp.draw() {
            error!("failed to draw pane: {}", err);
        }

        // Suggestions.
        let layout = widget.create_pango_layout(Some(&open_file.suggestions()));
        set_source_rgb_from_u8(ctx, 200, 200, 200);
        ctx.move_to(r.x, r.y + line_height.0 * 2.0);
        pangocairo::functions::show_layout(ctx, &layout);
    }
}
