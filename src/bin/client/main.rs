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
    gtk4::{self as gtk, gdk, glib::signal::Inhibit, prelude::*},
    highlight::{highlighter_thread, HighlightRequest},
    key_map::{Action, KeyMap, KeyMapLookup, KeyMapStack},
    key_sequence::{KeySequence, KeySequenceAtom},
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

struct App {
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
    fn handle_key_press(
        &mut self,
        key: gdk::keys::Key,
        state: gdk::ModifierType,
    ) -> Inhibit {
        let mut keymap_stack = KeyMapStack::default();
        keymap_stack.push(self.base_keymap.clone());

        // TODO: figure these customizations out better
        if self.minibuf.has_focus() {
            keymap_stack.push(self.minibuf.get_keymap());
        }
        if self.pane_tree.active().embuf().has_shell() {
            let mut map = KeyMap::new();
            map.insert(KeySequence::parse("<ret>").unwrap(), Action::Confirm);
            keymap_stack.push(map);
        }

        let text_view = if self.minibuf.state() == MinibufState::Inactive {
            self.pane_tree.active().view()
        } else {
            self.minibuf.view()
        };

        // Ignore lone modifier presses.
        if is_modifier(&key) {
            return Inhibit(false);
        }

        // TODO: we want to ignore combo modifier presses too if no
        // non-modifier key is selected, e.g. pressing alt and then
        // shift, but currently that is treated as a valid
        // sequence. Need to figure out how to prevent that.

        let atom = KeySequenceAtom::from_event(key, state);
        self.cur_seq.0.push(atom);

        let mut clear_seq = true;
        let mut inhibit = Inhibit(true);
        match keymap_stack.lookup(&self.cur_seq) {
            KeyMapLookup::NoEntry => {
                // Allow default handling to occur, e.g. inserting a
                // character into the text widget.
                inhibit = Inhibit(false);
            }
            KeyMapLookup::BadSequence => {
                // TODO: display some kind of non-blocking error
                dbg!("bad seq", &self.cur_seq);
            }
            KeyMapLookup::Prefix => {
                clear_seq = false;
                // Waiting for the sequence to be completed.
            }
            KeyMapLookup::Action(Action::Exit) => {
                dbg!("close!");
                self.window.close();
            }
            KeyMapLookup::Action(Action::OpenFile) => {
                self.minibuf.set_state(MinibufState::OpenFile);

                // Insert current directory.
                // TODO fix unwrap
                let def =
                    env::current_dir().unwrap().to_str().unwrap().to_string();
                self.minibuf.start_input("Open file: ", &def);
            }
            KeyMapLookup::Action(Action::SaveFile) => {
                self.pane_tree.active().embuf().save();
            }
            KeyMapLookup::Action(Action::SwitchToBuffer) => {
                self.minibuf.set_state(MinibufState::SelectBuffer);

                // TODO: provide a way to tab complete buffers and
                // list them, for now just print to console.
                for embuf in &self.buffers {
                    println!("{}", embuf.name());
                }

                self.minibuf.start_input("Select buffer: ", "");
            }
            KeyMapLookup::Action(Action::PreviousPane) => {
                let views = self.pane_tree.leaf_vec();
                let pos = views
                    .iter()
                    .position(|e| e == &self.pane_tree.active())
                    .unwrap();
                let prev = if pos == 0 { views.len() - 1 } else { pos - 1 };
                self.set_active_pane(views[prev].clone());
            }
            KeyMapLookup::Action(Action::NextPane) => {
                let views = self.pane_tree.leaf_vec();
                let pos = views
                    .iter()
                    .position(|e| e == &self.pane_tree.active())
                    .unwrap();
                let next = if pos == views.len() - 1 { 0 } else { pos + 1 };
                self.set_active_pane(views[next].clone());
            }
            KeyMapLookup::Action(Action::SplitHorizontal) => {
                self.pane_tree.split(gtk::Orientation::Horizontal);
                self.update_pane_tree();
            }
            KeyMapLookup::Action(Action::SplitVertical) => {
                self.pane_tree.split(gtk::Orientation::Vertical);
                self.update_pane_tree();
            }
            KeyMapLookup::Action(Action::ClosePane) => {
                self.pane_tree.close();
                self.update_pane_tree();
            }
            KeyMapLookup::Action(Action::Confirm) => {
                if self.minibuf.has_focus() {
                    if let Err(err) = self.handle_minibuf_confirm() {
                        self.minibuf.set_state(MinibufState::Inactive);
                        self.minibuf.show_error(err);
                    } else {
                        self.minibuf.set_state(MinibufState::Inactive);
                    }
                    self.pane_tree.active().grab_focus();
                } else if self.pane_tree.active().embuf().has_shell() {
                    // TODO: unwrap
                    self.pane_tree.active().embuf().send_to_shell().unwrap();
                }
            }
            KeyMapLookup::Action(Action::BackChar) => {
                text_view.emit_move_cursor(
                    gtk::MovementStep::VisualPositions,
                    -1,
                    false,
                );
            }
            KeyMapLookup::Action(Action::ForwardChar) => {
                text_view.emit_move_cursor(
                    gtk::MovementStep::VisualPositions,
                    1,
                    false,
                );
            }
            KeyMapLookup::Action(Action::BackLine) => {
                text_view.emit_move_cursor(
                    gtk::MovementStep::DisplayLines,
                    -1,
                    false,
                );
            }
            KeyMapLookup::Action(Action::ForwardLine) => {
                text_view.emit_move_cursor(
                    gtk::MovementStep::DisplayLines,
                    1,
                    false,
                );
            }
            KeyMapLookup::Action(Action::PageUp) => {
                self.pane_tree.active().view().emit_move_cursor(
                    gtk::MovementStep::Pages,
                    -1,
                    false,
                );
            }
            KeyMapLookup::Action(Action::PageDown) => {
                self.pane_tree.active().view().emit_move_cursor(
                    gtk::MovementStep::Pages,
                    1,
                    false,
                );
            }
            KeyMapLookup::Action(Action::OpenShell) => {
                // TODO fix unwrap
                let embuf = Embuf::launch_shell("TODO").unwrap();
                self.buffers.push(embuf.clone());
                self.pane_tree.active().set_buffer(&embuf);
            }
            KeyMapLookup::Action(Action::Cancel) => {
                if self.minibuf.state() != MinibufState::Inactive {
                    self.minibuf.cancel();
                    self.pane_tree.active().grab_focus();
                }
            }
            // TODO: move keymap handling for minibuf to minibuf impl?
            KeyMapLookup::Action(Action::Autocomplete) => {
                if self.minibuf.state() != MinibufState::Inactive {
                    self.minibuf.autocomplete();
                }
            }
        };

        if clear_seq {
            self.cur_seq.0.clear();
        }

        inhibit
    }

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

        persistence::add_embuf(&embuf).unwrap();
    }

    fn switch_to_buffer(&self, name: &str) {
        for embuf in &self.buffers {
            if embuf.name() == name {
                self.pane_tree.active().set_buffer(&embuf);
                break;
            }
        }
        persistence::store_layout(&self.pane_tree).unwrap();
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

        persistence::store_layout(&self.pane_tree).unwrap();
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

    app.buffers
        .extend(persistence::restore_embufs(hl_req_sender).unwrap());

    if let Ok(layout_history) = persistence::get_layout_history() {
        if let Some(layout) = layout_history.first() {
            app.pane_tree.deserialize(layout, &app.buffers);
        }
    }
    app.update_pane_tree();

    for path in &opt.files {
        // TODO: unwrap
        app.open_file(path).unwrap();
    }

    APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });

    let key_controller = gtk::EventControllerKey::new();
    key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    key_controller.connect_key_pressed(
        move |_self, keyval, _keycode, state| {
            APP.with(|app| {
                app.borrow_mut()
                    .as_mut()
                    .unwrap()
                    .handle_key_press(keyval, state)
            })
        },
    );
    window.add_controller(&key_controller);

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
    simple_logger::SimpleLogger::new().init().unwrap();

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
