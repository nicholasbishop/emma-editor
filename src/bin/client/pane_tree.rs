use crate::{
    buffer::{BufferId, Embuf},
    pane::Pane,
};
use gtk4::{self as gtk, prelude::*};
use log::error;
use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Deserialize, Serialize)]
pub enum Orientation {
    None,
    Horizontal,
    Vertical,
}

impl From<gtk::Orientation> for Orientation {
    fn from(o: gtk::Orientation) -> Self {
        match o {
            gtk::Orientation::Horizontal => Orientation::Horizontal,
            gtk::Orientation::Vertical => Orientation::Vertical,
            _ => panic!("invalid orientation: {}", o),
        }
    }
}

impl PartialEq<gtk::Orientation> for Orientation {
    fn eq(&self, other: &gtk::Orientation) -> bool {
        match self {
            Self::None => false,
            Self::Horizontal => *other == gtk::Orientation::Horizontal,
            Self::Vertical => *other == gtk::Orientation::Vertical,
        }
    }
}

pub trait Splitable {
    /// Make a new `Self` value from the old one.
    ///
    /// For `Pane` this is a new `Pane` with the same buffer as the
    /// old one. For the `u8` tests, it's just a copy of the value.
    fn split(&self) -> Self;
}

pub trait LeafValue: Clone + fmt::Debug + PartialEq + Splitable {}

impl Splitable for Pane {
    fn split(&self) -> Self {
        let pane = Pane::new(&self.embuf());
        crate::make_big(&pane.get_widget());
        pane
    }
}

impl LeafValue for Pane {}

#[derive(Debug, PartialEq)]
struct InternalNode<T: LeafValue> {
    children: Vec<NodePtr<T>>,
    orientation: Orientation,
}

impl<T: LeafValue> InternalNode<T> {
    fn child_index(&self, child: NodePtr<T>) -> Option<usize> {
        self.children.iter().position(|e| Rc::ptr_eq(e, &child))
    }
}

#[derive(Debug, PartialEq)]
enum NodeContents<T: LeafValue> {
    Internal(InternalNode<T>),
    Leaf(T),
}

#[derive(Debug)]
pub struct Node<T: LeafValue> {
    contents: NodeContents<T>,
    parent: NodeWeakPtr<T>,
}

impl<T: LeafValue> PartialEq for Node<T> {
    fn eq(&self, other: &Node<T>) -> bool {
        // Ignore parent pointer
        self.contents == other.contents
    }
}

impl<T: LeafValue> Node<T> {
    fn new_internal(
        children: Vec<NodePtr<T>>,
        orientation: Orientation,
    ) -> NodePtr<T> {
        NodePtr::new(RefCell::new(Node {
            contents: NodeContents::Internal(InternalNode {
                children,
                orientation,
            }),
            parent: NodeWeakPtr::new(),
        }))
    }

    fn new_leaf(value: T) -> NodePtr<T> {
        NodePtr::new(RefCell::new(Node {
            contents: NodeContents::Leaf(value),
            parent: NodeWeakPtr::new(),
        }))
    }

    fn internal(&self) -> Option<&InternalNode<T>> {
        match &self.contents {
            NodeContents::Internal(ref internal) => Some(internal),
            _ => None,
        }
    }

    fn internal_mut(&mut self) -> Option<&mut InternalNode<T>> {
        match &mut self.contents {
            NodeContents::Internal(ref mut internal) => Some(internal),
            _ => None,
        }
    }

    fn leaf(&self) -> Option<&T> {
        match &self.contents {
            NodeContents::Leaf(ref value) => Some(value),
            _ => None,
        }
    }

    fn leaf_mut(&mut self) -> Option<&mut T> {
        match &mut self.contents {
            NodeContents::Leaf(ref mut value) => Some(value),
            _ => None,
        }
    }

    fn insert(&mut self, index: usize, child: NodePtr<T>) {
        // TODO: fewer unwraps?
        self.internal_mut().unwrap().children.insert(index, child);
    }

    fn leaf_vec(&self) -> Vec<T> {
        match &self.contents {
            NodeContents::Leaf(value) => vec![value.clone()],
            NodeContents::Internal(internal) => internal
                .children
                .iter()
                .map(|n| n.borrow().leaf_vec())
                .fold(Vec::new(), |mut v1, v2| {
                    v1.extend(v2);
                    v1
                }),
        }
    }
}

type NodePtr<T> = Rc<RefCell<Node<T>>>;
type NodeWeakPtr<T> = Weak<RefCell<Node<T>>>;

pub struct Tree<T: LeafValue> {
    root: NodePtr<T>,
    active: NodePtr<T>,
}

