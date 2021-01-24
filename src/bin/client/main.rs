mod key_map;
mod key_sequence;

use gio::prelude::*;
use gtk::prelude::*;
use key_map::{Action, KeyMap, KeyMapLookup};
use key_sequence::{KeySequence, KeySequenceAtom};
use std::cell::RefCell;
use std::env;
use std::rc::Rc;

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

fn split_view(window: &gtk::ApplicationWindow, orientation: gtk::Orientation) {
    if let Some(focus) = window.get_focus() {
        if let Some(parent) = focus.get_parent() {
            if let Some(layout) = parent.dynamic_cast_ref::<gtk::Box>() {
                let new_view = gtk::TextView::new();

                // Check if the layout is in the correct orientation.
                if layout.get_orientation() == orientation {
                    pack(&layout, &new_view);
                } else {
                    // If there's only the one view in the layout,
                    // just switch the orientation. Otherwise, create
                    // a new layout to subdivide.
                    if layout.get_children().len() == 1 {
                        layout.set_orientation(orientation);
                        pack(&layout, &new_view);
                    } else {
                        let new_layout = make_box(orientation);
                        pack(&new_layout, &new_view);
                        pack(&layout, &new_layout);
                    }
                }
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
            KeyMapLookup::Action(Action::NextView) => {
                todo!("next view");
            }
            KeyMapLookup::Action(Action::SplitHorizontal) => {
                split_view(&window, gtk::Orientation::Horizontal);
            }
            KeyMapLookup::Action(Action::SplitVertical) => {
                split_view(&window, gtk::Orientation::Vertical);
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
