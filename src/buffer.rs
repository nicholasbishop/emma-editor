use {
    crate::{theme, util},
    anyhow::Error,
    fehler::throws,
    ropey::Rope,
    std::{
        fs, io,
        path::{Path, PathBuf},
    },
    syntect::{
        highlighting::{
            HighlightState, Highlighter, RangedHighlightIterator, Style,
        },
        parsing::{ParseState, ScopeStack, SyntaxSet},
    },
};

#[derive(Debug, Clone, Eq, PartialEq, Hash)]
pub struct BufferId(String);

impl BufferId {
    pub fn new() -> BufferId {
        BufferId(util::make_id("buffer"))
    }
}

#[derive(Debug)]
struct StyleSpan {
    len: usize,
    style: Style,
}

pub struct Buffer {
    text: Rope,
    path: PathBuf,

    // Outer vec: per line
    // Inner vec: style for a contiguous group of chars, covers the
    // whole line.
    // TODO: think about a smarter structure
    // TODO: put in arc for async update
    style_spans: Vec<Vec<StyleSpan>>,
}

impl Buffer {
    #[throws]
    pub fn from_path(path: &Path) -> Buffer {
        let text =
            Rope::from_reader(&mut io::BufReader::new(fs::File::open(path)?))?;
        let mut buf = Buffer {
            text,
            path: path.into(),
            style_spans: Vec::new(),
        };

        // TODO, async
        buf.recalc_style_spans();

        buf
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
