use gtk4::{self as gtk, gdk, prelude::*};

/// Set horizontal+vertical expand+fill on a widget.
fn make_big<W: IsA<gtk::Widget>>(widget: &W) {
    widget.set_halign(gtk::Align::Fill);
    widget.set_valign(gtk::Align::Fill);
    widget.set_hexpand(true);
    widget.set_vexpand(true);
}

fn build_ui(application: &gtk::Application) {
    let window = gtk::ApplicationWindow::new(application);

    window.set_title(Some("emma"));
    window.set_default_size(800, 800);

    let css = gtk::CssProvider::new();
    css.load_from_data(include_bytes!("theme.css"));
    gtk::StyleContext::add_provider_for_display(
        &gdk::Display::get_default().unwrap(),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let layout = gtk::Box::new(gtk::Orientation::Vertical, 0);

    // Arbitrary orientation, it only ever holds one widget.
    let split_root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    make_big(&split_root);
    //split_root.append(&pane_tree.render());
    let tv = gtk::TextView::new();
    make_big(&tv);
    split_root.append(&tv);

    layout.append(&split_root);

    window.set_child(Some(&layout));

    window.show();
}

fn main() {
    env_logger::init();

    let application =
        gtk::Application::new(Some("org.emma.Emma"), Default::default())
            .expect("Initialization failed...");

    application.connect_activate(move |app| build_ui(app));

    application.run(&[]);
}
