mod draw;
mod event;
mod persistence;

pub use draw::LineHeight;

use crate::buffer::{Buffer, BufferId};
use crate::config::Config;
use crate::pane_tree::PaneTree;
use crate::path_chooser::PathChooser;
use crate::rope::AbsLine;
use crate::search_widget::SearchWidget;
use crate::theme::Theme;
use crate::widget::Widget;
use anyhow::Result;
use glib::clone;
use gtk4::prelude::*;
use gtk4::{self as gtk, gdk, glib};
use persistence::PersistedBuffer;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tracing::{error, info};

#[derive(Clone, Debug, Eq, PartialEq)]
enum InteractiveState {
    Initial,
    Search,
}

enum Overlay {
    OpenFile(PathChooser),
    Search(SearchWidget),
}

impl Overlay {
    fn widget(&self) -> &dyn Widget {
        match self {
            Self::OpenFile(w) => w,
            Self::Search(w) => w,
        }
    }

    fn widget_mut(&mut self) -> &mut dyn Widget {
        match self {
            Self::OpenFile(w) => w,
            Self::Search(w) => w,
        }
    }
}

impl Widget for Overlay {
    fn buffer(&self) -> &Buffer {
        self.widget().buffer()
    }

    fn buffer_mut(&mut self) -> &mut Buffer {
        self.widget_mut().buffer_mut()
    }

    fn recalc_layout(&mut self, width: f64, line_height: LineHeight) {
        self.widget_mut().recalc_layout(width, line_height);
    }
}

pub type BufferMap = HashMap<BufferId, Buffer>;

// Pure state, no GTK stuff goes here.
pub(crate) struct AppState {
    key_handler: event::KeyHandler,

    buffers: HashMap<BufferId, Buffer>,
    pane_tree: PaneTree,

    interactive_state: InteractiveState,
    line_height: LineHeight,

    is_persistence_enabled: bool,

    overlay: Option<Overlay>,
}

impl AppState {
    // TODO: for the persisted data, perhaps we want a trait to abstract
    // that instead of passing the data in.
    fn load(
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
                buffer.set_cursor(pane.id(), Default::default());
                if let Some(cursors) = cursors.get(pane.buffer_id()) {
                    if let Some(pane_cursor) = cursors.get(pane.id()) {
                        buffer.set_cursor(pane.id(), *pane_cursor);
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
            .set_cursor(pane_tree.minibuf().id(), Default::default());

        Self {
            key_handler: event::KeyHandler::new().unwrap(),

            buffers,
            pane_tree,

            interactive_state: InteractiveState::Initial,
            // Outside of tests this is overwritten with a
            // dynamically-calculated value later.
            line_height: LineHeight(20.0),

            is_persistence_enabled: false,
            overlay: None,
        }
    }
}

pub fn init(application: &gtk::Application) {
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

    let persisted_buffers = match AppState::load_persisted_buffers() {
        Ok(pb) => pb,
        Err(err) => {
            error!("failed to load persisted buffers: {}", err);
            Vec::new()
        }
    };

    let pane_tree_json = AppState::load_persisted_pane_tree();

    let mut state = AppState::load(&persisted_buffers, pane_tree_json);
    state.is_persistence_enabled = true;
    let state = Rc::new(RefCell::new(state));

    // Create top-level window.
    let window = gtk::ApplicationWindow::new(application);
    window.set_title(Some("emma"));
    window.set_default_size(800, 800);
    window.maximize();
    window.show();

    // Create single widget that is used for drawing the whole
    // application.
    let widget = gtk::DrawingArea::new();
    widget.set_draw_func(clone!(
        #[strong]
        state,
        move |widget, ctx, width, height| {
            let mut state = state.borrow_mut();
            let width = width as f64;
            let height = height as f64;
            let line_height = state.line_height;

            state.pane_tree.recalc_layout(width, height, line_height);

            // TODO: generalize this somehow.
            if let Some(overlay) = &mut state.overlay {
                overlay.recalc_layout(width, line_height);
            }

            state.draw(
                widget,
                ctx,
                width,
                height,
                line_height,
                &Theme::current(),
            );
        }
    ));
    window.set_child(Some(&widget));

    let key_controller = gtk::EventControllerKey::new();
    key_controller.set_propagation_phase(gtk::PropagationPhase::Capture);
    key_controller.connect_key_pressed(clone!(
        #[strong]
        state,
        #[strong]
        window,
        #[strong]
        widget,
        move |_self, keyval, _keycode, modifiers| {
            // Not every action requires redraw, but most do, no harm
            // occasionally redrawing when not needed.
            widget.queue_draw();

            state.borrow_mut().handle_key_press(
                window.clone(),
                keyval,
                modifiers,
                state.clone(),
            )
        }
    ));
    window.add_controller(key_controller);

    state.borrow_mut().line_height = LineHeight::calculate(&widget);

    // Gtk warns if there's no handler for this signal, so add an empty
    // handler.
    application.connect_activate(|_| {});
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use anyhow::anyhow;

    // TODO: simplify AppState::load, then maybe won't need this anymore.
    pub(crate) fn create_empty_app_state() -> AppState {
        AppState::load(&[], Err(anyhow!("")))
    }

    // TODO: experimenting with gtk test.
    #[gtk::test]
    fn test_app_state() {
        let app_state = create_empty_app_state();

        let panes = app_state.pane_tree.panes();
        assert_eq!(panes.len(), 1);
        assert_eq!(app_state.pane_tree.active().id(), panes[0].id());

        // Scratch buffer and minibuf.
        assert_eq!(app_state.buffers.len(), 2);
        assert!(app_state.buffers.keys().any(|id| id.is_minibuf()));
    }
}
