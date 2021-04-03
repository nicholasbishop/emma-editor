mod draw;
mod event;

use {
    crate::{
        buffer::{Buffer, BufferId},
        pane_tree::PaneTree,
    },
    gtk4::{self as gtk, prelude::*},
    std::{cell::RefCell, collections::HashMap, path::Path},
};

// This global is needed for callbacks on the main thread. On other
// threads it is None.
std::thread_local! {
    static APP: RefCell<Option<App>> = RefCell::new(None);
}

pub struct App {
    window: gtk::ApplicationWindow,
    widget: gtk::DrawingArea,

    key_handler: event::KeyHandler,

    buffers: HashMap<BufferId, Buffer>,
    pane_tree: PaneTree,
}

pub fn init(application: &gtk::Application) {
    // Create single widget that is used for drawing the whole
    // application.
    let widget = gtk::DrawingArea::new();
    widget.set_draw_func(|_widget, ctx, width, height| {
        APP.with(|app| {
            let width = width as f64;
            let height = height as f64;
            app.borrow_mut()
                .as_mut()
                .unwrap()
                .pane_tree
                .recalc_layout(width, height);
            app.borrow().as_ref().unwrap().draw(ctx, width, height);
        })
    });

    // Create top-level window.
    let window = gtk::ApplicationWindow::new(application);
    window.set_title(Some("emma"));
    window.set_default_size(800, 800);
    window.set_child(Some(&widget));
    window.show();
    event::create_gtk_key_handler(&window);

    // TODO: load a temporary buffer
    let buffer_id = BufferId::new();
    let buffer = Buffer::from_path(Path::new("graphemes.txt")).unwrap();
    let mut buffers = HashMap::new();
    buffers.insert(buffer_id.clone(), buffer);

    let app = App {
        window,
        widget,

        key_handler: event::KeyHandler::new(),

        buffers,
        pane_tree: PaneTree::new(buffer_id),
    };

    // Store app in global.
    APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });
}
