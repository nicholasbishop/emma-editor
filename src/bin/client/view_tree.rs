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

struct InternalNode {
    children: Vec<NodePtr>,
}

enum NodeContents {
    Internal(InternalNode),
    Leaf(View),
}

pub struct Node {
    contents: NodeContents,
    parent: NodeWeakPtr,
}

impl Node {
    fn new_internal(
        children: Vec<NodePtr>,
    ) -> NodePtr {
        NodePtr::new(RefCell::new(Node {
            contents: NodeContents::Internal(InternalNode {
                children,
            }),
            parent: NodeWeakPtr::new(),
        }))
    }

    fn new_leaf() -> NodePtr {
        Self::new_leaf_with(View::default())
    }

    fn new_leaf_with(value: View) -> NodePtr {
        NodePtr::new(RefCell::new(Node {
            contents: NodeContents::Leaf(value),
            parent: NodeWeakPtr::new(),
        }))
    }
}

type NodePtr = Rc<RefCell<Node>>;
type NodeWeakPtr = Weak<RefCell<Node>>;

pub struct Tree {
    root: NodePtr,
}

impl Tree {
    /// Create a ViewTree containing a single View.
    pub fn new() -> Tree {
        let leaf = Node::new_leaf();
        let root = Node::new_internal(vec![leaf.clone()]);
        leaf.borrow_mut().parent = Rc::downgrade(&root);
        Tree { root }
    }
}

impl Node {
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

impl Tree {
    pub fn render(&self) -> gtk::Widget {
        self.root.borrow().render()
    }
}

pub type ViewTree = Tree;
