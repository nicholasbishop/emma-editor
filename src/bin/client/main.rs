mod buffer;
mod highlight;
mod key_map;
mod key_sequence;
mod pane;
mod shell;
mod shell_unix;
mod theme;

use {
    buffer::Embuf,
    crossbeam_channel::Sender,
    gtk4::{self as gtk, gdk, glib::signal::Inhibit, prelude::*},
    highlight::{highlighter_thread, HighlightRequest},
    key_map::{Action, KeyMap, KeyMapLookup, KeyMapStack},
    key_sequence::{KeySequence, KeySequenceAtom},
    pane::Pane,
    std::{
        cell::RefCell,
        path::{Path, PathBuf},
        {env, fs, thread},
    },
};

// This global is needed for callbacks on the main thread. On other
// threads it is None.
std::thread_local! {
    static APP: RefCell<Option<App>> = RefCell::new(None);
}

#[derive(Clone, Copy, Eq, PartialEq)]
enum MinibufState {
    Inactive,
    // TODO this will probably become more general
    OpenFile,
}

fn make_box(o: gtk::Orientation) -> gtk::Box {
    let spacing = 1;
    gtk::Box::new(o, spacing)
}

/// Set horizontal+vertical expand+fill on a widget.
fn make_big<W: IsA<gtk::Widget>>(widget: &W) {
    widget.set_halign(gtk::Align::Fill);
    widget.set_valign(gtk::Align::Fill);
    widget.set_hexpand(true);
    widget.set_vexpand(true);
}

