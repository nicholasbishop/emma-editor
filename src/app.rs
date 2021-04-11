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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InteractiveState {
    Initial,
    OpenFile,
}

pub type BufferMap = HashMap<BufferId, Buffer>;

struct App {
    window: gtk::ApplicationWindow,
    widget: gtk::DrawingArea,

    key_handler: event::KeyHandler,

    buffers: HashMap<BufferId, Buffer>,
    pane_tree: PaneTree,

    theme: Theme,
    interactive_state: InteractiveState,
}

pub fn init(application: &gtk::Application) {
    // Create single widget that is used for drawing the whole
    // application.
    let widget = gtk::DrawingArea::new();
    widget.set_draw_func(|widget, ctx, width, height| {
        APP.with(|app| {
            let width = width as f64;
            let height = height as f64;

            let font = Font::new(widget.get_pango_context());

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

    // TODO: load a temporary buffer
    let mut scratch_buffer =
        Buffer::from_path(Path::new("src/app.rs"), &theme).unwrap();

    // Create the minibuf buffer
    let mut minibuf = Buffer::create_minibuf(&theme);

    let mut app = App {
        window,
        widget,

        key_handler: event::KeyHandler::new(),

        buffers: HashMap::new(),
        pane_tree: PaneTree::new(&mut scratch_buffer, &mut minibuf),

        theme,
        interactive_state: InteractiveState::Initial,
    };

    let mut buffers = HashMap::new();
    buffers.insert(scratch_buffer.id().clone(), scratch_buffer);
    buffers.insert(minibuf.id().clone(), minibuf);
    app.buffers = buffers;

    // Store app in global.
    APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });
}
