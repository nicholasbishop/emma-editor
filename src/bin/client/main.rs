mod key_map;
mod key_sequence;

use gio::prelude::*;
use gtk::prelude::*;
use key_map::{Action, KeyMap, KeyMapLookup, KeyMapStack};
use key_sequence::{KeySequence, KeySequenceAtom};
use sourceview::prelude::*;
use std::cell::RefCell;
use std::rc::Rc;
use std::{env, fs};

#[derive(Clone, Copy, Eq, PartialEq)]
enum MinibufState {
    Inactive,
    // TODO this will probably become more general
    OpenFile,
}

#[derive(Clone, Eq, PartialEq)]
struct View(sourceview::View);

impl View {
    fn new() -> View {
        let view = sourceview::View::new();
        view.set_monospace(true);
        View(view)
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
    views: &mut Vec<View>,
) {
    // TODO: a more explicit tree structure might make this easier --
    // similar to how we do with the views vec
    if let Some(focus) = window.get_focus() {
        if let Some(parent) = focus.get_parent() {
            if let Some(layout) = parent.dynamic_cast_ref::<gtk::Box>() {
                let new_view = View::new();
                let focus_index =
                    views.iter().position(|e| e.0 == focus).unwrap();
                views.insert(focus_index + 1, new_view.clone());

                // Check if the layout is in the correct orientation.
                if layout.get_orientation() == orientation {
                    // Get the position of the current focused widget
                    // in its layout so that we can the new widget
                    // right after it.
                    let position =
                        get_widget_index_in_container(layout, &focus).unwrap();

                    pack(&layout, &new_view.0);
                    layout.reorder_child(&new_view.0, (position + 1) as i32);
                } else {
                    // If there's only the one view in the layout,
                    // just switch the orientation. Otherwise, create
                    // a new layout to subdivide.
                    if layout.get_children().len() == 1 {
                        layout.set_orientation(orientation);
                        pack(&layout, &new_view.0);
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
                        pack(&new_layout, &new_view.0);

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
    views: Vec<View>,
    buffers: Vec<sourceview::Buffer>,
    active_view: View,

    base_keymap: KeyMap,
    minibuf_state: MinibufState,
    cur_seq: KeySequence,
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
                let prompt_end = buf.get_iter_at_offset(prompt.len() as i32);
                buf.apply_tag(&tag, &start, &prompt_end);

                // Insert mark to indicate the beginning of the user
                // input.
                let mark_name = "input-start";
                if let Some(mark) = buf.get_mark(mark_name) {
                    buf.delete_mark(&mark);
                }
                let left_gravity = true;
                buf.create_mark(Some(mark_name), &prompt_end, left_gravity);
            }
            KeyMapLookup::Action(Action::PreviousView) => {
                if let Some(focus) = self.window.get_focus() {
                    let pos =
                        self.views.iter().position(|e| e.0 == focus).unwrap();
                    let prev = if pos == 0 {
                        self.views.len() - 1
                    } else {
                        pos - 1
                    };
                    self.views[prev].0.grab_focus();
                }
            }
            KeyMapLookup::Action(Action::NextView) => {
                if let Some(focus) = self.window.get_focus() {
                    let pos =
                        self.views.iter().position(|e| e.0 == focus).unwrap();
                    let next = if pos == self.views.len() - 1 {
                        0
                    } else {
                        pos + 1
                    };
                    self.views[next].0.grab_focus();
                }
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
            KeyMapLookup::Action(Action::CloseView) => {
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

                // TODO: check out the async loading feature of
                // sourceview. It says its unmaintained though and to
                // check out tepl...

                let path = text.as_str();

                // TODO: handle error
                let contents = fs::read_to_string(path).unwrap();

                let langman = sourceview::LanguageManager::new();
                let lang = langman.guess_language(Some(path), None);

                let tag_table: Option<&gtk::TextTagTable> = None;
                let buf = sourceview::Buffer::new(tag_table);
                buf.set_language(lang.as_ref());
                buf.set_text(&contents);

                self.buffers.push(buf.clone());

                self.active_view.0.set_buffer(Some(&buf));
            }
        }
    }
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

    let split_root = make_box(gtk::Orientation::Horizontal);
    let text = View::new();
    pack(&split_root, &text.0);

    let minibuf = gtk::TextView::new();
    minibuf.set_size_request(-1, 26); // TODO

    pack(&layout, &split_root);
    layout.pack_start(&minibuf, false, true, 0);

    window.add(&layout);

    window.add_events(gdk::EventMask::KEY_PRESS_MASK);

    let app = Rc::new(RefCell::new(App {
        window: window.clone(),
        minibuf,
        views: vec![text.clone()],
        // TODO: doesn't yet include the initial view's buffer.
        buffers: Vec::new(),
        active_view: text,

        base_keymap: KeyMap::new(),
        minibuf_state: MinibufState::Inactive,
        cur_seq: KeySequence::default(),
    }));

    window.connect_key_press_event(move |_, e| {
        app.borrow_mut().handle_key_press(e)
    });

    window.show_all();
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
