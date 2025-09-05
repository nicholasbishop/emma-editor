mod event;
mod persistence;

use crate::draw;
use anyhow::Result;
use emma_app::LineHeight;
use emma_app::buffer::{Buffer, BufferId};
use emma_app::config::Config;
use emma_app::key::{Key, Modifier, Modifiers};
use emma_app::key_map::Action;
use emma_app::message::{Message, create_message_pipe};
use emma_app::overlay::Overlay;
use emma_app::pane_tree::PaneTree;
use emma_app::rope::AbsLine;
use emma_app::theme::Theme;
use emma_app::widget::Widget;
use gtk4::gdk::ModifierType;
use gtk4::glib::{self, ControlFlow, IOCondition, Propagation, clone};
use gtk4::prelude::{
    ApplicationExt, DrawingAreaExtManual, EventControllerExt, GtkWindowExt,
    WidgetExt,
};
use gtk4::{
    Application, ApplicationWindow, CssProvider, DrawingArea,
    EventControllerKey, PropagationPhase, gdk,
};
use persistence::PersistedBuffer;
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use tracing::{error, info};

// Pure state, no GTK stuff goes here.
pub(crate) struct AppState {
    key_handler: event::KeyHandler,

    buffers: HashMap<BufferId, Buffer>,
    pane_tree: PaneTree,

    line_height: LineHeight,

    is_persistence_enabled: bool,

    overlay: Option<Overlay>,
}

impl AppState {
    pub fn buffers(&self) -> &HashMap<BufferId, Buffer> {
        &self.buffers
    }

    pub fn pane_tree(&self) -> &PaneTree {
        &self.pane_tree
    }

    pub fn overlay(&self) -> Option<&Overlay> {
        self.overlay.as_ref()
    }

