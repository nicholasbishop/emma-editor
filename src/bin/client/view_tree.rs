use glib::Cast;
use gtk::BoxExt;
use std::cell::RefCell;
use std::fmt;
use std::rc::{Rc, Weak};

// TODO: eventually this will be more than just a text view
#[derive(Debug, Default, PartialEq)]
pub struct View(gtk::TextView);

pub trait LeafValue: fmt::Debug + Default + PartialEq {}

impl LeafValue for View {}

#[derive(Debug, PartialEq)]
struct InternalNode<T: LeafValue> {
    children: Vec<NodePtr<T>>,
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
    ) -> NodePtr<T> {
        NodePtr::new(RefCell::new(Node {
            contents: NodeContents::Internal(InternalNode {
                children,
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
}

type NodePtr<T> = Rc<RefCell<Node<T>>>;
type NodeWeakPtr<T> = Weak<RefCell<Node<T>>>;

pub struct Tree<T: LeafValue> {
    root: NodePtr<T>,
}

impl<T: LeafValue> Tree<T> {
    /// Create a ViewTree containing a single View.
    pub fn new() -> Tree<T> {
        let leaf = Node::new_leaf();
        let root = Node::new_internal(vec![leaf.clone()]);
        leaf.borrow_mut().parent = Rc::downgrade(&root);
        Tree { root }
    }
}

impl Node<View> {
    pub fn render(&self) -> gtk::Widget {
        match &self.contents {
            NodeContents::Internal(internal) => {
                let orientation = gtk::Orientation::Horizontal;
                let spacing = 1;
                let layout = gtk::Box::new(orientation, spacing);
                for child in &internal.children {
                    let expand = true;
                    let fill = true;
                    let padding = 0;
                    layout.pack_start(
                        &child.borrow().render(),
                        expand,
                        fill,
                        padding,
                    );
                }
                layout.upcast()
            }
            NodeContents::Leaf(view) => view.0.clone().upcast(),
        }
    }
}

impl Tree<View> {
    pub fn render(&self) -> gtk::Widget {
        self.root.borrow().render()
    }
}

pub type ViewTree = Tree<View>;
