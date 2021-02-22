use crate::{
    buffer::{BufferId, Embuf},
    pane::Pane,
};
use gtk4::{self as gtk, prelude::*};
use log::error;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fmt;
use std::rc::Rc;

pub trait LeafValue: Clone + fmt::Debug + PartialEq {
    /// Make a new `Self` value from the old one.
    ///
    /// For `Pane` this is a new `Pane` with the same buffer as the
    /// old one. For the `u8` tests, it's just a copy of the value.
    fn split(&self) -> Self;

    fn is_active(&self) -> bool;

    fn set_active(&mut self, active: bool);
}

impl LeafValue for Pane {
    fn split(&self) -> Self {
        let pane = Pane::new(&self.embuf());
        crate::make_big(&pane.get_widget());
        pane
    }

    fn is_active(&self) -> bool {
        self.is_active()
    }

    fn set_active(&mut self, active: bool) {
        Pane::set_active(self, active);
    }
}

#[derive(Debug, PartialEq)]
pub struct InternalNode<T: LeafValue> {
    children: Vec<NodePtr<T>>,
    orientation: gtk::Orientation,
}

struct SplitInput<T: LeafValue> {
    cur: NodePtr<T>,
    orientation: gtk::Orientation,
    active: T,
    new_leaf: NodePtr<T>,
}

impl<T: LeafValue> SplitInput<T> {
    fn clone_with_cur(&self, cur: NodePtr<T>) -> Self {
        Self {
            cur,
            orientation: self.orientation,
            active: self.active.clone(),
            new_leaf: self.new_leaf.clone(),
        }
    }
}

enum SplitResult<T: LeafValue> {
    Split([NodePtr<T>; 2]),
    Single(NodePtr<T>),
}

impl<T: LeafValue> SplitResult<T> {
    fn get_single(&self) -> Option<NodePtr<T>> {
        if let Self::Single(node) = self {
            Some(node.clone())
        } else {
            None
        }
    }
}

#[derive(Debug, PartialEq)]
pub enum Node<T: LeafValue> {
    Internal(InternalNode<T>),
    Leaf(T),
}

impl<T: LeafValue> Node<T> {
    fn new_internal(
        children: Vec<NodePtr<T>>,
        orientation: gtk::Orientation,
    ) -> NodePtr<T> {
        NodePtr::new(RefCell::new(Node::Internal(InternalNode {
            children,
            orientation,
        })))
    }

    fn new_leaf(value: T) -> NodePtr<T> {
        NodePtr::new(RefCell::new(Node::Leaf(value)))
    }

    #[allow(dead_code)]
    fn internal_mut(&mut self) -> Option<&mut InternalNode<T>> {
        match self {
            Node::Internal(ref mut internal) => Some(internal),
            _ => None,
        }
    }

    fn leaf(&self) -> Option<&T> {
        match &self {
            Node::Leaf(ref value) => Some(value),
            _ => None,
        }
    }

    fn leaf_mut(&mut self) -> Option<&mut T> {
        match self {
            Node::Leaf(ref mut value) => Some(value),
            _ => None,
        }
    }

    fn leaf_node_vec(ptr: NodePtr<T>) -> Vec<NodePtr<T>> {
        match &*ptr.borrow() {
            Node::Leaf(_) => vec![ptr.clone()],
            Node::Internal(internal) => internal
                .children
                .iter()
                .map(|n| Self::leaf_node_vec(n.clone()))
                // TODO: can use flatten for this?
                .fold(Vec::new(), |mut v1, v2| {
                    v1.extend(v2);
                    v1
                }),
        }
    }

