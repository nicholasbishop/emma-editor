use {
    crate::buffer::Buffer,
    gtk4::{self as gtk, prelude::*},
};

type View = gtk::TextView;

#[derive(Clone, Eq, PartialEq)]
pub struct Pane {
    container: gtk::Box,

    info: gtk::Label,

    scrolled_window: gtk::ScrolledWindow,
    pub view: View,
}

impl Pane {
    pub fn new() -> Pane {
        let view = View::new();
        view.set_monospace(true);
        let scrolled_window = gtk::ScrolledWindow::new();
        scrolled_window.set_child(Some(&view));
        scrolled_window.set_vexpand(true);

        let info = gtk::Label::new(Some("TODO"));
        info.set_widget_name("info");
        info.set_xalign(0.0);

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.append(&scrolled_window);
        container.append(&info);

        Pane {
            container,
            info,
            scrolled_window,
            view,
        }
    }

    pub fn get_widget(&self) -> gtk::Widget {
        self.container.clone().upcast()
    }

    // TODO: might want to remove this and just stick to a concept of
    // "active" pane
    pub fn has_focus(&self) -> bool {
        self.view.has_focus()
    }

    pub fn grab_focus(&self) {
        self.view.grab_focus();
    }

    pub fn set_buffer(&self, buffer: &Buffer) {
        self.view.set_buffer(Some(buffer));
        self.update_info();
    }

    fn update_info(&self) {
        // TODO
    }
}
