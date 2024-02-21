use gtk::glib;
use once_cell::sync::Lazy;

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

pub static OLD_BOOKMARK_FILE_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| DATA_DIR_PATH.join("bookmarks.gemini"));

pub static BOOKMARK_FILE_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| DATA_DIR_PATH.join("bookmarks.toml"));

pub static SETTINGS_FILE_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| CONFIG_DIR_PATH.join("config.toml"));

pub static HISTORY_FILE_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| DATA_DIR_PATH.join("history.gemini"));

pub const STREAMABLE_EXTS: [&str; 8] = ["mp3", "mp4", "webm", "opus", "wav", "ogg", "mkv", "flac"];

pub fn glibctx() -> glib::MainContext {
    glib::MainContext::default()
}