    // TODO: for the persisted data, perhaps we want a trait to abstract
    // that instead of passing the data in.
    fn load(
        persisted_buffers: &[PersistedBuffer],
        pane_tree_json: Result<String>,
    ) -> Self {
        Theme::set_current(
            Theme::load_default().expect("failed to load built-in theme"),
        );

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
                PaneTree::new(&mut scratch_buffer)
            }
        };

        let scratch_buffer_id = scratch_buffer.id().clone();
        buffers.insert(scratch_buffer_id.clone(), scratch_buffer);

        // Ensure that all the panes are pointing to a valid buffer.
        for pane in pane_tree.panes_mut() {
            if let Some(buffer) = buffers.get_mut(pane.buffer_id()) {
                // Default the cursor to the top of the buffer, then try to
                // restore the proper location from persisted data.
                buffer.set_cursor(pane.id(), Default::default());
                if let Some(cursors) = cursors.get(pane.buffer_id())
                    && let Some(pane_cursor) = cursors.get(pane.id())
                {
                    buffer.set_cursor(pane.id(), *pane_cursor);
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

        Self {
            key_handler: event::KeyHandler::new().unwrap(),

            buffers,
            pane_tree,

            // Outside of tests this is overwritten with a
            // dynamically-calculated value later.
            line_height: LineHeight(20.0),

            is_persistence_enabled: false,
            overlay: None,
        }
    }
}

pub fn init(application: &Application) {
    // TODO: unwrap
    let (mut message_reader, message_writer) = create_message_pipe().unwrap();

    let config = match Config::load() {
        Ok(config) => config,
        Err(err) => {
            // TODO: would be good to show this error in the UI
            error!("failed to load config: {}", err);
            Config::default()
        }
    };

    let css = CssProvider::new();
    css.load_from_data(&format!(
        r#"
        widget {{ 
            font-family: monospace;
            font-size: {font_size}pt;
        }}
    "#,
        font_size = config.font_size
    ));
    gtk4::style_context_add_provider_for_display(
        &gdk::Display::default().unwrap(),
        &css,
        gtk4::STYLE_PROVIDER_PRIORITY_APPLICATION,
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
    let window = ApplicationWindow::new(application);
    window.set_title(Some("emma"));
    window.set_default_size(800, 800);
    window.maximize();
    window.show();

    // Create single widget that is used for drawing the whole
    // application.
    let widget = DrawingArea::new();
    widget.set_draw_func(clone!(
        #[strong]
        state,
        move |widget, ctx, width, height| {
            let mut state = state.borrow_mut();
            let width = width as f64;
            let height = height as f64;
            let line_height = state.line_height;

            state.pane_tree.recalc_layout(width, height);

            // TODO: generalize this somehow.
            if let Some(overlay) = &mut state.overlay {
                overlay.recalc_layout(width, line_height);
            }

            draw::draw(
                &state,
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

    let message_writer_2 = message_writer.try_clone().unwrap();

    let key_controller = EventControllerKey::new();
    key_controller.set_propagation_phase(PropagationPhase::Capture);
    key_controller.connect_key_pressed(clone!(
        #[strong]
        state,
        #[strong]
        widget,
        move |_self, keyval, _keycode, modifiers| {
            // Not every action requires redraw, but most do, no harm
            // occasionally redrawing when not needed.
            widget.queue_draw();

            state.borrow_mut().handle_key_press(
                key_from_gdk(keyval),
                modifiers_from_gdk(modifiers),
                &message_writer,
            );

            Propagation::Stop
        }
    ));
    window.add_controller(key_controller);

    let _source_id = glib::source::unix_fd_add_local(
        message_reader.as_raw_fd(),
        IOCondition::IN,
        clone!(
            #[strong]
            state,
            move |_raw_fd, _condition| {
                // Read from the FD until we can't (with some
                // kind of stopping point, in case the FD keeps
                // returning a flood of data?)

                // TODO: unwraps
                let msg = message_reader.read().unwrap();

                match msg {
                    Message::Close => window.close(),
                    Message::AppendToBuffer(buf_id, content) => {
                        state
                            .borrow_mut()
                            .handle_action(
                                Action::AppendToBuffer(buf_id, content),
                                &message_writer_2,
                            )
                            .unwrap();
                    }
                }

                // Keep the callback.
                ControlFlow::Continue
            }
        ),
    );

    state.borrow_mut().line_height = draw::calculate_line_height(&widget);

    // Gtk warns if there's no handler for this signal, so add an empty
    // handler.
    application.connect_activate(|_| {});
}

fn key_from_gdk(key: gtk4::gdk::Key) -> Key {
    use gtk4::gdk::Key as GKey;
    match key {
        GKey::BackSpace => Key::Backspace,
        GKey::Escape => Key::Escape,
        GKey::greater => Key::Greater,
        GKey::less => Key::Less,
        GKey::plus => Key::Plus,
        GKey::Return => Key::Return,
        GKey::space => Key::Space,

        GKey::Alt_L | GKey::Alt_R => Key::Modifier(Modifier::Alt),
        GKey::Control_L | GKey::Control_R => Key::Modifier(Modifier::Control),
        GKey::Shift_L | GKey::Shift_R => Key::Modifier(Modifier::Shift),

        _ => {
            if let Some(c) = key.to_unicode() {
                Key::Char(c)
            } else {
                todo!("unhandled key: {key}")
            }
        }
    }
}

fn modifiers_from_gdk(modifiers: gtk4::gdk::ModifierType) -> Modifiers {
    Modifiers {
        alt: modifiers.contains(ModifierType::ALT_MASK),
        control: modifiers.contains(ModifierType::CONTROL_MASK),
        shift: modifiers.contains(ModifierType::SHIFT_MASK),
    }
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
    #[gtk4::test]
    fn test_app_state() {
        let app_state = create_empty_app_state();

        let panes = app_state.pane_tree.panes();
        assert_eq!(panes.len(), 1);
        assert_eq!(app_state.pane_tree.active().id(), panes[0].id());

        // Scratch buffer.
        assert_eq!(app_state.buffers.len(), 1);
    }
}
