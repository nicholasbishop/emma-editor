use {
    crate::buffer::Embuf,
    gtk4::{self as gtk, prelude::*},
    std::{cell::RefCell, rc::Rc},
};

type View = gtk::TextView;

#[derive(Debug, Eq, PartialEq)]
struct PaneInternal {
    container: gtk::Box,

    info: gtk::Label,

    scrolled_window: gtk::ScrolledWindow,
    view: View,

    embuf: Embuf,
}

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Pane(Rc<RefCell<PaneInternal>>);

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
        // TODO
        self.0
            .borrow()
            .info
            .set_text(&format!("{}", self.borrow().embuf.path().display()));
    }
}