impl<T: LeafValue> Tree<T> {
    /// Create a Tree containing a single View.
    pub fn new(value: T) -> Tree<T> {
        let leaf = Node::new_leaf(value);
        let root = Node::new_internal(vec![leaf.clone()], Orientation::None);
        leaf.borrow_mut().parent = Rc::downgrade(&root);
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
    pub fn split(&self, orientation: gtk::Orientation) -> NodePtr<T> {
        // OK to unwrap, active node is always a leaf.
        let new_value = self.active.borrow().leaf().unwrap().split();

        let new_leaf = Node::new_leaf(new_value);

        // OK to unwrap: the parent pointer of a leaf node is always
        // valid and is always an internal node.
        let parent_ptr = self.active.borrow().parent.upgrade().unwrap();
        let mut parent = parent_ptr.borrow_mut();
        let parent_internal = parent.internal_mut().unwrap();

        // Get the position of the active node in its parent. Ok to
        // unwrap, a child is always in its parent.
        let position =
            parent_internal.child_index(self.active.clone()).unwrap();

        // If the parent doesn't have an orientation yet (i.e. it has
        // only one child), just set the correct orientation.
        if parent_internal.orientation == Orientation::None {
            parent_internal.orientation = orientation.into();
        }

        if parent_internal.orientation == orientation {
            // The orientation already matches, so just insert the new
            // node right after the active one.
            parent.insert(position + 1, new_leaf.clone());
            new_leaf.borrow_mut().parent = Rc::downgrade(&parent_ptr);
        } else {
            // Create a new internal node with the correct
            // orientation. The children are the active node and the
            // new node.
            let new_internal = Node::new_internal(
                vec![self.active.clone(), new_leaf.clone()],
                orientation.into(),
            );
            self.active.borrow_mut().parent = Rc::downgrade(&new_internal);
            new_leaf.borrow_mut().parent = Rc::downgrade(&new_internal);

            // In the parent, replace the active leaf with the new
            // internal node.
            parent_internal.children[position] = new_internal.clone();
            new_internal.borrow_mut().parent = Rc::downgrade(&parent_ptr);
        }

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
            match &node.contents {
                NodeContents::Leaf(value) => {
                    println!("{:?}", value);
                }
                NodeContents::Internal(internal) => {
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
            match &node.contents {
                NodeContents::Leaf(value) => {
                    if f(value) {
                        Some(node_ptr_clone)
                    } else {
                        None
                    }
                }
                NodeContents::Internal(internal) => {
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
        self.root.borrow().leaf_vec()
    }
}

const PANE_TREE_LAYOUT_TAG: &str = "pane_tree_layout_widget";

impl Node<Pane> {
    pub fn render(&self) -> gtk::Widget {
        match &self.contents {
            NodeContents::Internal(internal) => {
                let orientation = match internal.orientation {
                    Orientation::None => {
                        // Doesn't matter, arbitrarily pick horizontal.
                        gtk::Orientation::Horizontal
                    }
                    Orientation::Horizontal => gtk::Orientation::Horizontal,
                    Orientation::Vertical => gtk::Orientation::Vertical,
                };
                let spacing = 1;
                let layout = gtk::Box::new(orientation, spacing);
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
            NodeContents::Leaf(view) => view.get_widget(),
        }
    }

    fn serialize(&self, active_pane: &Pane) -> PaneTreeSerdeNode {
        match &self.contents {
            NodeContents::Leaf(pane) => PaneTreeSerdeNode::Leaf {
                active: pane == active_pane,
                buffer: pane.embuf().buffer_id(),
            },
            NodeContents::Internal(internal) => PaneTreeSerdeNode::Internal((
                internal.orientation,
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
                    *orientation,
                );
                for child in &node.borrow().internal().unwrap().children {
                    child.borrow_mut().parent = Rc::downgrade(&node);
                }
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
            match &node.borrow().contents {
                NodeContents::Leaf(pane) => {
                    if pane.is_active() {
                        Some(node_clone)
                    } else {
                        None
                    }
                }
                NodeContents::Internal(internal) => {
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
pub enum PaneTreeSerdeNode {
    Leaf { active: bool, buffer: BufferId },
    Internal((Orientation, Vec<PaneTreeSerdeNode>)),
}

#[cfg(test)]
mod tests {
    use super::*;

    impl Splitable for u8 {
        fn split(&self) -> Self {
            *self
        }
    }

    impl LeafValue for u8 {}

    #[test]
    fn test_tree() {
        let tree: Tree<u8> = Tree::new(1);

        // Horizontally split a node whose parent has no orientation.
        let new_node = tree.split(gtk::Orientation::Horizontal);
        *new_node.borrow_mut().leaf_mut().unwrap() = 2;
        assert_eq!(
            tree.root,
            Node::new_internal(
                vec![Node::new_leaf(1), Node::new_leaf(2)],
                Orientation::Horizontal
            )
        );

        // Horizontally split a node whose parent's orientation is
        // already horizontal. The "1" node is still active, so the
        // new horizontal layout should be [1, 3, 2].
        let new_node = tree.split(gtk::Orientation::Horizontal);
        *new_node.borrow_mut().leaf_mut().unwrap() = 3;
        assert_eq!(
            tree.root,
            Node::new_internal(
                vec![Node::new_leaf(1), Node::new_leaf(3), Node::new_leaf(2)],
                Orientation::Horizontal
            )
        );

        // Vertically split a node whose parent's orientation is
        // horizontal. The "1" node is still active, so the new
        // horizontal layout should be [X, 3, 2], where X is a
        // vertical layout containing [1, 4].
        let new_node = tree.split(gtk::Orientation::Vertical);
        *new_node.borrow_mut().leaf_mut().unwrap() = 4;
        assert_eq!(
            tree.root,
            Node::new_internal(
                vec![
                    Node::new_internal(
                        vec![Node::new_leaf(1), Node::new_leaf(4)],
                        Orientation::Vertical
                    ),
                    Node::new_leaf(3),
                    Node::new_leaf(2)
                ],
                Orientation::Horizontal
            )
        );

        // Split vertically again. The "1" node is still active, so
        // the horizontal layout should still be [X, 3, 2] where X is
        // a vertical layout now containing [1, 5, 4].
        let new_node = tree.split(gtk::Orientation::Vertical);
        *new_node.borrow_mut().leaf_mut().unwrap() = 5;
        assert_eq!(
            tree.root,
            Node::new_internal(
                vec![
                    Node::new_internal(
                        vec![
                            Node::new_leaf(1),
                            Node::new_leaf(5),
                            Node::new_leaf(4)
                        ],
                        Orientation::Vertical
                    ),
                    Node::new_leaf(3),
                    Node::new_leaf(2)
                ],
                Orientation::Horizontal
            )
        );

        // TODO: consider testing parent pointers in the above since
        // we've gotten that wrong before.
    }
}
