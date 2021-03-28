use {
    crate::{
        buffer::Buffer,
        draw,
        key_map::{Action, KeyMap, KeyMapLookup, KeyMapStack},
        key_sequence::{is_modifier, KeySequence, KeySequenceAtom},
    },
    gtk4::{self as gtk, gdk, glib::signal::Inhibit, prelude::*},
    std::{cell::RefCell, path::Path},
};

// This global is needed for callbacks on the main thread. On other
// threads it is None.
std::thread_local! {
    static APP: RefCell<Option<App>> = RefCell::new(None);
}

pub struct App {
    window: gtk::ApplicationWindow,
    widget: gtk::DrawingArea,

    base_keymap: KeyMap,
    cur_seq: KeySequence,

    buffers: Vec<Buffer>,
}

impl App {
    fn handle_key_press(
        &mut self,
        key: gdk::keys::Key,
        state: gdk::ModifierType,
    ) -> Inhibit {
        let mut keymap_stack = KeyMapStack::default();
        keymap_stack.push(self.base_keymap.clone());

        // Ignore lone modifier presses.
        if is_modifier(&key) {
            return Inhibit(false);
        }

        // TODO: we want to ignore combo modifier presses too if no
        // non-modifier key is selected, e.g. pressing alt and then
        // shift, but currently that is treated as a valid
        // sequence. Need to figure out how to prevent that.

        let atom = KeySequenceAtom::from_event(key.clone(), state);
        self.cur_seq.0.push(atom);

        let mut clear_seq = true;
        let inhibit = Inhibit(true);
        match keymap_stack.lookup(&self.cur_seq) {
            KeyMapLookup::BadSequence => {
                // TODO: display some kind of non-blocking error
                dbg!("bad seq", &self.cur_seq);
            }
            KeyMapLookup::Prefix => {
                clear_seq = false;
                // Waiting for the sequence to be completed.
            }
            KeyMapLookup::Action(Action::Exit) => {
                self.window.close();
            }
            _ => {
                todo!();
            }
        }

        if clear_seq {
            self.cur_seq.0.clear();
        }

        inhibit
    }
}

fn create_keyboard_input_handler(window: &gtk::ApplicationWindow) {
    let key_controller = gtk::EventControllerKey::new();
    key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    key_controller.connect_key_pressed(|_self, keyval, _keycode, state| {
        APP.with(|app| {
            app.borrow_mut()
                .as_mut()
                .unwrap()
                .handle_key_press(keyval, state)
        })
    });
    window.add_controller(&key_controller);
}

pub fn init(application: &gtk::Application) {
    // Create single widget that is used for drawing the whole
    // application.
    let widget = gtk::DrawingArea::new();
    widget.set_draw_func(|_widget, ctx, width, height| {
        APP.with(|app| {
            draw::draw(app.borrow().as_ref().unwrap(), ctx, width, height);
        })
    });

    // Create top-level window.
    let window = gtk::ApplicationWindow::new(application);
    window.set_title(Some("emma"));
    window.set_default_size(800, 800);
    window.set_child(Some(&widget));
    window.show();
    create_keyboard_input_handler(&window);

    // TODO: load a temporary buffer
    let buffer = Buffer::from_path(Path::new("src/app.rs")).unwrap();

    let app = App {
        window,
        widget,

        base_keymap: KeyMap::new(),
        cur_seq: KeySequence::default(),

        buffers: vec![buffer],
    };

    // Store app in global.
    APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });
}
