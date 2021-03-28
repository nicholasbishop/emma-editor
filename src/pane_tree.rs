use crate::{buffer::BufferId, util};

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

    pub buffer_id: BufferId,
    pub rect: Rect,
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
    active: PaneId,
}

impl PaneTree {
    pub fn new(buffer_id: BufferId) -> PaneTree {
        let pane_id = PaneId::new();
        PaneTree {
            root: Node::Leaf(Pane {
                id: pane_id.clone(),
                buffer_id,
                rect: Rect::default(),
            }),
            active: pane_id,
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
