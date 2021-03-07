mod buffer;
mod pane;
mod pane_tree;
mod theme;

use {
    anyhow::Error,
    buffer::Embuf,
    fehler::throws,
    gtk4::{self as gtk, gdk, prelude::*},
    pane::Pane,
    pane_tree::PaneTree,
    std::{
        cell::RefCell,
        path::{Path, PathBuf},
    },
};

// This global is needed for callbacks on the main thread. On other
// threads it is None.
std::thread_local! {
    static APP: RefCell<Option<App>> = RefCell::new(None);
}

/// Set horizontal+vertical expand+fill on a widget.
fn make_big<W: IsA<gtk::Widget>>(widget: &W) {
    widget.set_halign(gtk::Align::Fill);
    widget.set_valign(gtk::Align::Fill);
    widget.set_hexpand(true);
    widget.set_vexpand(true);
}

pub struct App {
    window: gtk::ApplicationWindow,
    pane_tree: PaneTree,
    split_root: gtk::Box,
    buffers: Vec<Embuf>,
}

impl App {
    fn set_active_pane(&mut self, pane: Pane) {
        self.pane_tree.set_active(pane);
    }

    #[throws]
    fn open_file(&mut self, path: &Path) {
        // TODO: handle error
        let embuf = Embuf::load_file(path)?;

        self.buffers.push(embuf.clone());

        self.pane_tree.active().set_buffer(&embuf);
        // Move the cursor from the end to the beginning of the buffer.
        self.pane_tree.active().view().emit_move_cursor(
            gtk::MovementStep::BufferEnds,
            -1,
            false,
        );
    }

    fn switch_to_buffer(&self, name: &str) {
        for embuf in &self.buffers {
            if embuf.name() == name {
                self.pane_tree.active().set_buffer(&embuf);
                break;
            }
        }
    }

    fn update_pane_tree(&self) {
        pane_tree::recursive_unparent_children(&self.split_root);
        self.split_root.append(&self.pane_tree.render());
    }
}

/// Hacky: scroll the restored buffers so that the cursor is visible.
fn restore_scroll_positions(app: &App) {
    for pane in app.pane_tree.leaf_vec() {
        let buf = pane.view().get_buffer();
        let offset = buf.get_property_cursor_position();
        let mut iter = buf.get_iter_at_offset(offset);
        let within_margin = 0.0;
        let use_align = false;
        let xalign = 0.0;
        let yalign = 0.0;
        pane.view().scroll_to_iter(
            &mut iter,
            within_margin,
            use_align,
            xalign,
            yalign,
        );
    }
}

fn build_ui(application: &gtk::Application, opt: &Opt) {
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

    let embuf = Embuf::new(Path::new("").into()); // TODO: should be path None
    let text = Pane::new(&embuf);
    make_big(&text.get_widget());
    text.set_active(true);

    let pane_tree = PaneTree::new(text);
    // Arbitrary orientation, it only ever holds one widget.
    let split_root = gtk::Box::new(gtk::Orientation::Vertical, 0);
    make_big(&split_root);
    split_root.append(&pane_tree.render());

    layout.append(&split_root);

    window.set_child(Some(&layout));

    let mut app = App {
        window: window.clone(),
        pane_tree,
        split_root,
        buffers: vec![embuf],
    };

    app.update_pane_tree();

    for path in &opt.files {
        // TODO: unwrap
        app.open_file(path).unwrap();
    }

    APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });

    window.show();
}

/// Emma text editor.
#[derive(argh::FromArgs)]
struct Opt {
    /// files to open on startup.
    #[argh(positional)]
    files: Vec<PathBuf>,
}

fn main() {
    env_logger::init();

    // TODO: glib has its own arg parsing that we could look at using,
    // but it's more complicated to understand than argh.
    let opt: Opt = argh::from_env();

    let application =
        gtk::Application::new(Some("org.emma.Emma"), Default::default())
            .expect("Initialization failed...");

    application.connect_activate(move |app| build_ui(app, &opt));

    application.run(&[]);
}
