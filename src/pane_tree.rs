// TODO: clippy bug? Triggering on Orientation enum.
#![allow(clippy::use_self)]

use crate::app::{BufferMap, LineHeight};
use crate::buffer::{AbsChar, Buffer, BufferId, RelLine};
use crate::rope::AbsLine;
use crate::util;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::fmt;

#[derive(Debug, Clone, Eq, Hash, PartialEq, Deserialize, Serialize)]
pub struct PaneId(String);

impl PaneId {
    pub fn new() -> Self {
        Self(util::make_id("pane"))
    }
}

impl fmt::Display for PaneId {
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, Eq, PartialEq, Deserialize, Serialize)]
pub enum Orientation {
    Horizontal,
    Vertical,
}

#[derive(Debug, Default, Clone, PartialEq, Deserialize, Serialize)]
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

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct Pane {
    id: PaneId,

    buffer_id: BufferId,
    rect: Rect,

    top_line: AbsLine,
    is_active: bool,
    show_info_bar: bool,
    is_cursor_visible: bool,
}

impl Pane {
    // Create a one-off pane for use in a widget (e.g. open_file).
    pub fn create_for_widget(buffer_id: BufferId) -> Self {
        Self {
            id: PaneId::new(),
            buffer_id,
            rect: Rect::default(),
            top_line: AbsLine::zero(),
            is_active: true,
            show_info_bar: false,
            is_cursor_visible: true,
        }
    }

    pub fn id(&self) -> &PaneId {
        &self.id
    }

    pub fn buffer_id(&self) -> &BufferId {
        &self.buffer_id
    }

    pub fn rect(&self) -> &Rect {
        &self.rect
    }

    pub fn set_rect(&mut self, rect: Rect) {
        self.rect = rect;
    }

    pub fn top_line(&self) -> AbsLine {
        self.top_line
    }

    pub fn set_top_line(&mut self, top_line: AbsLine) {
        self.top_line = top_line;
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
        // When loading persistent buffers, the "old buffer" might not
        // actually exist, so make removing the cursor conditional.
        if let Some(old_buf) = buffers.get_mut(&self.buffer_id) {
            old_buf.remove_cursor(self);
        }

        let new_buf = buffers.get_mut(new_buf_id).unwrap();
        new_buf.set_cursor(self.id(), AbsChar::default());

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
        pos: AbsChar,
        line_height: LineHeight,
    ) {
        let line_height = line_height.0;
        let line_index = buf.text().char_to_line(pos);

        let top =
            (line_index.0 as f64 - self.top_line.0 as f64 + 1.0) * line_height;
        let bottom = top + line_height;
        if top < self.rect.y || bottom > self.rect.bottom() {
            // Scroll current line to middle of the screen.
            let half_height = self.rect.height / 2.0;
            let half_height_in_lines =
                (half_height / line_height).round() as usize;
            self.top_line =
                line_index.saturating_sub(RelLine::new(half_height_in_lines));
        }
    }
}

enum SplitResult {
    Split([Node; 2]),
    Single(Node),
}

#[derive(Deserialize, Serialize)]
struct Internal {
    orientation: Orientation,
    children: Vec<Node>,
}

#[derive(Deserialize, Serialize)]
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

    fn find_leaf<F>(&self, f: F) -> Option<&Self>
    where
        F: Fn(&Pane) -> bool,
    {
        match self {
            Self::Leaf(leaf) => {
                if f(leaf) {
                    Some(self)
                } else {
                    None
                }
            }
            Self::Internal(internal) => {
                for child in &internal.children {
                    if let Some(active) = Self::active(child) {
                        return Some(active);
                    }
                }
                None
            }
        }
    }

    fn active(&self) -> Option<&Self> {
        self.find_leaf(|leaf| leaf.is_active())
    }

    fn active_mut(&mut self) -> Option<&mut Self> {
        match self {
            Self::Leaf(leaf) => {
                if leaf.is_active() {
                    Some(self)
                } else {
                    None
                }
            }
            Self::Internal(internal) => {
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
            return SplitResult::Split([self, Self::Leaf(new_pane)]);
        }

        if let Node::Internal(mut internal) = self {
            let mut new_children: Vec<Self> = Vec::new();
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
                            new_children.push(Self::Internal(Internal {
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
            SplitResult::Single(Self::Internal(internal))
        } else {
            SplitResult::Single(self)
        }
    }
}

#[derive(Deserialize, Serialize)]
pub struct PaneTree {
    root: Node,
}

impl PaneTree {
    pub fn new(initial_buffer: &mut Buffer) -> Self {
        let initial_pane = Pane {
            id: PaneId::new(),
            buffer_id: initial_buffer.id().clone(),
            rect: Rect::default(),
            top_line: AbsLine::zero(),
            is_active: true,
            show_info_bar: true,
            is_cursor_visible: true,
        };
        initial_buffer.set_cursor(initial_pane.id(), AbsChar::default());
        Self {
            root: Node::Leaf(initial_pane),
        }
    }

    pub fn load_from_json(json: &str) -> Result<Self> {
        let mut pane_tree: Self = serde_json::from_str(json)?;
        pane_tree.cleanup_after_load();
        Ok(pane_tree)
    }

    /// Call this after deserializing in case the persisted state
    /// doesn't make sense.
    fn cleanup_after_load(&mut self) {
        // Ensure exactly one pane is active.
        let mut any_active = false;
        for pane in self.root.panes_mut() {
            if pane.is_active {
                if any_active {
                    pane.is_active = false;
                } else {
                    any_active = true;
                }
            }
        }
        if !any_active {
            // No panes active, arbitrarily pick one to make active.
            self.root.panes_mut()[0].is_active = true;
        }
    }

    pub fn recalc_layout(&mut self, width: f64, height: f64) {
        self.root.recalc_layout(Rect {
            x: 0.0,
            y: 0.0,
            width,
            height,
        });
    }

    pub fn panes(&self) -> Vec<&Pane> {
        self.root.panes()
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
            buf.set_cursor(new_pane.id(), buf.cursor(active.id()));
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

    pub fn make_previous_pane_active(&mut self) {
        let panes = self.panes();
        let index = panes
            .iter()
            .position(|pane| pane.is_active())
            .expect("no active pane");
        let prev = if index == 0 {
            panes.len() - 1
        } else {
            index - 1
        };
        let pane_id = panes[prev].id().clone();
        self.set_active(&pane_id);
    }

    pub fn make_next_pane_active(&mut self) {
        let panes = self.panes();
        let index = panes
            .iter()
            .position(|pane| pane.is_active())
            .expect("no active pane");
        let next = if index + 1 == panes.len() {
            0
        } else {
            index + 1
        };
        let pane_id = panes[next].id().clone();
        self.set_active(&pane_id);
    }

    fn set_active(&mut self, id: &PaneId) {
        for pane in self.panes_mut() {
            pane.is_active = &pane.id == id;
        }
    }
}
