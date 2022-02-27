use crate::buffer::StyledLine;
use crate::rope::{LineDataVec, Rope};
use tree_sitter_highlight::{HighlightConfiguration, Highlighter};

// All very TODO

const HIGHLIGHT_NAMES: &[&str] = &[
    "attribute",
    "constant",
    "function.builtin",
    "function",
    "keyword",
    "operator",
    "property",
    "punctuation",
    "punctuation.bracket",
    "punctuation.delimiter",
    "string",
    "string.special",
    "tag",
    "type",
    "type.builtin",
    "variable",
    "variable.builtin",
    "variable.parameter",
];

pub fn recalc_style_spans(
    style_spans: &mut LineDataVec<StyledLine>,
    rope: &Rope,
) {
    style_spans.clear();

    let mut highlighter = Highlighter::new();

    let mut rust_config = HighlightConfiguration::new(
        tree_sitter_rust::language(),
        tree_sitter_rust::HIGHLIGHT_QUERY,
        "",
        "",
    )
    .unwrap();

    rust_config.configure(HIGHLIGHT_NAMES);
    let highlights = highlighter
        .highlight(&rust_config, b"const x = new Y();", None, |_| None)
        .unwrap();
}
