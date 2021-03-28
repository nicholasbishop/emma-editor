#![allow(dead_code)] // TODO

use crate::{
    buffer::{BufferId, Position},
    util,
};

#[derive(Debug, Clone, Eq, PartialEq)]
struct PaneId(String);

impl PaneId {
    fn new() -> PaneId {
        PaneId(util::make_id("pane"))
    }
}

#[derive(Debug, Clone, Eq, PartialEq)]
enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Default, Clone, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

pub struct Pane {
    id: PaneId,

    buffer_id: BufferId,
    rect: Rect,

    top_line: usize,
    cursor: Position,
    is_active: bool,
}

impl Pane {
    pub fn buffer_id(&self) -> &BufferId {
        &self.buffer_id
    }

    pub fn rect(&self) -> &Rect {
        &self.rect
    }

    pub fn top_line(&self) -> usize {
        self.top_line
    }

    pub fn cursor(&self) -> Position {
        self.cursor
    }

    pub fn is_active(&self) -> bool {
        self.is_active
    }
}

struct Internal {
    orientation: Orientation,
    children: Vec<Node>,
}

enum Node {
    Internal(Internal),
    Leaf(Pane),
}

pub struct PaneTree {
    root: Node,
}

impl PaneTree {
    pub fn new(buffer_id: BufferId) -> PaneTree {
        PaneTree {
            root: Node::Leaf(Pane {
                id: PaneId::new(),
                buffer_id,
                rect: Rect::default(),
                top_line: 0,
                cursor: Position::default(),
                is_active: true,
            }),
        }
    }

    pub fn recalc_layout(&mut self, width: f64, height: f64) {
        // TODO
        if let Node::Leaf(pane) = &mut self.root {
            pane.rect.width = width;
            pane.rect.height = height;
        }
    }

    pub fn panes(&self) -> Vec<&Pane> {
        let mut r = Vec::new();
        // TODO
        if let Node::Leaf(pane) = &self.root {
            r.push(pane);
        }
        r
    }
}