    fn split(input: SplitInput<T>) -> SplitResult<T> {
        if input.cur.borrow().leaf() == Some(&input.active) {
            return SplitResult::Split([input.cur, input.new_leaf]);
        }

        let mut node = input.cur.borrow_mut();
        if let Node::Internal(internal) = &mut *node {
            let mut new_children: Vec<NodePtr<T>> = Vec::new();
            let mut new_orientation = internal.orientation;
            for child in &internal.children {
                match Node::split(input.clone_with_cur(child.clone())) {
                    SplitResult::Split(split_children) => {
                        if internal.children.len() == 1 {
                            // Node has only one child, so just align
                            // the orientation with the split
                            // orientation.
                            new_orientation = input.orientation;
                        }

                        if input.orientation == new_orientation {
                            // Orientation matches, so just add the
                            // new child in the appropriate place.
                            new_children.extend(split_children.iter().cloned());
                        } else {
                            // Orientation doesn't match so a new
                            // internal node is needed.
                            new_children.push(Node::new_internal(
                                split_children.to_vec(),
                                input.orientation,
                            ));
                        }
                    }
                    SplitResult::Single(child) => new_children.push(child),
                }
            }
            internal.children = new_children;
            internal.orientation = new_orientation;
        }

        SplitResult::Single(input.cur.clone())
    }
}

type NodePtr<T> = Rc<RefCell<Node<T>>>;

pub struct Tree<T: LeafValue> {
    root: NodePtr<T>,
    active: NodePtr<T>,
}

impl<T: LeafValue> Tree<T> {
    /// Create a Tree containing a single View.
    pub fn new(value: T) -> Tree<T> {
        let leaf = Node::new_leaf(value);
        let root = Node::new_internal(
            vec![leaf.clone()],
            gtk::Orientation::Horizontal,
        );
        Tree { active: leaf, root }
    }

    /// Split the active node.
    ///
    /// The new node will be created either to the right of the active
    /// node if the orientation is horizontal, or beneath the active
    /// node if the orientation is vertical. The new node will be
    /// returned.
    ///
    /// Note that this does not change the active node.
    pub fn split(&mut self, orientation: gtk::Orientation) -> NodePtr<T> {
        let new_value = self.active.borrow().leaf().unwrap().split();
        let new_leaf = Node::new_leaf(new_value);

        self.root = Node::split(SplitInput {
            cur: self.root.clone(),
            orientation,
            active: self.active.borrow().leaf().unwrap().clone(),
            new_leaf: new_leaf.clone(),
        })
        .get_single()
        .unwrap();

        new_leaf
    }

    // For debugging.
    #[allow(dead_code)]
    fn dump(&self) {
        fn r<T: LeafValue>(node: NodePtr<T>, depth: usize) {
            for _ in 0..depth {
                print!("-");
            }

            let node = node.borrow();
            match &*node {
                Node::Leaf(value) => {
                    println!("{:?}", value);
                }
                Node::Internal(internal) => {
                    println!("internal:");
                    for child in &internal.children {
                        r(child.clone(), depth + 1);
                    }
                }
            }
        }

        println!("active={:?}, tree=", self.active.borrow().leaf().unwrap());
        r(self.root.clone(), 1);
        println!();
    }

    fn find_leaf<F: Fn(&T) -> bool>(&self, f: F) -> Option<NodePtr<T>> {
        fn r<T: LeafValue, F: Fn(&T) -> bool>(
            node_ptr: NodePtr<T>,
            f: &F,
        ) -> Option<NodePtr<T>> {
            let node_ptr_clone = node_ptr.clone();
            let node = node_ptr.borrow();
            match &*node {
                Node::Leaf(value) => {
                    if f(value) {
                        Some(node_ptr_clone)
                    } else {
                        None
                    }
                }
                Node::Internal(internal) => {
                    for child in &internal.children {
                        if let Some(found) = r(child.clone(), f) {
                            return Some(found);
                        }
                    }
                    None
                }
            }
        };

        r(self.root.clone(), &f)
    }

    pub fn leaf_vec(&self) -> Vec<T> {
        Node::leaf_node_vec(self.root.clone())
            .iter()
            .map(|n| n.borrow().leaf().unwrap().clone())
            .collect()
    }
}

const PANE_TREE_LAYOUT_TAG: &str = "pane_tree_layout_widget";

