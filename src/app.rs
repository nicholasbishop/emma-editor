mod draw;
mod event;
mod persistence;

pub use draw::LineHeight;

use crate::buffer::{Buffer, BufferId};
use crate::config::Config;
use crate::pane_tree::PaneTree;
use crate::theme::Theme;
use gtk4::prelude::*;
use gtk4::{self as gtk, gdk};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::Path;
use tracing::error;

// This global is needed for callbacks on the main thread. On other
// threads it is None.
std::thread_local! {
    static APP: RefCell<Option<App>> = RefCell::new(None);
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InteractiveState {
    Initial,
    OpenFile,
    Search,
}

pub type BufferMap = HashMap<BufferId, Buffer>;

struct App {
    window: gtk::ApplicationWindow,
    widget: gtk::DrawingArea,

    key_handler: event::KeyHandler,

    buffers: HashMap<BufferId, Buffer>,
    pane_tree: PaneTree,

    interactive_state: InteractiveState,
    line_height: LineHeight,
}

pub fn init(application: &gtk::Application) {
    // Create single widget that is used for drawing the whole
    // application.
    let widget = gtk::DrawingArea::new();
    widget.set_draw_func(|_widget, ctx, width, height| {
        APP.with(|app| {
            let width = width as f64;
            let height = height as f64;

            let mut app = app.borrow_mut();
            let app = app.as_mut().unwrap();

            app.pane_tree.recalc_layout(width, height, app.line_height);
            app.draw(ctx, width, height, app.line_height, &Theme::current());
        })
    });

    // Create top-level window.
    let window = gtk::ApplicationWindow::new(application);
    window.set_title(Some("emma"));
    window.set_default_size(800, 800);
    window.set_child(Some(&widget));

    let config = match Config::load() {
        Ok(config) => config,
        Err(err) => {
            // TODO: would be good to show this error in the UI
            error!("failed to load config: {}", err);
            Config::default()
        }
    };

    let css = gtk::CssProvider::new();
    css.load_from_data(
        format!(
            r#"
        widget {{ 
            font-family: monospace;
            font-size: {font_size}pt;
        }}
    "#,
            font_size = config.font_size
        )
        .as_bytes(),
    );
    gtk::StyleContext::add_provider_for_display(
        &gdk::Display::default().unwrap(),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    window.show();
    event::create_gtk_key_handler(&window);

    Theme::set_current(
        Theme::load_default().expect("failed to load built-in theme"),
    );

    // TODO: load a temporary buffer
    let mut scratch_buffer =
        Buffer::from_path(Path::new("src/app.rs")).unwrap();

    // Create the minibuf buffer
    let mut minibuf = Buffer::create_minibuf();

    let line_height = LineHeight::calculate(&widget);

    let mut app = App {
        window,
        widget,

        key_handler: event::KeyHandler::new().unwrap(),

        buffers: HashMap::new(),
        pane_tree: PaneTree::new(&mut scratch_buffer, &mut minibuf),

        interactive_state: InteractiveState::Initial,
        line_height,
    };

    let mut buffers = HashMap::new();
    buffers.insert(scratch_buffer.id().clone(), scratch_buffer);
    buffers.insert(minibuf.id().clone(), minibuf);
    app.buffers = buffers;

    if let Err(err) = app.persistence_load() {
        error!("failed to load persistent data: {}", err);
    }

    // Store app in global.
    APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });
}
