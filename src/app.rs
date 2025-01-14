mod draw;
mod event;
mod persistence;

pub use draw::LineHeight;

use crate::buffer::{Buffer, BufferId};
use crate::config::Config;
use crate::pane_tree::PaneTree;
use crate::rope::AbsLine;
use crate::theme::Theme;
use anyhow::Result;
use gtk4::prelude::*;
use gtk4::{self as gtk, gdk};
use persistence::PersistedBuffer;
use std::cell::RefCell;
use std::collections::HashMap;
use tracing::{error, info};

// This global is needed for callbacks on the main thread. On other
// threads it is None.
std::thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum InteractiveState {
    Initial,
    OpenFile,
    Search,
}

pub type BufferMap = HashMap<BufferId, Buffer>;

// Pure state, no GTK stuff goes here.
struct AppState {
    key_handler: event::KeyHandler,

    buffers: HashMap<BufferId, Buffer>,
    pane_tree: PaneTree,

    interactive_state: InteractiveState,
    line_height: LineHeight,
}

impl AppState {
    // TODO: for the persisted data, perhaps we want a trait to abstract
    // that instead of passing the data in.
    fn load(
        line_height: LineHeight,
        persisted_buffers: &[PersistedBuffer],
        pane_tree_json: Result<String>,
    ) -> Self {
        Theme::set_current(
            Theme::load_default().expect("failed to load built-in theme"),
        );

        // Create the minibuf buffer
        let mut minibuf = Buffer::create_minibuf();

        // Always create an empty scratch buffer.
        let mut scratch_buffer = Buffer::create_empty();

        let mut buffers = HashMap::new();
        let mut cursors = HashMap::new();
        for pb in persisted_buffers {
            info!("loading {:?}", pb);
            cursors.insert(pb.buffer_id.clone(), pb.cursors.clone());
            // TODO; handle no path cases as well.
            if let Some(path) = &pb.path {
                buffers.insert(
                    pb.buffer_id.clone(),
                    Buffer::from_path(path).unwrap(),
                );
            }
        }

        let mut pane_tree = match pane_tree_json
            .and_then(|json| PaneTree::load_from_json(&json))
        {
            Ok(pt) => pt,
            Err(err) => {
                error!("failed to load persisted pane tree: {}", err);
                PaneTree::new(&mut scratch_buffer, &mut minibuf)
            }
        };

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

        Self {
            key_handler: event::KeyHandler::new().unwrap(),

            buffers,
            pane_tree,

            interactive_state: InteractiveState::Initial,
            line_height,
        }
    }
}

struct App {
    window: gtk::ApplicationWindow,
    widget: gtk::DrawingArea,

    state: AppState,
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

            app.state.pane_tree.recalc_layout(
                width,
                height,
                app.state.line_height,
            );
            app.state.draw(
                &app.widget,
                ctx,
                width,
                height,
                app.state.line_height,
                &Theme::current(),
            );
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

    let persisted_buffers = match AppState::load_persisted_buffers() {
        Ok(pb) => pb,
        Err(err) => {
            error!("failed to load persisted buffers: {}", err);
            Vec::new()
        }
    };

    let pane_tree_json = AppState::load_persisted_pane_tree();

    let line_height = LineHeight::calculate(&widget);

    let app = App {
        window,
        widget,

        state: AppState::load(line_height, &persisted_buffers, pane_tree_json),
    };

    // Gtk warns if there's no handler for this signal, so add an empty
    // handler.
    application.connect_activate(|_| {});

    // Store app in global.
    APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use anyhow::anyhow;

    // TODO: experimenting with gtk test.
    #[gtk::test]
    fn test_app_state() {
        let app_state = AppState::load(LineHeight(12.0), &[], Err(anyhow!("")));

        let panes = app_state.pane_tree.panes();
        assert_eq!(panes.len(), 1);
        assert_eq!(app_state.pane_tree.active().id(), panes[0].id());

        // Scratch buffer and minibuf.
        assert_eq!(app_state.buffers.len(), 2);
        assert!(app_state.buffers.keys().any(|id| id.is_minibuf()));
    }
}
