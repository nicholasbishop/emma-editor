use {
    crate::{buffer::Buffer, draw},
    gtk4::{self as gtk, cairo, gdk, glib::signal::Inhibit, prelude::*},
    parking_lot::RwLock,
    std::{cell::RefCell, sync::Arc},
};

// This global is needed for callbacks on the main thread. On other
// threads it is None.
std::thread_local! {
    static APP: RefCell<Option<App>> = RefCell::new(None);
}

pub struct App {
    window: gtk::ApplicationWindow,
    widget: gtk::DrawingArea,
    buffers: Vec<Arc<RwLock<Buffer>>>,
}

impl App {
    fn draw(&self, ctx: &cairo::Context, width: i32, height: i32) {
        draw::draw(self, ctx, width, height);
    }

    fn handle_key_press(
        &mut self,
        key: gdk::keys::Key,
        state: gdk::ModifierType,
    ) -> Inhibit {
        todo!();
    }
}

pub fn init(application: &gtk::Application) {
    // Create single widget that is used for drawing the whole
    // application.
    let widget = gtk::DrawingArea::new();
    widget.set_draw_func(|_widget, ctx, width, height| {
        APP.with(|app| {
            app.borrow_mut().as_mut().unwrap().draw(ctx, width, height)
        })
    });

    // Create top-level window.
    let window = gtk::ApplicationWindow::new(application);
    window.set_title(Some("emma"));
    window.set_default_size(800, 800);
    window.set_child(Some(&widget));
    window.show();

    // Add keyboard input handler.
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

    let app = App {
        window,
        widget,
        buffers: Vec::new(),
    };

    // Store app in global.
    APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });
}
