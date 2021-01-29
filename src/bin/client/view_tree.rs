// TODO
#![allow(dead_code)]

use std::cell::RefCell;
use std::rc::{Rc, Weak};

// TODO: eventually this will be more than just a text view
#[derive(Default)]
pub struct View(gtk::TextView);

enum Orientation {
    None,
    Horizontal,
    Vertical,
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

struct InternalNode<T> {
    children: Vec<NodePtr<T>>,
    orientation: Orientation,
}

enum NodeContents<T> {
    Internal(InternalNode<T>),
    Leaf(T),
}

struct Node<T> {
    contents: NodeContents<T>,
    parent: NodeWeakPtr<T>,
}

impl<T: Default> Node<T> {
    fn new_leaf() -> NodePtr<T> {
        NodePtr::new(RefCell::new(Node {
            contents: NodeContents::Leaf(T::default()),
            parent: NodeWeakPtr::new(),
        }))
    }

    fn internal(&self) -> Option<&InternalNode<T>> {
        match &self.contents {
            NodeContents::Internal(internal) => Some(internal),
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

    fn insert(&self, _index: usize, _child: NodePtr<T>) {
        todo!();
    }
}

type NodePtr<T> = Rc<RefCell<Node<T>>>;
type NodeWeakPtr<T> = Weak<RefCell<Node<T>>>;

pub struct Tree<T> {
    root: NodePtr<T>,
    active: NodePtr<T>,
}

impl<T: Default> Tree<T> {
    /// Create a ViewTree containing a single View.
    pub fn new() -> Tree<T> {
        let leaf = Node::new_leaf();
        let root = NodePtr::new(RefCell::new(Node {
            contents: NodeContents::Internal(InternalNode {
                children: vec![leaf.clone()],
                orientation: Orientation::None,
            }),
            parent: NodeWeakPtr::new(),
        }));
        leaf.borrow_mut().parent = Rc::downgrade(&root);
        Tree { active: leaf, root }
    }

    /// Split the active view.
    pub fn split(&self, orientation: gtk::Orientation) {
        let new_leaf = Node::new_leaf();

        let parent = self.active.borrow().parent.upgrade().unwrap();
        let parent = parent.borrow_mut();

        if parent.internal().unwrap().orientation == orientation {
            // Get the position of the active view in its layout so
            // that we can insert the new view right after it.
            let position = parent.child_index(self.active.clone()).unwrap();

            parent.insert(position + 1, new_leaf);
        }
    }
}

pub type ViewTree = Tree<View>;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tree() {
        let tree: Tree<u8> = Tree::new();
        *tree.active.borrow_mut().leaf_mut().unwrap() = 1;
        tree.split(gtk::Orientation::Horizontal);
    }
}
