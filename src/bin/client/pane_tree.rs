use crate::{
    buffer::{BufferId, Embuf},
    pane::Pane,
};
use gtk4::{self as gtk, prelude::*};
use serde::{Deserialize, Serialize};
use std::fmt;

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
    children: Vec<Node<T>>,
    orientation: gtk::Orientation,
}

#[derive(Clone)]
struct SplitInput<T: LeafValue> {
    orientation: gtk::Orientation,
    active: T,
    new_leaf: T,
}

enum SplitResult<T: LeafValue> {
    Split([Node<T>; 2]),
    Single(Node<T>),
}

#[derive(Debug, PartialEq)]
pub enum Node<T: LeafValue> {
    Internal(InternalNode<T>),
    Leaf(T),
}

impl<T: LeafValue> Node<T> {
    fn new_internal(
        children: Vec<Node<T>>,
        orientation: gtk::Orientation,
    ) -> Node<T> {
        Node::Internal(InternalNode {
            children,
            orientation,
        })
    }

    fn new_leaf(value: T) -> Node<T> {
        Node::Leaf(value)
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

    #[allow(dead_code)]
    fn leaf_mut(&mut self) -> Option<&mut T> {
        match self {
            Node::Leaf(ref mut value) => Some(value),
            _ => None,
        }
    }

    fn leaf_vec(&self) -> Vec<T> {
        match self {
            Node::Leaf(leaf) => vec![leaf.clone()],
            Node::Internal(internal) => internal
                .children
                .iter()
                .map(|n| n.leaf_vec())
                // TODO: can use flatten for this?
                .fold(Vec::new(), |mut v1, v2| {
                    v1.extend(v2);
                    v1
                }),
        }
    }

    fn split(self, input: SplitInput<T>) -> SplitResult<T> {
        if self.leaf() == Some(&input.active) {
            return SplitResult::Split([self, Node::new_leaf(input.new_leaf)]);
        }

        if let Node::Internal(mut internal) = self {
            let mut new_children: Vec<Node<T>> = Vec::new();
            let mut new_orientation = internal.orientation;
            let num_children = internal.children.len();
            for child in internal.children {
                match child.split(input.clone()) {
                    SplitResult::Split([child1, child2]) => {
                        if num_children == 1 {
                            // Node has only one child, so just align
                            // the orientation with the split
                            // orientation.
                            new_orientation = input.orientation;
                        }

                        if input.orientation == new_orientation {
                            // Orientation matches, so just add the
                            // new child in the appropriate place.
                            new_children.push(child1);
                            new_children.push(child2);
                        } else {
                            // Orientation doesn't match so a new
                            // internal node is needed.
                            new_children.push(Node::new_internal(
                                vec![child1, child2],
                                input.orientation,
                            ));
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

    fn get_active(&self) -> Option<T> {
        match self {
            Node::Leaf(leaf) => {
                if leaf.is_active() {
                    Some(leaf.clone())
                } else {
                    None
                }
            }
            Node::Internal(internal) => {
                for child in &internal.children {
                    if let Some(active) = Self::get_active(child) {
                        return Some(active);
                    }
                }
                None
            }
        }
    }
}

pub struct Tree<T: LeafValue> {
    root: Node<T>,
}

impl<T: LeafValue> Tree<T> {
    /// Create a Tree containing a single leaf.
    pub fn new(mut value: T) -> Tree<T> {
        value.set_active(true);
        Tree {
            root: Node::new_leaf(value),
        }
    }

    pub fn active(&self) -> T {
        self.root
            .get_active()
            // OK to unwrap here because there is always exactly one
            // active leaf.
            .unwrap()
    }

    pub fn set_active(&mut self, active: T) {
        let leaves = self.leaf_vec();
        for mut leaf in leaves {
            leaf.set_active(leaf == active);
        }
    }

    fn take_root(&mut self) -> Node<T> {
        // TODO: this seems silly, creating a temporary unused node
        // just so I can move out of self.root, not sure how to avoid
        // thought.
        std::mem::replace(
            &mut self.root,
            Node::new_internal(Vec::new(), gtk::Orientation::Horizontal),
        )
    }

    /// Split the active node.
    ///
    /// The new node will be created either to the right of the active
    /// node if the orientation is horizontal, or beneath the active
    /// node if the orientation is vertical. The new node will be
    /// returned.
    ///
    /// Note that this does not change the active node.
    pub fn split(&mut self, orientation: gtk::Orientation) -> T {
        let active = self.active();
        let new_value = active.split();

        let root = self.take_root();
        let sr = root.split(SplitInput {
            orientation,
            active,
            new_leaf: new_value.clone(),
        });
        self.root = match sr {
            SplitResult::Single(single) => single,
            SplitResult::Split([child1, child2]) => {
                Node::new_internal(vec![child1, child2], orientation)
            }
        };

        new_value
    }

    // For debugging.
    #[allow(dead_code)]
    fn dump(&self) {
        fn r<T: LeafValue>(node: &Node<T>, depth: usize) {
            for _ in 0..depth {
                print!("-");
            }

            match node {
                Node::Leaf(value) => {
                    println!("{:?}", value);
                }
                Node::Internal(internal) => {
                    println!("internal:");
                    for child in &internal.children {
                        r(child, depth + 1);
                    }
                }
            }
        }

        println!("active={:?}, tree=", self.active());
        r(&self.root, 1);
        println!();
    }

    pub fn leaf_vec(&self) -> Vec<T> {
        self.root.leaf_vec()
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
                    let child_widget = child.render();
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
                    .map(|n| n.serialize(active_pane))
                    .collect(),
            )),
        }
    }

    fn deserialize(
        root: &PaneTreeSerdeNode,
        embufs: &[Embuf],
        proto: &Pane,
    ) -> Node<Pane> {
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
        self.root.render()
    }

    pub fn serialize(&self) -> PaneTreeSerdeNode {
        self.root.serialize(&self.active())
    }

    pub fn deserialize(&mut self, root: &PaneTreeSerdeNode, embufs: &[Embuf]) {
        self.root = Node::deserialize(root, embufs, &self.active());
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
    use std::cell::RefCell;
    use std::rc::Rc;

    #[derive(Clone, Debug, PartialEq)]
    struct TestPaneData {
        value: u8,
        active: bool,
    }

    type TestPane = Rc<RefCell<TestPaneData>>;

    fn mkinactive(value: u8) -> TestPane {
        Rc::new(RefCell::new(TestPaneData {
            value,
            active: false,
        }))
    }

    fn mkactive(value: u8) -> TestPane {
        Rc::new(RefCell::new(TestPaneData {
            value,
            active: true,
        }))
    }

    impl LeafValue for TestPane {
        fn split(&self) -> Self {
            mkinactive(self.borrow().value)
        }

        fn is_active(&self) -> bool {
            self.borrow().active
        }

        fn set_active(&mut self, active: bool) {
            self.borrow_mut().active = active;
        }
    }

    #[test]
    fn test_tree() {
        let mut tree: Tree<TestPane> = Tree::new(mkinactive(1));

        // Horizontally split a node whose parent has no orientation.
        let new_leaf = tree.split(gtk::Orientation::Horizontal);
        new_leaf.borrow_mut().value = 2;
        assert_eq!(
            tree.root,
            Node::new_internal(
                vec![
                    Node::new_leaf(mkactive(1)),
                    Node::new_leaf(mkinactive(2))
                ],
                gtk::Orientation::Horizontal
            )
        );

        // Horizontally split a node whose parent's orientation is
        // already horizontal. The "1" node is still active, so the
        // new horizontal layout should be [1, 3, 2].
        let new_leaf = tree.split(gtk::Orientation::Horizontal);
        new_leaf.borrow_mut().value = 3;
        assert_eq!(
            tree.root,
            Node::new_internal(
                vec![
                    Node::new_leaf(mkactive(1)),
                    Node::new_leaf(mkinactive(3)),
                    Node::new_leaf(mkinactive(2))
                ],
                gtk::Orientation::Horizontal
            )
        );

        // Vertically split a node whose parent's orientation is
        // horizontal. The "1" node is still active, so the new
        // horizontal layout should be [X, 3, 2], where X is a
        // vertical layout containing [1, 4].
        let new_leaf = tree.split(gtk::Orientation::Vertical);
        new_leaf.borrow_mut().value = 4;
        assert_eq!(
            tree.root,
            Node::new_internal(
                vec![
                    Node::new_internal(
                        vec![
                            Node::new_leaf(mkactive(1)),
                            Node::new_leaf(mkinactive(4))
                        ],
                        gtk::Orientation::Vertical
                    ),
                    Node::new_leaf(mkinactive(3)),
                    Node::new_leaf(mkinactive(2))
                ],
                gtk::Orientation::Horizontal
            )
        );

        // Split vertically again. The "1" node is still active, so
        // the horizontal layout should still be [X, 3, 2] where X is
        // a vertical layout now containing [1, 5, 4].
        let new_leaf = tree.split(gtk::Orientation::Vertical);
        new_leaf.borrow_mut().value = 5;
        assert_eq!(
            tree.root,
            Node::new_internal(
                vec![
                    Node::new_internal(
                        vec![
                            Node::new_leaf(mkactive(1)),
                            Node::new_leaf(mkinactive(5)),
                            Node::new_leaf(mkinactive(4))
                        ],
                        gtk::Orientation::Vertical
                    ),
                    Node::new_leaf(mkinactive(3)),
                    Node::new_leaf(mkinactive(2))
                ],
                gtk::Orientation::Horizontal
            )
        );
    }
}
