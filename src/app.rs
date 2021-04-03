mod draw;
mod event;

pub use draw::Font;
use {
    crate::{
        buffer::{Buffer, BufferId},
        pane_tree::PaneTree,
        theme::Theme,
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

    theme: Theme,
}

pub fn init(application: &gtk::Application) {
    // Create single widget that is used for drawing the whole
    // application.
    let widget = gtk::DrawingArea::new();
    widget.set_draw_func(|_widget, ctx, width, height| {
        APP.with(|app| {
            let width = width as f64;
            let height = height as f64;

            let font = Font::new(ctx);

            let mut app = app.borrow_mut();
            let app = app.as_mut().unwrap();

            app.pane_tree.recalc_layout(width, height, &font);
            app.draw(ctx, width, height, &font, &app.theme);
        })
    });

    // Create top-level window.
    let window = gtk::ApplicationWindow::new(application);
    window.set_title(Some("emma"));
    window.set_default_size(800, 800);
    window.set_child(Some(&widget));
    window.show();
    event::create_gtk_key_handler(&window);

    let theme = Theme::load_default().expect("failed to load built-in theme");

    let mut buffers = HashMap::new();

    // TODO: load a temporary buffer
    let buffer_id = BufferId::new();
    let buffer = Buffer::from_path(Path::new("graphemes.txt"), &theme).unwrap();
    buffers.insert(buffer_id.clone(), buffer);

    // Create the minibuf buffer
    let minibuf_buffer_id = BufferId::new();
    buffers.insert(minibuf_buffer_id.clone(), Buffer::create_minibuf(&theme));

    let app = App {
        window,
        widget,

        key_handler: event::KeyHandler::new(),

        buffers,
        pane_tree: PaneTree::new(buffer_id, minibuf_buffer_id),

        theme,
    };

    // Store app in global.
    APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });
}
