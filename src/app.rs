mod draw;
mod event;
mod persistence;

pub use draw::LineHeight;

use crate::buffer::{Buffer, BufferId};
use crate::config::Config;
use crate::pane_tree::PaneTree;
use crate::rope::AbsLine;
use crate::theme::Theme;
use gtk4::prelude::*;
use gtk4::{self as gtk, gdk};
use std::cell::RefCell;
use std::collections::HashMap;
use tracing::{error, info};

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
    window.maximize();

    let config = match Config::load() {
        Ok(config) => config,
        Err(err) => {
            // TODO: would be good to show this error in the UI
            error!("failed to load config: {}", err);
            Config::default()
        }
    };

    let css = gtk::CssProvider::new();
    css.load_from_data(&format!(
        r#"
        widget {{ 
            font-family: monospace;
            font-size: {font_size}pt;
        }}
    "#,
        font_size = config.font_size
    ));
    gtk::style_context_add_provider_for_display(
        &gdk::Display::default().unwrap(),
        &css,
        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    );

    window.show();
    event::create_gtk_key_handler(&window);

    Theme::set_current(
        Theme::load_default().expect("failed to load built-in theme"),
    );

    // Create the minibuf buffer
    let mut minibuf = Buffer::create_minibuf();

    // Always create an empty scratch buffer.
    let mut scratch_buffer = Buffer::create_empty();

    let mut buffers = HashMap::new();
    let mut cursors = HashMap::new();
    match App::load_persisted_buffers() {
        Ok(pb) => {
            for pb in pb {
                info!("loading {:?}", pb);
                cursors.insert(pb.buffer_id.clone(), pb.cursors);
                // TODO; handle no path cases as well.
                if let Some(path) = pb.path {
                    buffers.insert(
                        pb.buffer_id,
                        Buffer::from_path(&path).unwrap(),
                    );
                }
            }
        }
        Err(err) => {
            error!("failed to load persisted buffers: {}", err);
        }
    };

    let mut pane_tree = match App::load_pane_tree() {
        Ok(pt) => pt,
        Err(err) => {
            error!("failed to load persisted pane tree: {}", err);
            PaneTree::new(&mut scratch_buffer, &mut minibuf)
        }
    };
    pane_tree.cleanup_after_load();

    let minibuf_id = minibuf.id().clone();
    let scratch_buffer_id = scratch_buffer.id().clone();
    buffers.insert(minibuf_id.clone(), minibuf);
    buffers.insert(scratch_buffer_id.clone(), scratch_buffer);

    // Ensure that all the panes are pointing to a valid buffer.
    for pane in pane_tree.panes_mut() {
        if let Some(buffer) = buffers.get_mut(pane.buffer_id()) {
            // Default the cursor to the top of the buffer, then try to
            // restore the proper location from persisted data.
            buffer.set_cursor(pane, Default::default());
            if let Some(cursors) = cursors.get(pane.buffer_id()) {
                if let Some(pane_cursor) = cursors.get(pane.id()) {
                    buffer.set_cursor(pane, *pane_cursor);
                }
            }
        } else {
            pane.switch_buffer(&mut buffers, &scratch_buffer_id);
        }

        // Ensure that the pane's top-line is valid.
        let buffer = buffers.get(pane.buffer_id()).unwrap();
        if pane.top_line() >= AbsLine(buffer.text().len_lines()) {
            pane.set_top_line(AbsLine(0));
        }
    }
    buffers
        .get_mut(&minibuf_id)
        .unwrap()
        .set_cursor(pane_tree.minibuf(), Default::default());

    let line_height = LineHeight::calculate(&widget);

    let app = App {
        window,
        widget,

        key_handler: event::KeyHandler::new().unwrap(),

        buffers,
        pane_tree,

        interactive_state: InteractiveState::Initial,
        line_height,
    };

    application.connect_activate(|_| {
        APP.with(|app| {
            let mut app = app.borrow_mut();
            let app = app.as_mut().unwrap();
            // TODO: the idea is to bring the app window to the front
            // here, but it doesn't seem to work.
            app.window.present();
        })
    });

    // Store app in global.
    APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });
}
