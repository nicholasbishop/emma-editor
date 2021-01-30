mod view_tree;

use gio::prelude::*;
use gtk::prelude::*;
use std::env;
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
    let view_tree = ViewTree::new();

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
