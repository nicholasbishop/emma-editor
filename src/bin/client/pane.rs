use gtk::prelude::*;

type View = gtk::TextView;

#[derive(Clone, Eq, PartialEq)]
pub struct Pane {
    container: gtk::Box,

    info: gtk::Label,

    scrolled_window: gtk::ScrolledWindow,
    view: View,
}

impl Pane {
    pub fn new() -> Pane {
        let view = View::new();
        view.set_monospace(true);
        let adj: Option<&gtk::Adjustment> = None;
        let scrolled_window = gtk::ScrolledWindow::new(adj, adj);
        scrolled_window.add(&view);

        let info = gtk::Label::new(Some("TODO"));
        info.set_widget_name("info");
        info.set_xalign(0.0);

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
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

    pub fn get_view(&self) -> &View {
        &self.view
    }
}
