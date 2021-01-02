use gio::prelude::*;
use gtk::Application;
use std::cell::RefCell;
use std::rc::Rc;

mod common;
mod component;
mod config;
mod gemini;
mod tab;
mod window;

use gtk::prelude::*;

fn main() {
    gtk::init().unwrap();
    env_logger::init();

    let application = Application::new(
        Some("com.ranfdev.Geopard"),
        gio::ApplicationFlags::FLAGS_NONE,
    )
    .expect("Failed to init gtk app");

    let app_clone = application.clone();
    let windows = Rc::new(RefCell::new(vec![]));

    let windows_clone = windows.clone();
    application.connect_activate(move |_| {
        let window = window::Window::new(&app_clone);
        window.widget().show_all();
        window.widget().present();
        windows_clone.borrow_mut().push(window);
    });

    let ret = application.run(&std::env::args().collect::<Vec<String>>());
    std::process::exit(ret);
}
