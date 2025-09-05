use crate::draw;
use emma_app::config::Config;
use emma_app::key::{Key, Modifier, Modifiers};
use emma_app::key_map::Action;
use emma_app::message::{Message, create_message_pipe};
use emma_app::state::AppState;
use emma_app::theme::Theme;
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
use std::cell::RefCell;
use std::rc::Rc;
use tracing::error;

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
    state.enable_persistence();
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
            let line_height = state.line_height();

            state.recalc_layout(width, height);

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

    state
        .borrow_mut()
        .set_line_height(draw::calculate_line_height(&widget));

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