impl Node<Pane> {
    pub fn render(&self) -> gtk::Widget {
        match &self {
            Node::Internal(internal) => {
                let spacing = 1;
                let layout = gtk::Box::new(internal.orientation, spacing);
                crate::make_big(&layout);

                // Tag the layout so that we know which widgets are
                // part of the pane tree layout in
                // `recursive_unparent`.
                layout.set_widget_name(PANE_TREE_LAYOUT_TAG);

                for child in &internal.children {
                    let child_widget = child.borrow().render();
                    crate::make_big(&child_widget);
                    layout.append(&child_widget);
                }
                layout.upcast()
            }
            Node::Leaf(view) => view.get_widget(),
        }
    }

    fn serialize(&self, active_pane: &Pane) -> PaneTreeSerdeNode {
        match &self {
            Node::Leaf(pane) => PaneTreeSerdeNode::Leaf {
                active: pane == active_pane,
                buffer: pane.embuf().buffer_id(),
            },
            Node::Internal(internal) => PaneTreeSerdeNode::Internal((
                OrientationSerde::from_gtk(internal.orientation),
                internal
                    .children
                    .iter()
                    .map(|n| n.borrow().serialize(active_pane))
                    .collect(),
            )),
        }
    }

    fn deserialize(
        root: &PaneTreeSerdeNode,
        embufs: &[Embuf],
        proto: &Pane,
    ) -> NodePtr<Pane> {
        match root {
            PaneTreeSerdeNode::Leaf { active, buffer } => {
                let pane = proto.split();
                // TODO: we need to actually restore buffer ids
                if let Some(embuf) =
                    embufs.iter().find(|embuf| &embuf.buffer_id() == buffer)
                {
                    pane.set_buffer(embuf);
                }
                if *active {
                    pane.set_active(true);
                }
                Node::new_leaf(pane)
            }
            PaneTreeSerdeNode::Internal((orientation, children)) => {
                let node = Node::new_internal(
                    children
                        .iter()
                        .map(|c| Node::deserialize(c, embufs, proto))
                        .collect(),
                    orientation.to_gtk(),
                );
                node
            }
        }
    }
}

impl Tree<Pane> {
    pub fn render(&self) -> gtk::Widget {
        self.root.borrow().render()
    }

    pub fn active(&self) -> Pane {
        self.active.borrow_mut().leaf_mut().unwrap().clone()
    }

    pub fn set_active(&mut self, pane: &Pane) {
        if let Some(node) = self.find_leaf(|value| value == pane) {
            self.active = node;
        } else {
            // Should never happen: this pane is not in the tree.
            error!("failed to set active pane");
        }
    }

    pub fn serialize(&self) -> PaneTreeSerdeNode {
        self.root.borrow().serialize(&self.active())
    }

    pub fn deserialize(&mut self, root: &PaneTreeSerdeNode, embufs: &[Embuf]) {
        self.root = Node::deserialize(root, embufs, &self.active());

        fn find_active(node: NodePtr<Pane>) -> Option<NodePtr<Pane>> {
            let node_clone = node.clone();
            match &*node.borrow() {
                Node::Leaf(pane) => {
                    if pane.is_active() {
                        Some(node_clone)
                    } else {
                        None
                    }
                }
                Node::Internal(internal) => {
                    for child in &internal.children {
                        if let Some(node) = find_active(child.clone()) {
                            return Some(node);
                        }
                    }
                    None
                }
            }
        }
        self.active = find_active(self.root.clone()).unwrap();
    }
}

pub type PaneTree = Tree<Pane>;

pub fn recursive_unparent_children<W: IsA<gtk::Widget>>(root: &W) {
    fn r<W: IsA<gtk::Widget>>(root: &W, check_name: bool) {
        // Avoid recursing down into the Pane widgets, we only want to
        // unparent from the Box widgets created during `render`.
        if check_name && root.get_widget_name() != PANE_TREE_LAYOUT_TAG {
            return;
        }

        // Unparent children.
        while let Some(child) = root.get_first_child() {
            let check_name = true;
            r(&child, check_name);

            child.unparent();
        }
    }

    // Don't check the name of the initial widget since it's the split
    // root container, outside of the pane tree.
    let check_name = false;
    r(root, check_name);
}

#[derive(Debug, Deserialize, Serialize)]
pub enum OrientationSerde {
    Horizontal,
    Vertical,
}

