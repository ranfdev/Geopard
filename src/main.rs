use crate::common::{
    BOOKMARK_FILE_PATH, CONFIG_DIR_PATH, DATA_DIR_PATH, DEFAULT_BOOKMARKS, HISTORY_FILE_PATH,
    SETTINGS_FILE_PATH,
};
use anyhow::Context;
use async_fs::File;
use futures::prelude::*;
use gio::prelude::*;
use gtk::Application;
use std::cell::RefCell;
use std::rc::Rc;

mod build_config;
mod common;
mod component;
mod config;
mod gemini;
mod tab;
mod window;

use gtk::prelude::*;

async fn read_config() -> anyhow::Result<config::Config> {
    toml::from_str(&async_fs::read_to_string(&*SETTINGS_FILE_PATH).await?)
        .context("Reading config file")
}
async fn create_dir_if_not_exists(path: &std::path::PathBuf) -> anyhow::Result<()> {
    if !path.exists() {
        async_fs::create_dir_all(&*path)
            .await
            .context(format!("Failed to create directory {:?}", path))?
    }
    Ok(())
}
async fn init_file_if_not_exists(
    path: &std::path::PathBuf,
    text: Option<&[u8]>,
) -> anyhow::Result<()> {
    if !path.exists() {
        let mut file = File::create(path)
            .await
            .context(format!("Failed to init file {:?}", path))?;

        if let Some(text) = text {
            file.write_all(text).await?;
        }
    }
    Ok(())
}
async fn create_base_files() -> anyhow::Result<()> {
    create_dir_if_not_exists(&DATA_DIR_PATH).await?;
    create_dir_if_not_exists(&CONFIG_DIR_PATH).await?;
    init_file_if_not_exists(&BOOKMARK_FILE_PATH, Some(DEFAULT_BOOKMARKS.as_bytes())).await?;
    init_file_if_not_exists(&HISTORY_FILE_PATH, None).await?;
    let default_config = toml::to_string(&*config::DEFAULT_CONFIG).unwrap();

    init_file_if_not_exists(&SETTINGS_FILE_PATH, Some(default_config.as_bytes())).await?;

    Ok(())
}

fn main() {
    gtk::init().unwrap();
    env_logger::init();

    let application = adw::Application::new(
        Some(build_config::APP_ID),
        gio::ApplicationFlags::FLAGS_NONE,
    );
    println!("{}", build_config::APP_ID);

    let config = futures::executor::block_on(async {
        create_base_files().await.unwrap();
        read_config().await.unwrap()
    });

    let app_clone = application.clone();
    let windows = Rc::new(RefCell::new(vec![]));

    application.connect_activate(move |_| {
        let window = window::Window::new(&app_clone, config.clone());
        window.widget().present();
        windows.borrow_mut().push(window);
    });

    let ret = application.run();
    std::process::exit(ret);
}
