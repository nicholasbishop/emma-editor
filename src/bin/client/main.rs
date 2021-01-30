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

fn print_widget_tree_impl<W: IsA<gtk::Widget>>(root: &W, depth: usize) {
    // Indent
    let mut line = String::new();
    for _ in 0..depth {
        line.push_str("  ");
    }

    let root_type = root.get_type().to_string();
    let root_name = root.get_widget_name().to_string();
    line.push_str(&root_type);

    // The default name is just the type, so skip it if it is that
    if root_name != root_type {
        line.push_str(" - ");
        line.push_str(&root_name);
    }

    line.push_str(&format!(" (refcount={})", root.ref_count()));

    println!("{}", line);

    // Print children
    if let Some(container) = root.dynamic_cast_ref::<gtk::Container>() {
        for child in container.get_children() {
            print_widget_tree_impl(&child, depth + 1);
        }
    }
}

fn print_widget_tree<W: IsA<gtk::Widget>>(root: &W, msg: &str) {
    println!("{}", msg);
    print_widget_tree_impl(root, 0);
    println!();
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

    let keymap = KeyMap::new();
    let cur_seq = Rc::new(RefCell::new(KeySequence::default()));

    // TODO replace this with the tree
    //let views:  = Rc::new(RefCell::new(Vec::new()));
    //views.borrow_mut().push(text);

    window.add_events(gdk::EventMask::KEY_PRESS_MASK);
    window2.connect_key_press_event(move |_, e| {
        let split = |orientation| {
            // view_tree.split(orientation);
            view_tree_container.remove(&view_tree_container.get_children()[0]);
            print_widget_tree(&window, "after remove");

            pack(&view_tree_container, &view_tree.render());
            //pack(&view_tree_container, &gtk::TextView::new());
            print_widget_tree(&window, "after add");

            view_tree_container.show_all();
            print_widget_tree(&window, "after show");
        };

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
                todo!();
                // let views = views.borrow();
                // if let Some(focus) = window.get_focus() {
                //     let pos = views.iter().position(|e| *e == focus).unwrap();
                //     let prev = if pos == 0 { views.len() - 1 } else { pos - 1 };
                //     views[prev].grab_focus();
                // }
            }
            KeyMapLookup::Action(Action::NextView) => {
                todo!();
                // let views = views.borrow();
                // if let Some(focus) = window.get_focus() {
                //     let pos = views.iter().position(|e| *e == focus).unwrap();
                //     let next = if pos == views.len() - 1 { 0 } else { pos + 1 };
                //     views[next].grab_focus();
                // }
            }
            KeyMapLookup::Action(Action::SplitHorizontal) => {
                split(gtk::Orientation::Horizontal);
            }
            KeyMapLookup::Action(Action::SplitVertical) => {
                split(gtk::Orientation::Vertical);
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
