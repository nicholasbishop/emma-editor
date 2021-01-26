use std::cell::RefCell;
use std::rc::{Rc, Weak};

type View = gtk::TextView;

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

struct InternalNode {
    children: Vec<NodePtr>,
    orientation: Orientation,
}

enum NodeContents {
    Internal(InternalNode),
    Leaf(View),
}

struct Node {
    contents: NodeContents,
    parent: NodeWeakPtr,
}

impl Node {
    fn new_leaf() -> NodePtr {
        NodePtr::new(RefCell::new(Node {
            contents: NodeContents::Leaf(View::new()),
            parent: NodeWeakPtr::new(),
        }))
    }

    fn internal(&self) -> Option<&InternalNode> {
        match &self.contents {
            NodeContents::Internal(internal) => Some(internal),
            _ => None,
        }
    }

    fn child_index(&self, child: NodePtr) -> Option<usize> {
        if let NodeContents::Internal(internal) = &self.contents {
            internal.children.iter().position(|e| Rc::ptr_eq(e, &child))
        } else {
            None
        }
    }

    fn insert(&self, _index: usize, _child: NodePtr) {
        todo!();
    }
}

type NodePtr = Rc<RefCell<Node>>;
type NodeWeakPtr = Weak<RefCell<Node>>;

pub struct ViewTree {
    root: NodePtr,
    active: NodePtr,
}

impl ViewTree {
    /// Create a ViewTree containing a single View.
    pub fn new() -> ViewTree {
        let leaf = Node::new_leaf();
        let root = NodePtr::new(RefCell::new(Node {
            contents: NodeContents::Internal(InternalNode {
                children: vec![leaf.clone()],
                orientation: Orientation::None,
            }),
            parent: NodeWeakPtr::new(),
        }));
        leaf.borrow_mut().parent = Rc::downgrade(&root);
        ViewTree { active: leaf, root }
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
