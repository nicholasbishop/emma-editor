#![warn(clippy::use_self)]

mod draw;
mod init;
mod key;

use gtk4::Application;
use gtk4::prelude::{ApplicationExt, ApplicationExtManual};

fn main() {
    tracing_subscriber::fmt::init();

    let application = Application::builder()
        .application_id("org.emma.Emma")
        .register_session(true)
        .build();

    application.connect_startup(init::init);

    application.run();
}
