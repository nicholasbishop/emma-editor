use {
    crate::buffer::Buffer,
    gtk4::{
        prelude::*, Adjustment, Box, Label, Orientation, ScrolledWindow,
        TextView, Widget,
    },
};

type View = TextView;

#[derive(Clone, Eq, PartialEq)]
pub struct Pane {
    container: Box,

    info: Label,

    scrolled_window: ScrolledWindow,
    view: View,
}

impl Pane {
    pub fn new() -> Pane {
        let view = View::new();
        view.set_monospace(true);
        let adj: Option<&Adjustment> = None;
        let scrolled_window = ScrolledWindow::new(adj, adj);
        scrolled_window.add(&view);

        let info = Label::new(Some("TODO"));
        info.set_widget_name("info");
        info.set_xalign(0.0);

        let container = Box::new(Orientation::Vertical, 0);
        let expand = true;
        let fill = true;
        let padding = 0;
        container.pack_start(&scrolled_window, expand, fill, padding);
        let expand = false;
        container.pack_start(&info, expand, fill, padding);

        Pane {
            container,
            info,
            scrolled_window,
            view,
        }
    }

    pub fn get_widget(&self) -> Widget {
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
    }
}
