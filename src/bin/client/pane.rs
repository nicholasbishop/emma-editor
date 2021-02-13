use {
    crate::buffer::EmBuf,
    gtk4::{self as gtk, prelude::*},
};

type View = gtk::TextView;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Pane {
    container: gtk::Box,

    info: gtk::Label,

    scrolled_window: gtk::ScrolledWindow,
    pub view: View,

    pub embuf: EmBuf,
}

impl Pane {
    pub fn new(embuf: &EmBuf) -> Pane {
        let view = View::new();
        view.set_monospace(true);
        let scrolled_window = gtk::ScrolledWindow::new();
        scrolled_window.set_child(Some(&view));
        scrolled_window.set_vexpand(true);

        let info = gtk::Label::new(None);
        info.set_widget_name("info");
        info.set_xalign(0.0);

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.append(&scrolled_window);
        container.append(&info);

        let pane = Pane {
            container,
            info,
            scrolled_window,
            view,
            embuf: embuf.clone(),
        };
        pane.set_buffer(embuf);
        pane
    }

    pub fn get_widget(&self) -> gtk::Widget {
        self.container.clone().upcast()
    }

    pub fn set_buffer(&self, embuf: &EmBuf) {
        self.view.set_buffer(Some(&embuf.storage()));
        self.update_info();
    }

    fn update_info(&self) {
        // TODO
        self.info
            .set_text(&format!("{}", self.embuf.path().display()));
    }
}
