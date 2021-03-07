mod buffer;
mod highlight;
mod key_map;
mod key_sequence;
mod minibuf;
mod pane;
mod pane_tree;
mod persistence;
mod shell;
mod shell_unix;
mod theme;

use {
    anyhow::Error,
    buffer::Embuf,
    crossbeam_channel::Sender,
    fehler::throws,
    gtk4::{self as gtk, gdk, glib, glib::signal::Inhibit, prelude::*},
    highlight::{highlighter_thread, HighlightRequest},
    key_map::{Action, KeyMap, KeyMapLookup, KeyMapStack},
    key_sequence::{KeySequence, KeySequenceAtom},
    log::error,
    minibuf::{Minibuf, MinibufState},
    pane::Pane,
    pane_tree::PaneTree,
    std::{
        cell::RefCell,
        env,
        path::{Path, PathBuf},
        thread,
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

fn is_modifier(key: &gdk::keys::Key) -> bool {
    matches!(
        *key,
        gdk::keys::constants::Alt_L
            | gdk::keys::constants::Alt_R
            | gdk::keys::constants::Control_L
            | gdk::keys::constants::Control_R
            | gdk::keys::constants::Shift_L
            | gdk::keys::constants::Shift_R
    )
}

// For debugging.
#[allow(dead_code)]
fn dump_tree<W: IsA<gtk::Widget>>(widget: &W, title: &str) {
    fn r<W: IsA<gtk::Widget>>(widget: &W, depth: usize) {
        for _ in 0..depth {
            print!("  ");
        }
        println!("{} ({:?})", widget.get_widget_name(), widget);

        // Don't recurse into Pane widgets.
        if widget.get_widget_name() == "Pane" {
            return;
        }

        // Dump children.
        let mut iter = widget.get_first_child();
        while let Some(child) = iter {
            r(&child, depth + 1);
            iter = child.get_next_sibling();
        }
    }

    println!("{}", title);
    r(widget, 0);
    println!();
}

pub struct App {
    window: gtk::ApplicationWindow,
    minibuf: Minibuf,
    pane_tree: PaneTree,
    split_root: gtk::Box,
    buffers: Vec<Embuf>,

    base_keymap: KeyMap,
    cur_seq: KeySequence,

    highlight_request_sender: Sender<HighlightRequest>,
}

impl App {
    fn set_active_pane(&mut self, pane: Pane) {
        self.pane_tree.set_active(pane);
    }

    #[throws]
    fn open_file(&mut self, path: &Path) {
        // TODO: handle error
        let embuf =
            Embuf::load_file(path, self.highlight_request_sender.clone())?;

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

    #[throws]
    fn handle_minibuf_confirm(&mut self) {
        match self.minibuf.state() {
            MinibufState::Inactive => {}
            MinibufState::OpenFile => {
                let input = self.minibuf.take_input();
                self.open_file(Path::new(input.as_str()))?;
            }
            MinibufState::SelectBuffer => {
                let input = self.minibuf.take_input();
                self.switch_to_buffer(&input);
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

    let minibuf = Minibuf::new();

    layout.append(&split_root);
    layout.append(&minibuf.widget());

    window.set_child(Some(&layout));

    let (hl_req_sender, hl_req_receiver) = crossbeam_channel::unbounded();
    thread::spawn(|| highlighter_thread(hl_req_receiver));

    let mut app = App {
        window: window.clone(),
        minibuf,
        pane_tree,
        split_root,
        buffers: vec![embuf],

        base_keymap: KeyMap::new(),
        cur_seq: KeySequence::default(),

        highlight_request_sender: hl_req_sender.clone(),
    };

    match persistence::restore_embufs(hl_req_sender) {
        Ok(restore_info) => app.buffers.extend(restore_info),
        Err(err) => {
            error!("failed to restore buffers: {:#}", err);
        }
    }

    if let Ok(layout_history) = persistence::get_layout_history() {
        if let Some(layout) = layout_history.first() {
            app.pane_tree.deserialize(layout, &app.buffers);
        }
    }
    app.update_pane_tree();

    // Scrolling doesn't work until the window is shown, so post it to
    // an idle callback.
    glib::idle_add(|| {
        APP.with(|app| {
            let app = app.borrow();
            // OK to unwrap because APP is always set on the main
            // thread by the time this callback is run.
            let app = app.as_ref().expect("APP is not set");
            restore_scroll_positions(app);
        });
        Continue(false)
    });

    for path in &opt.files {
        // TODO: unwrap
        app.open_file(path).unwrap();
    }

    APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });

    if let Err(err) = persistence::restore_window_layout(&window) {
        error!("failed to restore window layout: {:#}", err);
    }

    window.show();

    glib::timeout_add_seconds(1, || {
        APP.with(|app| {
            let app = app.borrow();
            // OK to unwrap because APP is always set on the main
            // thread by the time this timeout callback is added.
            let app = app.as_ref().expect("APP is not set");
            // TODO: right now this is going to force a wakeup and DB
            // write every second, not very power friendly. Should
            // think about how to do a dirty bit without making things
            // too complicated.
            if let Err(err) = persistence::persist_app(app) {
                error!("persist failed: {:#}", err);
            }
        });
        Continue(true)
    });
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

    persistence::init_db().unwrap();

    let application =
        gtk::Application::new(Some("org.emma.Emma"), Default::default())
            .expect("Initialization failed...");

    application.connect_activate(move |app| build_ui(app, &opt));

    application.run(&[]);
}
