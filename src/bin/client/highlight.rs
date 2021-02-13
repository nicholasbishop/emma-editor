use {
    crate::{
        buffer::{Buffer, BufferGeneration, BufferId},
        theme, APP,
    },
    crossbeam_channel::Receiver,
    gtk4::{self as gtk, gdk, glib, prelude::*},
    std::{
        collections::HashMap,
        hash::{Hash, Hasher},
        ops::Range,
        path::PathBuf,
    },
    syntect::{
        highlighting::{
            HighlightState, Highlighter, RangedHighlightIterator, Style, Theme,
        },
        parsing::{ParseState, ScopeStack, SyntaxSet},
        util::LinesWithEndings,
    },
};

#[derive(Eq, PartialEq)]
struct StyleWithHash(Style);

#[allow(clippy::derive_hash_xor_eq)]
impl Hash for StyleWithHash {
    fn hash<H: Hasher>(&self, state: &mut H) {
        let mut hash_color = |c: &syntect::highlighting::Color| {
            c.r.hash(state);
            c.g.hash(state);
            c.b.hash(state);
            c.a.hash(state);
        };

        // TODO: would be nice if Style just implemented Hash.
        hash_color(&self.0.foreground);
        hash_color(&self.0.background);
        self.0.font_style.hash(state);
    }
}

fn gdk_rgba_from_syntect_color(
    color: &syntect::highlighting::Color,
) -> gdk::RGBA {
    gdk::RGBA {
        red: (color.r as f32) / 255.0,
        green: (color.g as f32) / 255.0,
        blue: (color.b as f32) / 255.0,
        alpha: (color.a as f32) / 255.0,
    }
}

type HighlightSpan = (Range<i32>, Style);

// TODO: for now use an easier mode of highlighting with no
// caching or other speedups.
//
// TODO: path from buffer?
//
// TODO rename
fn calc_highlight_spans(
    syntax_set: &SyntaxSet,
    theme: &Theme,
    req: &HighlightRequest,
) -> Vec<HighlightSpan> {
    // TODO: unwraps
    let syntax = syntax_set.find_syntax_for_file(&req.path).unwrap().unwrap();

    let mut parse_state = ParseState::new(syntax);

    // TODO: our theme
    let highlighter = Highlighter::new(theme);

    let mut highlight_state =
        HighlightState::new(&highlighter, ScopeStack::new());

    // let start = buf.get_start_iter();
    // let end = buf.get_end_iter();
    // buf.remove_all_tags(&start, &end);
    // let text = buf.get_text(&start, &end, true).unwrap();

    let mut offset = 0;

    let mut spans = Vec::new();

    // TODO: maybe better to use a gtk/sourceview iter if it exists?
    for line in LinesWithEndings::from(&req.text) {
        let changes = parse_state.parse_line(&line, syntax_set);

        let iter = RangedHighlightIterator::new(
            &mut highlight_state,
            &changes,
            line,
            &highlighter,
        );

        for (style, _, range) in iter {
            spans.push((
                Range {
                    start: offset + range.start as i32,
                    end: offset + range.end as i32,
                },
                style,
            ));
        }

        offset += line.len() as i32;
    }

    spans
}

fn highlight_buffer(buf: &Buffer, spans: &[HighlightSpan]) {
    let tag_table = buf.get_tag_table();

    let mut style_to_tag = HashMap::new();

    for (range, style) in spans {
        let tag =
            style_to_tag
                .entry(StyleWithHash(*style))
                .or_insert_with(|| {
                    let tag = gtk::TextTag::new(None);
                    // TODO: set other properties
                    tag.set_property_foreground_rgba(Some(
                        &gdk_rgba_from_syntect_color(&style.foreground),
                    ));
                    tag_table.add(&tag);
                    tag
                });

        // Apply tag.
        let start = buf.get_iter_at_offset(range.start);
        let end = buf.get_iter_at_offset(range.end);
        buf.apply_tag(tag, &start, &end);
    }
}

pub struct HighlightRequest {
    pub buffer_id: BufferId,
    pub path: PathBuf,
    pub generation: BufferGeneration,

    // TODO: with big buffers copying the whole thing in memory will
    // be a problem.
    pub text: String,
}

pub fn highlighter_thread(receiver: Receiver<HighlightRequest>) {
    let syntax_set = SyntaxSet::load_defaults_newlines();
    // TODO: consider moving this to the main thread so it panics the
    // whole program
    let theme = theme::load_default_theme().unwrap();
    // Plan for speed improvement:
    //
    // Have a background thread for doing the
    // highlighting. (TODO: think about a pool.)
    //
    // When the buffer changes, send a message to the bg
    // thread. It needs to somehow include a reference to the
    // buffer, like an ID, but not access to the buffer
    // itself. The whole buffer contents is passed to the
    // thread. (TODO: future performance concern for large
    // buffers).
    //
    // The bg thread keeps a queue of buffers that need
    // highlighting. If a change comes in and the buffer is
    // already being highlighted then it stops and throws that
    // work away. Similarly if the buffer is already in the
    // queue then the previous queue entry is removed. New
    // entries take priority.
    //
    // Whenever the bg thread finishes processing a buffer it queues
    // up an operation using idle_add. That operation runs on the main
    // thread and handles actually updating the buffer. How will it
    // have a reference to the App? Sounds like we need a global for
    // that? Either that or have a persistent idle handler that checks
    // for messages ina channel.

    let mut queue: Vec<HighlightRequest> = Vec::new();

    loop {
        let req = receiver.recv().unwrap();

        // Check if the buffer is already in the queue and drop it if
        // so
        queue.retain(|elem| elem.buffer_id != req.buffer_id);

        queue.push(req);

        while !queue.is_empty() {
            // TODO: make it possible to exit the loop early if new
            // things come in on the channel

            let req = queue.pop().unwrap();

            let spans = calc_highlight_spans(&syntax_set, &theme, &req);

            // Send message back
            glib::idle_add(move || {
                // TODO: move to top-level?

                APP.with(|app| {
                    let app = app.borrow();
                    let app = app.as_ref().unwrap();

                    let buf = app
                        .buffers
                        .iter()
                        .find(|buf| buf.buffer_id() == req.buffer_id)
                        .unwrap();

                    if buf.generation() == req.generation {
                        highlight_buffer(&buf.storage(), &spans);
                    }
                });

                Continue(false)
            });
        }
    }
}
