#[rustfmt::skip]
mod build_config;
mod common;
mod config;
mod lossy_text_read;
mod session_provider;
mod widgets;

use std::cell::RefCell;
use std::rc::Rc;
use std::{env, process};

use anyhow::Context;
use async_fs::File;
use common::bookmarks_url;
use futures::prelude::*;
use gtk::gio;
use gtk::prelude::*;
use log::error;

use crate::common::{
    BOOKMARK_FILE_PATH, CONFIG_DIR_PATH, DATA_DIR_PATH, DEFAULT_BOOKMARKS, HISTORY_FILE_PATH,
    SETTINGS_FILE_PATH,
};

async fn read_config() -> anyhow::Result<config::Config> {
    toml::from_str(&async_fs::read_to_string(&*SETTINGS_FILE_PATH).await?)
        .context("Reading config file")
}

async fn create_dir_if_not_exists(path: &std::path::Path) -> anyhow::Result<()> {
    if !path.exists() {
        async_fs::create_dir_all(path)
            .await
            .context(format!("Failed to create directory {:?}", path))?
    }

    Ok(())
}

async fn init_file_if_not_exists(
    path: &std::path::Path,
    text: Option<&[u8]>,
) -> anyhow::Result<()> {
    if !path.exists() {
        let mut file = File::create(path)
            .await
            .context(format!("Failed to init file {:?}", path))?;

        if let Some(text) = text {
            file.write_all(text).await?;
        }

        file.flush().await?;
    }

    Ok(())
}

async fn create_base_files() -> anyhow::Result<()> {
    let default_config = toml::to_string(&*config::DEFAULT_CONFIG).unwrap();

    create_dir_if_not_exists(&DATA_DIR_PATH).await?;
    create_dir_if_not_exists(&CONFIG_DIR_PATH).await?;
    init_file_if_not_exists(&BOOKMARK_FILE_PATH, Some(DEFAULT_BOOKMARKS.as_bytes())).await?;
    init_file_if_not_exists(&HISTORY_FILE_PATH, None).await?;
    init_file_if_not_exists(&SETTINGS_FILE_PATH, Some(default_config.as_bytes())).await?;

    Ok(())
}

fn main() {
    gtk::init().unwrap();
    env_logger::init();

    let resources = match env::var("MESON_DEVENV") {
        Err(_) => gio::Resource::load(config::RESOURCES_FILE)
            .unwrap_or_else(|_| panic!("Unable to load {}", config::RESOURCES_FILE)),
        Ok(_) => match env::current_exe() {
            Ok(mut resource_path) => {
                for _ in 0..2 {
                    resource_path.pop();
                }
                resource_path.push("share/geopard/resources.gresource");

                gio::Resource::load(&resource_path)
                    .expect("Unable to load resources.gresource in devenv")
            }
            Err(err) => {
                error!("Unable to find the current path: {}", err);
                process::exit(-1);
            }
        },
    };

    gio::resources_register(&resources);

    let application = adw::Application::builder()
        .application_id(config::APP_ID)
        .flags(gio::ApplicationFlags::HANDLES_OPEN)
        .resource_base_path("/com/ranfdev/Geopard/")
        .build();

    println!("{}", config::APP_ID);

    let config = futures::executor::block_on(async {
        create_base_files().await.unwrap();
        read_config().await.unwrap()
    });

    let windows = Rc::new(RefCell::new(vec![]));

    application
        .connect_activate(move |app| app.open(&[gio::File::for_uri(bookmarks_url().as_str())], ""));

    application.connect_open(move |app, files, _| {
        let window = widgets::Window::new(app, config.clone());
        window.present();
        windows.borrow_mut().push(window.clone());

        for f in files {
            gtk::prelude::WidgetExt::activate_action(&window, "win.new-empty-tab", None).unwrap();
            gtk::prelude::WidgetExt::activate_action(
                &window,
                "win.open-url",
                Some(&f.uri().to_variant()),
            )
            .unwrap();
        }
    });

    let shortcuts: &[(&'static str, &[&str])] = &[
        ("win.previous", &["<Alt>Left", "<Alt>KP_Left"]),
        ("win.next", &["<Alt>Right", "<Alt>KP_Right"]),
        ("win.reload", &["<Ctrl>r", "F5"]),
        ("win.show-bookmarks", &["<Ctrl>b"]),
        ("win.bookmark-current", &["<Ctrl>d"]),
        ("win.new-tab", &["<Ctrl>t"]),
        ("win.close-tab", &["<Ctrl>w"]),
        ("win.focus-url-bar", &["F6", "<Ctrl>L"]),
        ("win.zoom-in", &["<Ctrl>plus"]),
        ("win.zoom-out", &["<Ctrl>minus"]),
        ("win.reset-zoom", &["<Ctrl>0"]),
        // I can't directly use the action `overview.open` because the application cannot see that action.
        // Only widgets inside a `TabOverview` see that.
        ("win.open-overview", &["<Shift><Ctrl>o"]),
        // FIXME: win.view-source
    ];

    for (action_name, accels) in shortcuts {
        application.set_accels_for_action(action_name, accels);
    }

    let ret = application.run();
    process::exit(ret.into());
}
