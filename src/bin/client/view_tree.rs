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

    fn child_index(&self, child: NodePtr<T>) -> Option<usize> {
        if let NodeContents::Internal(internal) = &self.contents {
            internal.children.iter().position(|e| Rc::ptr_eq(e, &child))
        } else {
            None
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

    /// Split the active view.
    pub fn split(&self, orientation: gtk::Orientation) -> NodePtr<T> {
        let new_leaf = Node::new_leaf();

        let parent = self.active.borrow().parent.upgrade().unwrap();
        let mut parent = parent.borrow_mut();

        // OK to unwrap: the parent pointer of a leaf node is always
        // valid.
        let parent_internal = parent.internal_mut().unwrap();

        if parent_internal.orientation == Orientation::None {
            parent_internal.orientation = orientation.into();
            parent.insert(1, new_leaf.clone());
        } else if parent_internal.orientation == orientation {
            // Get the position of the active view in its layout so
            // that we can insert the new view right after it.
            let position = parent.child_index(self.active.clone()).unwrap();

            parent.insert(position + 1, new_leaf.clone());
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
        *tree.active.borrow_mut().leaf_mut().unwrap() = 1;
        let new_node = tree.split(gtk::Orientation::Horizontal);
        *new_node.borrow_mut().leaf_mut().unwrap() = 2;

        assert_eq!(
            tree.root,
            Node::new_internal(
                vec![Node::new_leaf_with(1), Node::new_leaf_with(2),],
                Orientation::Horizontal
            )
        );
    }
}
