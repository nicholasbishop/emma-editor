use gtk::prelude::*;

#[derive(Clone, Eq, PartialEq)]
pub struct Pane {
    scrolled_window: gtk::ScrolledWindow,
    view: sourceview::View,
}

impl Pane {
    pub fn new() -> Pane {
        let view = sourceview::View::new();
        view.set_monospace(true);
        let adj: Option<&gtk::Adjustment> = None;
        let scrolled_window = gtk::ScrolledWindow::new(adj, adj);
        scrolled_window.add(&view);
        Pane {
            scrolled_window,
            view,
        }
    }

    pub fn get_widget(&self) -> gtk::Widget {
        self.scrolled_window.clone().upcast()
    }

    // TODO: might want to remove this and just stick to a concept of
    // "active" pane
    pub fn has_focus(&self) -> bool {
        self.view.has_focus()
    }

    pub fn grab_focus(&self) {
        self.view.grab_focus();
    }

    pub fn get_view(&self) -> &sourceview::View {
        &self.view
    }
}
