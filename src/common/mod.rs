use gtk::{glib, gio};
use once_cell::sync::Lazy;
use url::Url;

pub static DOWNLOAD_PATH: Lazy<std::path::PathBuf> = Lazy::new(|| {
    let mut download_path = glib::user_special_dir(glib::UserDirectory::Downloads)
        .expect("Can't access download directory");
    download_path.push("Geopard");
    if !download_path.exists() {
        std::fs::create_dir(&download_path).expect("Can't create download folder");
    }
    download_path
});

pub static ABOUT_PAGE: &str = std::include_str!("../../README.gemini");

pub static DATA_DIR_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| glib::user_data_dir().join("geopard"));

pub static KNOWN_HOSTS_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| DATA_DIR_PATH.join("known_hosts"));

pub static CONFIG_DIR_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| glib::user_config_dir().join("geopard"));

pub static BOOKMARK_FILE_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| DATA_DIR_PATH.join("bookmarks.gemini"));

pub static SETTINGS_FILE_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| CONFIG_DIR_PATH.join("config.toml"));

pub static HISTORY_FILE_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| DATA_DIR_PATH.join("history.gemini"));

pub static DEFAULT_BOOKMARKS: &str = r"# Bookmarks

This is a gemini file where you can put all your bookmarks.
You can even edit this file in a text editor. That's how you
should remove bookmarks.

## Default bookmarks

=> gemini://geminiprotocol.net Gemini project
=> gemini://warmedal.se/~antenna/ Antenna aggregator
=> about:help About geopard + help

## Custom bookmarks

";

pub const STREAMABLE_EXTS: [&str; 8] = ["mp3", "mp4", "webm", "opus", "wav", "ogg", "mkv", "flac"];

pub fn bookmarks_url() -> Url {
    Url::parse(&format!("file://{}", BOOKMARK_FILE_PATH.to_str().unwrap())).unwrap()
}

pub fn glibctx() -> glib::MainContext {
    glib::MainContext::default()
}

pub fn open_uri_externally(uri: &str) {
    gtk::UriLauncher::new(&uri).launch(None::<&gtk::Window>, None::<&gio::Cancellable>, |res| {
        if let Err(e) = res {
            log::error!("error opening external uri {:?}", e);
        }
    });
}

pub fn open_file_externally(path: &std::path::Path) {
    let file = gio::File::for_path(path);
    gtk::FileLauncher::new(Some(&file)).launch(None::<&gtk::Window>, None::<&gio::Cancellable>, |res| {
        if let Err(e) = res {
            log::error!("error opening external file {:?}", e);
        }
    });
}
