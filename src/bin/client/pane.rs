use {
    crate::buffer::Embuf,
    gtk4::{self as gtk, prelude::*},
    std::{cell::RefCell, rc::Rc},
};

type View = gtk::TextView;

#[derive(Debug)]
struct PaneInternal {
    container: gtk::Box,

    info: gtk::Label,

    scrolled_window: gtk::ScrolledWindow,
    view: View,

    embuf: Embuf,
}

#[derive(Debug, Clone)]
pub struct Pane(Rc<RefCell<PaneInternal>>);

impl PartialEq for Pane {
    fn eq(&self, other: &Pane) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}

impl Eq for Pane {}

impl Pane {
    pub fn new(embuf: &Embuf) -> Pane {
        let view = View::new();
        view.set_monospace(true);
        let scrolled_window = gtk::ScrolledWindow::new();
        scrolled_window.set_child(Some(&view));
        scrolled_window.set_vexpand(true);

        let info = gtk::Label::new(None);
        info.set_widget_name("info");
        info.set_xalign(0.0);

        let container = gtk::Box::new(gtk::Orientation::Vertical, 0);
        container.set_widget_name("Pane");
        container.append(&scrolled_window);
        container.append(&info);

        let pane = Pane(Rc::new(RefCell::new(PaneInternal {
            container,
            info,
            scrolled_window,
            view,
            embuf: embuf.clone(),
        })));

        pane.set_buffer(embuf);
        pane
    }

    fn borrow(&self) -> std::cell::Ref<PaneInternal> {
        self.0.borrow()
    }

    pub fn view(&self) -> View {
        self.borrow().view.clone()
    }

    pub fn embuf(&self) -> Embuf {
        self.borrow().embuf.clone()
    }

    pub fn get_widget(&self) -> gtk::Widget {
        self.borrow().container.clone().upcast()
    }

    pub fn set_buffer(&self, embuf: &Embuf) {
        self.0.borrow_mut().embuf = embuf.clone();
        self.borrow().view.set_buffer(Some(&embuf.storage()));
        self.update_info();
    }

    fn update_info(&self) {
        // TODO: handle multiple buffers with same name.
        let name = self.borrow().embuf.name();
        self.borrow().info.set_text(&name);
    }

    pub fn set_active(&self, active: bool) {
        let info_name = if active { "info-active" } else { "info" };
        let internal = self.borrow();
        internal.info.set_widget_name(info_name);

        if active {
            internal.view.grab_focus();
        }
    }
}
