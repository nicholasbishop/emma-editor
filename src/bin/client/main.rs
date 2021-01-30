mod key_map;
mod key_sequence;
mod view_tree;

use gio::prelude::*;
use gtk::prelude::*;
use key_map::{Action, KeyMap, KeyMapLookup};
use key_sequence::{KeySequence, KeySequenceAtom};
use std::cell::RefCell;
use std::env;
use std::rc::Rc;
use view_tree::ViewTree;

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

fn get_widget_index_in_container<
    L: IsA<gtk::Container>,
    W: IsA<gtk::Widget>,
>(
    layout: &L,
    widget: &W,
) -> Option<usize> {
    layout.get_children().iter().position(|elem| elem == widget)
}

fn print_widget_tree_impl<W: IsA<gtk::Widget>>(root: &W, depth: usize) {
    // Indent
    let mut line = String::new();
    for _ in 0..depth {
        line.push_str("  ");
    }

    let root_type = root.get_type().to_string();
    let root_name = root.get_widget_name().to_string();
    line.push_str(&root_type);

    // The default name is just the type, so skip it if it is that
    if root_name != root_type {
        line.push_str(" - ");
        line.push_str(&root_name);
    }

    line.push_str(&format!(" (refcount={})", root.ref_count()));

    println!("{}", line);

    // Print children
    if let Some(container) = root.dynamic_cast_ref::<gtk::Container>() {
        for child in container.get_children() {
            print_widget_tree_impl(&child, depth + 1);
        }
    }
}

fn print_widget_tree<W: IsA<gtk::Widget>>(root: &W, msg: &str) {
    println!("{}", msg);
    print_widget_tree_impl(root, 0);
    println!();
}

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("emma");
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(640, 480);

    let css = gtk::CssProvider::new();
    css.load_from_data(include_bytes!("theme.css")).unwrap();
    gtk::StyleContext::add_provider_for_screen(
        &gdk::Screen::get_default().unwrap(),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let layout = make_box(gtk::Orientation::Vertical);
    layout.set_widget_name("root_layout");

    // Arbitrary orientation since it contains a single element.
    let view_tree_container = make_box(gtk::Orientation::Horizontal);
    view_tree_container.set_widget_name("view_tree_container");
    let view_tree = ViewTree::new();

    let minibuf = gtk::TextView::new();
    minibuf.set_size_request(-1, 26); // TODO

    pack(&layout, &view_tree_container);
    layout.pack_start(&minibuf, false, true, 0);

    pack(&view_tree_container, &view_tree.render());

    window.add(&layout);
    // TODO: use clone macro
    let window2 = window.clone();

    let keymap = KeyMap::new();
    let cur_seq = Rc::new(RefCell::new(KeySequence::default()));

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
