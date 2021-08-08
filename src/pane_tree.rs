use {
    crate::{
        app::{BufferMap, LineHeight},
        buffer::{Buffer, BufferId, CharIndex},
        util,
    },
    std::fmt,
};

#[derive(Debug, Clone, Eq, Hash, PartialEq)]
pub struct PaneId(String);

impl PaneId {
    fn new() -> PaneId {
        PaneId(util::make_id("pane"))
    }

    fn minibuf() -> PaneId {
        PaneId("pane-minibuf".into())
    }
}

impl fmt::Display for PaneId {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.0)
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

impl Rect {
    pub fn bottom(&self) -> f64 {
        self.y + self.height
    }
}

#[derive(Clone, Debug)]
pub struct Pane {
    id: PaneId,

    buffer_id: BufferId,
    rect: Rect,

    top_line: usize,
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

    pub fn is_active(&self) -> bool {
        self.is_active
    }

    pub fn show_info_bar(&self) -> bool {
        self.show_info_bar
    }

    pub fn is_cursor_visible(&self) -> bool {
        self.is_cursor_visible
    }

    pub fn switch_buffer(
        &mut self,
        buffers: &mut BufferMap,
        new_buf_id: &BufferId,
    ) {
        let old_buf = buffers.get_mut(&self.buffer_id).unwrap();
        old_buf.remove_cursor(self);

        let new_buf = buffers.get_mut(new_buf_id).unwrap();
        new_buf.set_cursor(self, CharIndex::default());

        // TODO: think about what should happen to the cursor when a
        // buffer is viewed by only one pane, then that pane switches
        // away from the buffer so no panes are showing it, then a
        // pane shows the buffer again. It aught to put the cursor at
        // the same place, but currently we don't have anything like
        // that...

        self.buffer_id = new_buf_id.clone();
    }

    // If the cursor is not visible in the pane, scroll it so that the
    // cursor is vertically in the middle of the pane.
    pub fn maybe_rescroll(
        &mut self,
        buf: &Buffer,
        pos: CharIndex,
        line_height: LineHeight,
    ) {
        let line_height = line_height.0;
        let line_index = buf.text().char_to_line(pos.0);

        let top =
            (line_index as f64 - self.top_line as f64 + 1.0) * line_height;
        let bottom = top + line_height;
        if top < self.rect.y || bottom > self.rect.bottom() {
            // Scroll current line to middle of the screen.
            let half_height = self.rect.height / 2.0;
            let half_height_in_lines =
                (half_height / line_height).round() as usize;
            self.top_line = line_index.saturating_sub(half_height_in_lines);
        }
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

    fn find_leaf<F>(&self, f: F) -> Option<&Node>
    where
        F: Fn(&Pane) -> bool,
    {
        match self {
            Node::Leaf(leaf) => {
                if f(leaf) {
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

    fn active(&self) -> Option<&Node> {
        self.find_leaf(|leaf| leaf.is_active())
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
    is_minibuf_interactive: bool,
    active_id_before_minibuf: Option<PaneId>,
}

impl PaneTree {
    pub fn new(
        initial_buffer: &mut Buffer,
        minibuf_buffer: &mut Buffer,
    ) -> PaneTree {
        let initial_pane = Pane {
            id: PaneId::new(),
            buffer_id: initial_buffer.id().clone(),
            rect: Rect::default(),
            top_line: 0,
            is_active: true,
            show_info_bar: true,
            is_cursor_visible: true,
        };
        let minibuf_pane = Pane {
            id: PaneId::minibuf(),
            buffer_id: minibuf_buffer.id().clone(),
            rect: Rect::default(),
            top_line: 0,
            is_active: false,
            show_info_bar: false,
            is_cursor_visible: false,
        };
        initial_buffer.set_cursor(&initial_pane, CharIndex::default());
        minibuf_buffer.set_cursor(&minibuf_pane, CharIndex::default());
        PaneTree {
            root: Node::Leaf(initial_pane),
            minibuf: minibuf_pane,
            is_minibuf_interactive: true,
            active_id_before_minibuf: None,
        }
    }

    pub fn recalc_layout(
        &mut self,
        width: f64,
        height: f64,
        line_height: LineHeight,
    ) {
        let minibuf_height = line_height.0;
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
        if self.is_minibuf_interactive && self.minibuf.is_active() {
            return &self.minibuf;
        }

        if let Some(Node::Leaf(pane)) = self.root.active() {
            pane
        } else {
            panic!("no active pane");
        }
    }

    pub fn active_mut(&mut self) -> &mut Pane {
        if self.is_minibuf_interactive && self.minibuf.is_active() {
            return &mut self.minibuf;
        }

        if let Some(Node::Leaf(pane)) = self.root.active_mut() {
            pane
        } else {
            panic!("no active pane");
        }
    }

    pub fn active_excluding_minibuf(&self) -> &Pane {
        if let Some(id) = &self.active_id_before_minibuf {
            if let Some(Node::Leaf(pane)) =
                self.root.find_leaf(|pane| &pane.id == id)
            {
                pane
            } else {
                panic!("invalid active_id_before_minibuf");
            }
        } else if let Some(Node::Leaf(pane)) = self.root.active() {
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

    pub fn split(&mut self, orientation: Orientation, buf: &mut Buffer) {
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
            // Copy the active pane's cursor.
            buf.set_cursor(&new_pane, buf.cursor(active));
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
        self.minibuf.is_active = &self.minibuf.id == id;
    }

    pub fn set_minibuf_interactive(&mut self, interactive: bool) {
        self.is_minibuf_interactive = interactive;
        self.minibuf.is_cursor_visible = interactive;
        let minibuf_id = self.minibuf.id.clone();
        if interactive {
            self.active_id_before_minibuf = Some(self.active().id().clone());
            self.set_active(&minibuf_id);
        } else {
            // TODO: check this case better. If the minibuf is
            // transitioning from interactive -> not-interactive then
            // active_id_before_minibuf should always be set, but not
            // when transitioning from not-interactive ->
            // not-interactive.
            if let Some(id) = self.active_id_before_minibuf.take() {
                self.set_active(&id);
            }
        }
    }
}
