use gio::prelude::*;
use gtk::prelude::*;
use std::env;
use glib::Cast;
use gtk::BoxExt;
use std::cell::RefCell;
use std::rc::{Rc, Weak};

pub type View = gtk::TextView;

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
            NodeContents::Leaf(view) => view.clone().upcast(),
        }
    }
}

impl Tree {
    pub fn render(&self) -> gtk::Widget {
        self.root.borrow().render()
    }
}

fn make_box(o: gtk::Orientation) -> gtk::Box {
    let spacing = 1;
    gtk::Box::new(o, spacing)
}

fn pack<W: IsA<gtk::Widget>>(layout: &gtk::Box, child: &W) {
    let expand = true;
    let fill = true;
    let padding = 0;
    layout.pack_start(child, expand, fill, padding);
}

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("emma");
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(640, 480);

    let layout = make_box(gtk::Orientation::Vertical);
    layout.set_widget_name("root_layout");

    // Arbitrary orientation since it contains a single element.
    let view_tree_container = make_box(gtk::Orientation::Horizontal);
    view_tree_container.set_widget_name("view_tree_container");
    let view_tree = Tree::new();

    let minibuf = gtk::TextView::new();
    minibuf.set_size_request(-1, 26); // TODO

    pack(&layout, &view_tree_container);
    layout.pack_start(&minibuf, false, true, 0);

    pack(&view_tree_container, &view_tree.render());

    window.add(&layout);
    // TODO: use clone macro
    let window2 = window.clone();

    view_tree_container.remove(&view_tree_container.get_children()[0]);
    pack(&view_tree_container, &view_tree.render());

    window2.show_all();
}

fn main() {
    let application =
        gtk::Application::new(Some("org.emma.Emma"), Default::default())
            .expect("Initialization failed...");

    application.connect_activate(|app| {
        build_ui(app);
    });

    application.run(&env::args().collect::<Vec<_>>());
}