impl OrientationSerde {
    fn from_gtk(o: gtk::Orientation) -> OrientationSerde {
        match o {
            gtk::Orientation::Horizontal => OrientationSerde::Horizontal,
            gtk::Orientation::Vertical => OrientationSerde::Vertical,
            _ => panic!("invalid orientation: {}", o),
        }
    }

    fn to_gtk(&self) -> gtk::Orientation {
        match self {
            OrientationSerde::Horizontal => gtk::Orientation::Horizontal,
            OrientationSerde::Vertical => gtk::Orientation::Vertical,
        }
    }
}

#[derive(Debug, Deserialize, Serialize)]
pub enum PaneTreeSerdeNode {
    Leaf { active: bool, buffer: BufferId },
    Internal((OrientationSerde, Vec<PaneTreeSerdeNode>)),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Debug, PartialEq)]
    struct TestPane {
        value: u8,
        active: bool,
    }

    impl TestPane {
        fn new(value: u8) -> TestPane {
            TestPane {
                value,
                active: false,
            }
        }
    }

    impl LeafValue for TestPane {
        fn split(&self) -> Self {
            Self {
                value: self.value,
                active: false,
            }
        }

        fn is_active(&self) -> bool {
            self.active
        }

        fn set_active(&mut self, active: bool) {
            self.active = active;
        }
    }

    #[test]
    fn test_tree() {
        let mut tree: Tree<TestPane> = Tree::new(TestPane::new(1));

        // Horizontally split a node whose parent has no orientation.
        let new_node = tree.split(gtk::Orientation::Horizontal);
        *new_node.borrow_mut().leaf_mut().unwrap() = TestPane::new(2);
        assert_eq!(
            tree.root,
            Node::new_internal(
                vec![
                    Node::new_leaf(TestPane::new(1)),
                    Node::new_leaf(TestPane::new(2))
                ],
                gtk::Orientation::Horizontal
            )
        );

        // Horizontally split a node whose parent's orientation is
        // already horizontal. The "1" node is still active, so the
        // new horizontal layout should be [1, 3, 2].
        let new_node = tree.split(gtk::Orientation::Horizontal);
        *new_node.borrow_mut().leaf_mut().unwrap() = TestPane::new(3);
        assert_eq!(
            tree.root,
            Node::new_internal(
                vec![
                    Node::new_leaf(TestPane::new(1)),
                    Node::new_leaf(TestPane::new(3)),
                    Node::new_leaf(TestPane::new(2))
                ],
                gtk::Orientation::Horizontal
            )
        );

        // Vertically split a node whose parent's orientation is
        // horizontal. The "1" node is still active, so the new
        // horizontal layout should be [X, 3, 2], where X is a
        // vertical layout containing [1, 4].
        let new_node = tree.split(gtk::Orientation::Vertical);
        *new_node.borrow_mut().leaf_mut().unwrap() = TestPane::new(4);
        assert_eq!(
            tree.root,
            Node::new_internal(
                vec![
                    Node::new_internal(
                        vec![
                            Node::new_leaf(TestPane::new(1)),
                            Node::new_leaf(TestPane::new(4))
                        ],
                        gtk::Orientation::Vertical
                    ),
                    Node::new_leaf(TestPane::new(3)),
                    Node::new_leaf(TestPane::new(2))
                ],
                gtk::Orientation::Horizontal
            )
        );

        // Split vertically again. The "1" node is still active, so
        // the horizontal layout should still be [X, 3, 2] where X is
        // a vertical layout now containing [1, 5, 4].
        let new_node = tree.split(gtk::Orientation::Vertical);
        *new_node.borrow_mut().leaf_mut().unwrap() = TestPane::new(5);
        assert_eq!(
            tree.root,
            Node::new_internal(
                vec![
                    Node::new_internal(
                        vec![
                            Node::new_leaf(TestPane::new(1)),
                            Node::new_leaf(TestPane::new(5)),
                            Node::new_leaf(TestPane::new(4))
                        ],
                        gtk::Orientation::Vertical
                    ),
                    Node::new_leaf(TestPane::new(3)),
                    Node::new_leaf(TestPane::new(2))
                ],
                gtk::Orientation::Horizontal
            )
        );
    }
}
