mod buffer;
mod highlight;
mod key_map;
mod key_sequence;
mod pane;
mod theme;

use buffer::EmBuf;
use crossbeam_channel::Sender;
use gio::prelude::*;
use gtk::prelude::*;
use highlight::{highlighter_thread, HighlightRequest};
use key_map::{Action, KeyMap, KeyMapLookup, KeyMapStack};
use key_sequence::{KeySequence, KeySequenceAtom};
use pane::Pane;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::{env, fs, thread};

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

fn split_view(
    window: &gtk::ApplicationWindow,
    orientation: gtk::Orientation,
    views: &mut Vec<Pane>,
) {
    // TODO: a more explicit tree structure might make this easier --
    // similar to how we do with the views vec
    if let Some(focus) = window.get_focus() {
        if let Some(parent) = focus.get_parent() {
            if let Some(layout) = parent.dynamic_cast_ref::<gtk::Box>() {
                let new_view = Pane::new();
                let new_widget = new_view.get_widget();
                let focus_index =
                    views.iter().position(|e| e.has_focus()).unwrap();
                views.insert(focus_index + 1, new_view);

                // Check if the layout is in the correct orientation.
                if layout.get_orientation() == orientation {
                    // Get the position of the current focused widget
                    // in its layout so that we can the new widget
                    // right after it.
                    let position =
                        get_widget_index_in_container(layout, &focus).unwrap();

                    pack(&layout, &new_widget);
                    layout.reorder_child(&new_widget, (position + 1) as i32);
                } else {
                    // If there's only the one view in the layout,
                    // just switch the orientation. Otherwise, create
                    // a new layout to subdivide.
                    if layout.get_children().len() == 1 {
                        layout.set_orientation(orientation);
                        pack(&layout, &new_widget);
                    } else {
                        let new_layout = make_box(orientation);

                        // Get the position of the current focused
                        // widget in its layout so that we can later
                        // put a new layout widget in the same place.
                        let position =
                            get_widget_index_in_container(layout, &focus)
                                .unwrap();

                        // Move the focused view from the old layout
                        // to the new layout
                        layout.remove(&focus);
                        pack(&new_layout, &focus);

                        // Add the new view and add the new layout.
                        pack(&new_layout, &new_widget);

                        // Add the new layout to the old layout, and
                        // move it to the right location. TODO: not
                        // sure if there's a better way to do this, or
                        // if the current way is always correct.
                        pack(layout, &new_layout);
                        layout.reorder_child(&new_layout, position as i32);
                    }
                }

                layout.show_all();
            }
        }
    }
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

struct App {
    window: gtk::ApplicationWindow,
    minibuf: gtk::TextView,
    views: Vec<Pane>,
    buffers: Vec<Rc<RefCell<EmBuf>>>,
    active_view: Pane,

    base_keymap: KeyMap,
    minibuf_state: MinibufState,
    cur_seq: KeySequence,

    highlight_request_sender: Sender<HighlightRequest>,
}

impl App {
    fn handle_key_press(&mut self, e: &gdk::EventKey) -> Inhibit {
        let mut keymap_stack = KeyMapStack::default();
        keymap_stack.push(self.base_keymap.clone());
        if self.window.get_focus() == Some(self.minibuf.clone().upcast()) {
            keymap_stack.push(get_minibuf_keymap(self.minibuf_state));
        }

        // Ignore lone modifier presses.
        if e.get_is_modifier() {
            return Inhibit(false);
        }

        // TODO: we want to ignore combo modifier presses too if no
        // non-modifier key is selected, e.g. pressing alt and then
        // shift, but currently that is treated as a valid
        // sequence. Need to figure out how to prevent that.

        let atom = KeySequenceAtom::from_event(e);
        self.cur_seq.0.push(atom);

        let mut clear_seq = true;
        let mut inhibit = true;
        match keymap_stack.lookup(&self.cur_seq) {
            KeyMapLookup::NoEntry => {
                // Allow default handling to occur, e.g. inserting a
                // character into the text widget.
                inhibit = false;
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
                let buf = self.minibuf.get_buffer().unwrap();

                // Create prompt tag.
                let tag = gtk::TextTag::new(Some("prompt"));
                tag.set_property_editable(false);
                tag.set_property_foreground(Some("#edd400"));
                buf.get_tag_table().unwrap().add(&tag);

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
                let pos =
                    self.views.iter().position(|e| e.has_focus()).unwrap();
                let prev = if pos == 0 {
                    self.views.len() - 1
                } else {
                    pos - 1
                };
                self.views[prev].grab_focus();
            }
            KeyMapLookup::Action(Action::NextPane) => {
                let pos =
                    self.views.iter().position(|e| e.has_focus()).unwrap();
                let next = if pos == self.views.len() - 1 {
                    0
                } else {
                    pos + 1
                };
                self.views[next].grab_focus();
            }
            KeyMapLookup::Action(Action::SplitHorizontal) => {
                split_view(
                    &self.window,
                    gtk::Orientation::Horizontal,
                    &mut self.views,
                );
            }
            KeyMapLookup::Action(Action::SplitVertical) => {
                split_view(
                    &self.window,
                    gtk::Orientation::Vertical,
                    &mut self.views,
                );
            }
            KeyMapLookup::Action(Action::ClosePane) => {
                todo!();
            }
            KeyMapLookup::Action(Action::Confirm) => {
                if self.minibuf.has_focus() {
                    self.handle_minibuf_confirm();
                }
            }
        };

        if clear_seq {
            self.cur_seq.0.clear();
        }

        Inhibit(inhibit)
    }

    fn open_file(&mut self, path: &Path) {
        // TODO: we may end up not needing sourceview since we're
        // already not using it for highlighting...

        // TODO: check out the async loading feature of
        // sourceview. It says its unmaintained though and to
        // check out tepl...

        // TODO: handle error
        let contents = fs::read_to_string(path).unwrap();

        let buffer = Rc::new(RefCell::new(EmBuf::new(path.into())));

        let sender = self.highlight_request_sender.clone();
        let buffer_clone = buffer.clone();
        let storage = buffer.borrow().storage.clone();
        storage.connect_changed(move |_| {
            let mut buffer = buffer_clone.borrow_mut();
            buffer.generation += 1;

            let storage = buffer.storage.clone();

            let start = storage.get_start_iter();
            let end = storage.get_end_iter();
            let text = storage.get_text(&start, &end, true).unwrap();

            let req = HighlightRequest {
                buffer_id: buffer.buffer_id.clone(),
                text: text.to_string(),
                generation: buffer.generation,
                path: buffer.path.clone(),
            };
            sender.send(req).unwrap();
        });
        storage.set_text(&contents);

        self.buffers.push(buffer);

        self.active_view.get_view().set_buffer(Some(&storage));
    }

    fn handle_minibuf_confirm(&mut self) {
        match self.minibuf_state {
            MinibufState::Inactive => {}
            MinibufState::OpenFile => {
                let buf = self.minibuf.get_buffer().unwrap();

                // TODO: dedup
                let mark_name = "input-start";
                let mark = buf.get_mark(mark_name).unwrap();
                let start = buf.get_iter_at_mark(&mark);
                let end = buf.get_end_iter();

                let text = buf.get_text(&start, &end, false).unwrap();

                buf.set_text("");

                self.minibuf_state = MinibufState::Inactive;

                self.open_file(Path::new(text.as_str()));
            }
        }
    }
}

fn build_ui(application: &gtk::Application, opt: &Opt) {
    let window = gtk::ApplicationWindow::new(application);

    window.set_title("emma");
    window.set_position(gtk::WindowPosition::Center);
    window.set_default_size(800, 1200);

    let css = gtk::CssProvider::new();
    css.load_from_data(include_bytes!("theme.css")).unwrap();
    gtk::StyleContext::add_provider_for_screen(
        &gdk::Screen::get_default().unwrap(),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    let layout = make_box(gtk::Orientation::Vertical);

    let split_root = make_box(gtk::Orientation::Horizontal);
    let text = Pane::new();
    pack(&split_root, &text.get_widget());

    let minibuf = gtk::TextView::new();
    minibuf.set_size_request(-1, 26); // TODO

    pack(&layout, &split_root);
    layout.pack_start(&minibuf, false, true, 0);

    window.add(&layout);

    window.add_events(gdk::EventMask::KEY_PRESS_MASK);

    let (hl_req_sender, hl_req_receiver) = crossbeam_channel::unbounded();
    thread::spawn(|| highlighter_thread(hl_req_receiver));

    let mut app = App {
        window: window.clone(),
        minibuf,
        views: vec![text.clone()],
        // TODO: doesn't yet include the initial view's buffer.
        buffers: Vec::new(),
        active_view: text,

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

    window.connect_key_press_event(move |_, e| {
        APP.with(|app| app.borrow_mut().as_mut().unwrap().handle_key_press(e))
    });

    window.show_all();
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
