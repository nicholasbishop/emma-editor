mod draw;
mod event;
mod persistence;

pub use draw::LineHeight;

use crate::buffer::{Buffer, BufferId};
use crate::config::Config;
use crate::open_file::OpenFile;
use crate::pane_tree::PaneTree;
use crate::rope::AbsLine;
use crate::theme::Theme;
use anyhow::Result;
use gtk4::prelude::*;
use gtk4::{self as gtk, gdk};
use persistence::PersistedBuffer;
use relm4::abstractions::DrawHandler;
use relm4::{Component, ComponentParts, ComponentSender};
use std::collections::HashMap;
use std::path::PathBuf;
use tracing::{error, info};

#[derive(Clone, Debug, Eq, PartialEq)]
enum InteractiveState {
    Initial,
    #[allow(unused)] // TODO
    OpenFile(
        /// Default path.
        PathBuf,
    ),
    Search,
}

pub type BufferMap = HashMap<BufferId, Buffer>;

// Pure state, no GTK stuff goes here.
pub(crate) struct App {
    key_handler: event::KeyHandler,

    buffers: HashMap<BufferId, Buffer>,
    pane_tree: PaneTree,

    interactive_state: InteractiveState,
    line_height: LineHeight,

    is_persistence_enabled: bool,

    // TODO: maybe an enum for the interactive overlay widgets?
    open_file: Option<OpenFile>,

    draw_handler: DrawHandler,
}

impl App {
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

            is_persistence_enabled: false,
            open_file: None,

            draw_handler: DrawHandler::new(),
        }
    }
}

#[derive(Debug)]
pub enum AppEvent {
    Resized,
    KeyPressed(gdk::Key, gdk::ModifierType),
}

#[relm4::component(pub)]
impl Component for App {
    type CommandOutput = ();
    type Input = AppEvent;
    type Output = ();
    type Init = ();

    view! {
        gtk::Window {
            set_title: Some("emma"),
            set_default_width: 800,
            set_default_height: 800,

            add_controller = gtk::EventControllerKey {
                connect_key_pressed[sender] => move |controller, keyval, _keycode, state| {
                    controller.set_propagation_phase(gtk::PropagationPhase::Capture);
                    sender.input(AppEvent::KeyPressed(keyval, state));
                    gtk::glib::signal::Propagation::Stop
                }
            },

            #[local_ref]
            area -> gtk::DrawingArea {
                connect_resize[sender] => move |_, _, _| {
                    sender.input(AppEvent::Resized);
                }
            },
        },
    }

    /// Initialize the UI and model.
    fn init(
        _data: Self::Init,
        window: Self::Root,
        sender: ComponentSender<Self>,
    ) -> relm4::ComponentParts<Self> {
        window.maximize();

        let config = match Config::load() {
            Ok(config) => config,
            Err(err) => {
                // TODO: would be good to show this error in the UI
                error!("failed to load config: {}", err);
                Config::default()
            }
        };

        let css = format!(
            r#"
            * {{ 
              font-family: monospace;
              font-size: {font_size}pt;
            }}
            "#,
            font_size = config.font_size
        );
        relm4::set_global_css(&css);

        let persisted_buffers = match App::load_persisted_buffers() {
            Ok(pb) => pb,
            Err(err) => {
                error!("failed to load persisted buffers: {}", err);
                Vec::new()
            }
        };

        let pane_tree_json = App::load_persisted_pane_tree();

        let mut model =
            App::load(LineHeight(0.0), &persisted_buffers, pane_tree_json);

        let area = model.draw_handler.drawing_area();
        let widgets = view_output!();

        model.line_height = LineHeight::calculate(&window);

        ComponentParts { model, widgets }
    }

    fn update(
        &mut self,
        msg: AppEvent,
        _sender: ComponentSender<Self>,
        root: &Self::Root,
    ) {
        match msg {
            AppEvent::KeyPressed(key, state) => {
                // TODO: remove Propagation return?
                self.handle_key_press(root, key, state);
            }
            AppEvent::Resized => {}
        }

        // Draw:

        let ctx = self.draw_handler.get_context();

        let width = root.width() as f64;
        let height = root.height() as f64;

        self.pane_tree
            .recalc_layout(width, height, self.line_height);

        // TODO: generalize this somehow.
        if let Some(open_file) = &mut self.open_file {
            open_file.recalc_layout(width, height, self.line_height);
        }

        self.draw(
            root,
            &ctx,
            width,
            height,
            self.line_height,
            &Theme::current(),
        );
    }
}

#[cfg(any())]
pub fn init(application: &gtk::Application) {
    // Create single widget that is used for drawing the whole
    // application.
    let widget = gtk::DrawingArea::new();

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

    let persisted_buffers = match App::load_persisted_buffers() {
        Ok(pb) => pb,
        Err(err) => {
            error!("failed to load persisted buffers: {}", err);
            Vec::new()
        }
    };

    let pane_tree_json = App::load_persisted_pane_tree();

    let line_height = LineHeight::calculate(&widget);

    let mut app = App {
        window,
        widget,

        state: App::load(line_height, &persisted_buffers, pane_tree_json),
    };
    app.state.is_persistence_enabled = true;

    // Gtk warns if there's no handler for this signal, so add an empty
    // handler.
    application.connect_activate(|_| {});

    // Store app in global.
    APP.with(|cell| {
        *cell.borrow_mut() = Some(app);
    });
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use anyhow::anyhow;

    // TODO: simplify App::load, then maybe won't need this anymore.
    pub(crate) fn create_empty_app_state() -> App {
        App::load(LineHeight(12.0), &[], Err(anyhow!("")))
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
