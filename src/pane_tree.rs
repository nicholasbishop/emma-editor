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

#[derive(Debug, Clone, Copy, Eq, PartialEq)]
pub enum Orientation {
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

    pub fn set_cursor(&mut self, cursor: Position) {
        self.cursor = cursor;
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

impl Node {
    fn active_mut(&mut self) -> Option<&mut Node> {
        match self {
            Node::Leaf(leaf) => {
                if leaf.is_active() {
                    Some(self)
                } else {
                    None
                }
            }
            Node::Internal(internal) => {
                for child in &mut internal.children {
                    if let Some(active) = Self::active_mut(child) {
                        return Some(active);
                    }
                }
                None
            }
        }
    }
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

    pub fn active_mut(&mut self) -> &mut Pane {
        if let Some(Node::Leaf(pane)) = self.root.active_mut() {
            pane
        } else {
            panic!("no active pane");
        }
    }

    pub fn split(&mut self, orientation: Orientation) {
        todo!();
    }
}
