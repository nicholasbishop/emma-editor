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

fn split_view(
    window: &gtk::ApplicationWindow,
    orientation: gtk::Orientation,
    views: &mut Vec<gtk::TextView>,
) {
    // TODO: a more explicit tree structure might make this easier --
    // similar to how we do with the views vec
    if let Some(focus) = window.get_focus() {
        if let Some(parent) = focus.get_parent() {
            if let Some(layout) = parent.dynamic_cast_ref::<gtk::Box>() {
                let new_view = gtk::TextView::new();
                let focus_index =
                    views.iter().position(|e| *e == focus).unwrap();
                views.insert(focus_index + 1, new_view.clone());

                // Check if the layout is in the correct orientation.
                if layout.get_orientation() == orientation {
                    // Get the position of the current focused widget
                    // in its layout so that we can the new widget
                    // right after it.
                    let position =
                        get_widget_index_in_container(layout, &focus).unwrap();

                    pack(&layout, &new_view);
                    layout.reorder_child(&new_view, (position + 1) as i32);
                } else {
                    // If there's only the one view in the layout,
                    // just switch the orientation. Otherwise, create
                    // a new layout to subdivide.
                    if layout.get_children().len() == 1 {
                        layout.set_orientation(orientation);
                        pack(&layout, &new_view);
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
                        pack(&new_layout, &new_view);

                        // Add the new layout to the old layout, and
                        // move it to the right location. TODO: not
                        // sure if there's a better way to do this, or
                        // if the current way is always correct.
                        pack(layout, &new_layout);
                        layout.reorder_child(&new_layout, position as i32);
                    }
                }

                layout.show_all();
                window.set_focus(Some(&new_view));
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

    let view_tree = ViewTree::new();

    let split_root = make_box(gtk::Orientation::Horizontal);
    let text = gtk::TextView::new();
    pack(&split_root, &text);

    let minibuf = gtk::TextView::new();
    minibuf.set_size_request(-1, 26); // TODO

    pack(&layout, &split_root);
    layout.pack_start(&minibuf, false, true, 0);

    window.add(&layout);
    // TODO: use clone macro
    let window2 = window.clone();

    let keymap = KeyMap::new();
    let cur_seq = Rc::new(RefCell::new(KeySequence::default()));

    let views = Rc::new(RefCell::new(Vec::new()));
    views.borrow_mut().push(text);

    window.add_events(gdk::EventMask::KEY_PRESS_MASK);
    window2.connect_key_press_event(move |_, e| {
        // Ignore lone modifier presses.
        if e.get_is_modifier() {
            return Inhibit(false);
        }

        // TODO: we want to ignore combo modifier presses too if no
        // non-modifier key is selected, e.g. pressing alt and then
        // shift, but currently that is treated as a valid
        // sequence. Need to figure out how to prevent that.

        let atom = KeySequenceAtom::from_event(e);
        cur_seq.borrow_mut().0.push(atom);

        let mut clear_seq = true;
        let mut inhibit = true;
        match keymap.lookup(&cur_seq.borrow()) {
            KeyMapLookup::NoEntry => {
                // Allow default handling to occur, e.g. inserting a
                // character into the text widget.
                inhibit = false;
            }
            KeyMapLookup::BadSequence => {
                // TODO: display some kind of non-blocking error
                dbg!("bad seq", cur_seq.borrow());
            }
            KeyMapLookup::Prefix => {
                clear_seq = false;
                // Waiting for the sequence to be completed.
            }
            KeyMapLookup::Action(Action::Exit) => {
                dbg!("close!");
                window.close();
            }
            KeyMapLookup::Action(Action::OpenFile) => {
                dbg!("todo: open file");
            }
            KeyMapLookup::Action(Action::PreviousView) => {
                let views = views.borrow();
                if let Some(focus) = window.get_focus() {
                    let pos = views.iter().position(|e| *e == focus).unwrap();
                    let prev = if pos == 0 { views.len() - 1 } else { pos - 1 };
                    views[prev].grab_focus();
                }
            }
            KeyMapLookup::Action(Action::NextView) => {
                let views = views.borrow();
                if let Some(focus) = window.get_focus() {
                    let pos = views.iter().position(|e| *e == focus).unwrap();
                    let next = if pos == views.len() - 1 { 0 } else { pos + 1 };
                    views[next].grab_focus();
                }
            }
            KeyMapLookup::Action(Action::SplitHorizontal) => {
                view_tree.split(gtk::Orientation::Horizontal);
                split_view(
                    &window,
                    gtk::Orientation::Horizontal,
                    &mut views.borrow_mut(),
                );
            }
            KeyMapLookup::Action(Action::SplitVertical) => {
                split_view(
                    &window,
                    gtk::Orientation::Vertical,
                    &mut views.borrow_mut(),
                );
            }
            KeyMapLookup::Action(Action::CloseView) => {
                todo!();
            }
        };

        if clear_seq {
            cur_seq.borrow_mut().0.clear();
        }

        Inhibit(inhibit)
    });

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