fn get_minibuf_keymap(state: MinibufState) -> KeyMap {
    let mut map = KeyMap::new();
    match state {
        MinibufState::Inactive => {}
        MinibufState::OpenFile => {
            map.insert(KeySequence::parse("<ret>").unwrap(), Action::Confirm);
        }
    }
    map
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

struct App {
    window: gtk::ApplicationWindow,
    minibuf: gtk::TextView,
    views: Vec<Pane>,
    buffers: Vec<Embuf>,
    active_pane: Pane,

    base_keymap: KeyMap,
    minibuf_state: MinibufState,
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
        if self.window.get_focus() == Some(self.minibuf.clone().upcast()) {
            keymap_stack.push(get_minibuf_keymap(self.minibuf_state));
        }
        if self.active_pane.embuf().has_shell() {
            let mut map = KeyMap::new();
            map.insert(KeySequence::parse("<ret>").unwrap(), Action::Confirm);
            keymap_stack.push(map);
        }

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
                self.minibuf_state = MinibufState::OpenFile;
                self.minibuf.grab_focus();

                let prompt = "Open file: ";
                let buf = self.minibuf.get_buffer();

                // Create prompt tag.
                let tag = gtk::TextTag::new(Some("prompt"));
                tag.set_property_editable(false);
                tag.set_property_foreground(Some("#edd400"));
                buf.get_tag_table().add(&tag);

                // Add prompt text and apply tag.
                buf.set_text(prompt);
                let start = buf.get_start_iter();
                let mut prompt_end =
                    buf.get_iter_at_offset(prompt.len() as i32);
                buf.apply_tag(&tag, &start, &prompt_end);

                // Insert mark to indicate the beginning of the user
                // input.
                let mark_name = "input-start";
                if let Some(mark) = buf.get_mark(mark_name) {
                    buf.delete_mark(&mark);
                }
                let left_gravity = true;
                buf.create_mark(Some(mark_name), &prompt_end, left_gravity);

                // Insert current directory.
                // TODO fix unwrap
                buf.insert(
                    &mut prompt_end,
                    env::current_dir().unwrap().to_str().unwrap(),
                );
            }
            KeyMapLookup::Action(Action::PreviousPane) => {
                let pos = self
                    .views
                    .iter()
                    .position(|e| e == &self.active_pane)
                    .unwrap();
                let prev = if pos == 0 {
                    self.views.len() - 1
                } else {
                    pos - 1
                };
                self.set_active_pane(self.views[prev].clone());
            }
            KeyMapLookup::Action(Action::NextPane) => {
                let pos = self
                    .views
                    .iter()
                    .position(|e| e == &self.active_pane)
                    .unwrap();
                let next = if pos == self.views.len() - 1 {
                    0
                } else {
                    pos + 1
                };
                self.set_active_pane(self.views[next].clone());
            }
            KeyMapLookup::Action(Action::SplitHorizontal) => {
                self.split_view(gtk::Orientation::Horizontal);
            }
            KeyMapLookup::Action(Action::SplitVertical) => {
                self.split_view(gtk::Orientation::Vertical);
            }
            KeyMapLookup::Action(Action::ClosePane) => {
                todo!();
            }
            KeyMapLookup::Action(Action::Confirm) => {
                if self.minibuf.has_focus() {
                    self.handle_minibuf_confirm();
                } else if self.active_pane.embuf().has_shell() {
                    // TODO: unwrap
                    self.active_pane.embuf().send_to_shell().unwrap();
                }
            }
            KeyMapLookup::Action(Action::PageUp) => {
                self.active_pane.view().emit_move_cursor(
                    gtk::MovementStep::Pages,
                    -1,
                    false,
                );
            }
            KeyMapLookup::Action(Action::PageDown) => {
                self.active_pane.view().emit_move_cursor(
                    gtk::MovementStep::Pages,
                    1,
                    false,
                );
            }
            KeyMapLookup::Action(Action::OpenShell) => {
                // TODO fix unwrap
                let embuf = Embuf::launch_shell().unwrap();
                self.buffers.push(embuf.clone());
                self.active_pane.set_buffer(&embuf);
            }
            KeyMapLookup::Action(Action::Cancel) => {
                if self.minibuf_state != MinibufState::Inactive {
                    self.cancel_minibuf();
                }
            }
        };

        if clear_seq {
            self.cur_seq.0.clear();
        }

        inhibit
    }

    fn set_active_pane(&mut self, pane: Pane) {
        self.active_pane = pane;
        for pane in &self.views {
            pane.set_active(false);
        }
        self.active_pane.set_active(true);
    }

    fn open_file(&mut self, path: &Path) {
        // TODO: we may end up not needing sourceview since we're
        // already not using it for highlighting...

        // TODO: check out the async loading feature of
        // sourceview. It says its unmaintained though and to
        // check out tepl...

        // TODO: handle error
        let contents = fs::read_to_string(path).unwrap();

        let embuf = Embuf::new(path.into());

        let sender = self.highlight_request_sender.clone();
        let storage = embuf.storage();
        self.buffers.push(embuf.clone());
        let embuf_clone = embuf.clone();

        storage.connect_changed(move |_| {
            embuf.increment_generation();

            let storage = embuf.storage();

            let start = storage.get_start_iter();
            let end = storage.get_end_iter();
            let text = storage.get_text(&start, &end, true);

            let req = HighlightRequest {
                buffer_id: embuf.buffer_id(),
                text: text.to_string(),
                generation: embuf.generation(),
                path: embuf.path(),
            };
            sender.send(req).unwrap();
        });
        storage.set_text(&contents);

        self.active_pane.set_buffer(&embuf_clone);
        // Move the cursor from the end to the beginning of the buffer.
        self.active_pane.view().emit_move_cursor(
            gtk::MovementStep::BufferEnds,
            -1,
            false,
        );
    }

    fn handle_minibuf_confirm(&mut self) {
        match self.minibuf_state {
            MinibufState::Inactive => {}
            MinibufState::OpenFile => {
                let buf = self.minibuf.get_buffer();

                // TODO: dedup
                let mark_name = "input-start";
                let mark = buf.get_mark(mark_name).unwrap();
                let start = buf.get_iter_at_mark(&mark);
                let end = buf.get_end_iter();

                let text = buf.get_text(&start, &end, false);

                buf.set_text("");

                self.minibuf_state = MinibufState::Inactive;

                self.open_file(Path::new(text.as_str()));
            }
        }
    }

    fn cancel_minibuf(&mut self) {
        match self.minibuf_state {
            MinibufState::Inactive => {}
            MinibufState::OpenFile => {
                let buf = self.minibuf.get_buffer();

                buf.set_text("");

                self.minibuf_state = MinibufState::Inactive;
            }
        }
    }

    fn split_view(&mut self, orientation: gtk::Orientation) {
        let active = &self.active_pane;

        // TODO: a more explicit tree structure might make this easier --
        // similar to how we do with the views vec
        if let Some(parent) = self.active_pane.get_widget().get_parent() {
            if let Some(layout) = parent.dynamic_cast_ref::<gtk::Box>() {
                let new_view = Pane::new(&active.embuf());
                let new_widget = new_view.get_widget();
                make_big(&new_widget);

                // TODO
                let active_index =
                    self.views.iter().position(|e| e == active).unwrap();
                self.views.insert(active_index + 1, new_view);

                // Check if the layout is in the correct orientation.
                if layout.get_orientation() == orientation {
                    // Insert after active pane.
                    layout.insert_child_after(
                        &new_widget,
                        Some(&active.get_widget()),
                    );
                } else {
                    // If there's only the one view in the layout,
                    // just switch the orientation. Otherwise, create
                    // a new layout to subdivide.
                    if layout.get_first_child() == layout.get_last_child() {
                        layout.set_orientation(orientation);
                        layout.append(&new_widget);
                    } else {
                        let new_layout = make_box(orientation);
                        make_big(&new_layout);

                        // Insert the new layout after the active pane.
                        layout.insert_child_after(
                            &new_layout,
                            Some(&active.get_widget()),
                        );

                        // Move the active pane from the old layout
                        // to the new layout
                        layout.remove(&active.get_widget());
                        new_layout.append(&active.get_widget());

                        // Add the new pane to the new layout.
                        new_layout.append(&new_widget);
                    }
                }
            }
        }
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

    let split_root = make_box(gtk::Orientation::Horizontal);
    let embuf = Embuf::new(Path::new("").into()); // TODO: should be path None
    let text = Pane::new(&embuf);
    text.set_active(true);
    make_big(&split_root);
    make_big(&text.get_widget());
    split_root.append(&text.get_widget());

    let minibuf = gtk::TextView::new();
    minibuf.set_size_request(-1, 26); // TODO

    layout.append(&split_root);
    layout.append(&minibuf);

    window.set_child(Some(&layout));

    let (hl_req_sender, hl_req_receiver) = crossbeam_channel::unbounded();
    thread::spawn(|| highlighter_thread(hl_req_receiver));

    let mut app = App {
        window: window.clone(),
        minibuf,
        views: vec![text.clone()],
        buffers: vec![embuf],
        active_pane: text,

        base_keymap: KeyMap::new(),
        minibuf_state: MinibufState::Inactive,
        cur_seq: KeySequence::default(),

        highlight_request_sender: hl_req_sender,
    };

    for path in &opt.files {
        app.open_file(path);
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
    // TODO: glib has its own arg parsing that we could look at using,
    // but it's more complicated to understand than argh.
    let opt: Opt = argh::from_env();

    let application =
        gtk::Application::new(Some("org.emma.Emma"), Default::default())
            .expect("Initialization failed...");

    application.connect_activate(move |app| build_ui(app, &opt));

    application.run(&[]);
}
