use crate::{
    app::Font,
    buffer::{BufferId, Position},
    util,
};

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct PaneId(String);

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

#[derive(Clone)]
pub struct Pane {
    id: PaneId,

    buffer_id: BufferId,
    rect: Rect,

    top_line: usize,
    cursor: Position,
    is_active: bool,
    show_info_bar: bool,
    is_cursor_visible: bool,
}

impl Pane {
    pub fn id(&self) -> &PaneId {
        &self.id
    }

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

    pub fn show_info_bar(&self) -> bool {
        self.show_info_bar
    }

    pub fn is_cursor_visible(&self) -> bool {
        self.is_cursor_visible
    }
}

enum SplitResult {
    Split([Node; 2]),
    Single(Node),
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
    fn leaf(&self) -> Option<&Pane> {
        if let Node::Leaf(pane) = self {
            Some(pane)
        } else {
            None
        }
    }

    fn active(&self) -> Option<&Node> {
        match self {
            Node::Leaf(leaf) => {
                if leaf.is_active() {
                    Some(self)
                } else {
                    None
                }
            }
            Node::Internal(internal) => {
                for child in &internal.children {
                    if let Some(active) = Self::active(child) {
                        return Some(active);
                    }
                }
                None
            }
        }
    }

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

    fn panes(&self) -> Vec<&Pane> {
        match self {
            Node::Leaf(pane) => vec![pane],
            Node::Internal(internal) => {
                internal.children.iter().flat_map(|n| n.panes()).collect()
            }
        }
    }

    fn panes_mut(&mut self) -> Vec<&mut Pane> {
        match self {
            Node::Leaf(pane) => vec![pane],
            Node::Internal(internal) => internal
                .children
                .iter_mut()
                .flat_map(|n| n.panes_mut())
                .collect(),
        }
    }

    fn recalc_layout(&mut self, rect: Rect) {
        match self {
            Node::Leaf(leaf) => {
                leaf.rect = rect;
            }
            Node::Internal(internal) => match internal.orientation {
                Orientation::Horizontal => {
                    let mut x = rect.x;
                    let width = rect.width / internal.children.len() as f64;
                    for child in &mut internal.children {
                        child.recalc_layout(Rect {
                            x,
                            y: rect.y,
                            width,
                            height: rect.height,
                        });
                        x += width;
                    }
                }
                Orientation::Vertical => {
                    let mut y = rect.y;
                    let height = rect.height / internal.children.len() as f64;
                    for child in &mut internal.children {
                        child.recalc_layout(Rect {
                            x: rect.x,
                            y,
                            width: rect.width,
                            height,
                        });
                        y += height;
                    }
                }
            },
        }
    }

    fn split(
        self,
        orientation: Orientation,
        active_pane_id: &PaneId,
        new_pane: Pane,
    ) -> SplitResult {
        if self.leaf().map(|pane| &pane.id) == Some(active_pane_id) {
            return SplitResult::Split([self, Node::Leaf(new_pane)]);
        }

        if let Node::Internal(mut internal) = self {
            let mut new_children: Vec<Node> = Vec::new();
            let mut new_orientation = internal.orientation;
            let num_children = internal.children.len();
            for child in internal.children {
                match child.split(orientation, active_pane_id, new_pane.clone())
                {
                    SplitResult::Split([child1, child2]) => {
                        if num_children == 1 {
                            // Node has only one child, so just align
                            // the orientation with the split
                            // orientation.
                            new_orientation = orientation;
                        }

                        if orientation == new_orientation {
                            // Orientation matches, so just add the
                            // new child in the appropriate place.
                            new_children.push(child1);
                            new_children.push(child2);
                        } else {
                            // Orientation doesn't match so a new
                            // internal node is needed.
                            new_children.push(Node::Internal(Internal {
                                orientation,
                                children: vec![child1, child2],
                            }));
                        }
                    }
                    SplitResult::Single(child) => new_children.push(child),
                }
            }
            internal.children = new_children;
            internal.orientation = new_orientation;
            SplitResult::Single(Node::Internal(internal))
        } else {
            SplitResult::Single(self)
        }
    }
}

pub struct PaneTree {
    root: Node,
    minibuf: Pane,
}

impl PaneTree {
    pub fn new(
        initial_buffer_id: BufferId,
        minibuf_buffer_id: BufferId,
    ) -> PaneTree {
        PaneTree {
            root: Node::Leaf(Pane {
                id: PaneId::new(),
                buffer_id: initial_buffer_id,
                rect: Rect::default(),
                top_line: 0,
                cursor: Position::default(),
                is_active: true,
                show_info_bar: true,
                is_cursor_visible: true,
            }),
            minibuf: Pane {
                id: PaneId::new(),
                buffer_id: minibuf_buffer_id,
                rect: Rect::default(),
                top_line: 0,
                cursor: Position::default(),
                is_active: false,
                show_info_bar: false,
                is_cursor_visible: false,
            },
        }
    }

    pub fn recalc_layout(&mut self, width: f64, height: f64, font: &Font) {
        let minibuf_height = font.line_height();
        self.minibuf.rect = Rect {
            x: 0.0,
            y: height - minibuf_height,
            width,
            height: minibuf_height,
        };
        self.root.recalc_layout(Rect {
            x: 0.0,
            y: 0.0,
            width,
            height: height - minibuf_height,
        });
    }

    pub fn panes(&self) -> Vec<&Pane> {
        self.root.panes()
    }

    pub fn minibuf(&self) -> &Pane {
        &self.minibuf
    }

    pub fn panes_mut(&mut self) -> Vec<&mut Pane> {
        self.root.panes_mut()
    }

    pub fn active(&self) -> &Pane {
        if let Some(Node::Leaf(pane)) = self.root.active() {
            pane
        } else {
            panic!("no active pane");
        }
    }

    pub fn active_mut(&mut self) -> &mut Pane {
        if let Some(Node::Leaf(pane)) = self.root.active_mut() {
            pane
        } else {
            panic!("no active pane");
        }
    }

    fn take_root(&mut self) -> Node {
        // TODO: this seems silly, creating a temporary unused node
        // just so I can move out of self.root, not sure how to avoid
        // though.
        std::mem::replace(
            &mut self.root,
            Node::Internal(Internal {
                orientation: Orientation::Horizontal,
                children: Vec::new(),
            }),
        )
    }

    pub fn split(&mut self, orientation: Orientation) {
        let active_id;
        let new_pane;
        {
            let active = self.active();
            active_id = active.id.clone();
            new_pane = Pane {
                id: PaneId::new(),
                is_active: false,
                ..active.clone()
            };
        }

        // TODO: make just have this method take self instead?
        let root = self.take_root();
        self.root = match root.split(orientation, &active_id, new_pane) {
            SplitResult::Single(single) => single,
            SplitResult::Split([child1, child2]) => Node::Internal(Internal {
                orientation,
                children: vec![child1, child2],
            }),
        }
    }

    pub fn set_active(&mut self, id: &PaneId) {
        for pane in self.panes_mut() {
            pane.is_active = &pane.id == id;
        }
    }
}
