// TODO
#![allow(dead_code)]

use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

// TODO: eventually this will be more than just a text view
#[derive(Debug, Default, PartialEq)]
pub struct View(gtk::TextView);

#[derive(Debug, Eq, PartialEq)]
enum Orientation {
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

pub trait LeafValue: fmt::Debug + Default + PartialEq {}

impl LeafValue for View {}

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

    fn new_leaf() -> NodePtr<T> {
        Self::new_leaf_with(T::default())
    }

    fn new_leaf_with(value: T) -> NodePtr<T> {
        NodePtr::new(RefCell::new(Node {
            contents: NodeContents::Leaf(value),
            parent: NodeWeakPtr::new(),
        }))
    }

    fn internal(&self) -> Option<&InternalNode<T>> {
        match &self.contents {
            NodeContents::Internal(internal) => Some(internal),
            _ => None,
        }
    }

    fn internal_mut(&mut self) -> Option<&mut InternalNode<T>> {
        match &mut self.contents {
            NodeContents::Internal(ref mut internal) => Some(internal),
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
}

type NodePtr<T> = Rc<RefCell<Node<T>>>;
type NodeWeakPtr<T> = Weak<RefCell<Node<T>>>;

pub struct Tree<T: LeafValue> {
    root: NodePtr<T>,
    active: NodePtr<T>,
}

impl<T: LeafValue> Tree<T> {
    /// Create a ViewTree containing a single View.
    pub fn new() -> Tree<T> {
        let leaf = Node::new_leaf();
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
        let new_leaf = Node::new_leaf();

        let parent = self.active.borrow().parent.upgrade().unwrap();
        let mut parent = parent.borrow_mut();

        // OK to unwrap: the parent pointer of a leaf node is always
        // valid.
        let parent_internal = parent.internal_mut().unwrap();

        // Get the position of the active node in its parent.
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
        } else {
            // Create a new internal node with the correct
            // orientation. The children are the active node and the
            // new node.
            let new_internal = Node::new_internal(
                vec![self.active.clone(), new_leaf.clone()],
                orientation.into(),
            );

            // In the parent, replace the active leaf with the new
            // internal node.
            parent_internal.children[position] = new_internal;
        }

        new_leaf
    }
}

pub type ViewTree = Tree<View>;

#[cfg(test)]
mod tests {
    use super::*;

    impl LeafValue for u8 {}

    #[test]
    fn test_tree() {
        let tree: Tree<u8> = Tree::new();

        // Horizontally split a node whose parent has no orientation.
        *tree.active.borrow_mut().leaf_mut().unwrap() = 1;
        let new_node = tree.split(gtk::Orientation::Horizontal);
        *new_node.borrow_mut().leaf_mut().unwrap() = 2;
        assert_eq!(
            tree.root,
            Node::new_internal(
                vec![Node::new_leaf_with(1), Node::new_leaf_with(2)],
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
                vec![
                    Node::new_leaf_with(1),
                    Node::new_leaf_with(3),
                    Node::new_leaf_with(2)
                ],
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
                        vec![Node::new_leaf_with(1), Node::new_leaf_with(4)],
                        Orientation::Vertical
                    ),
                    Node::new_leaf_with(3),
                    Node::new_leaf_with(2)
                ],
                Orientation::Horizontal
            )
        );
    }
}
