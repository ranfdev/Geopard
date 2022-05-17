use async_trait::async_trait;
use futures::prelude::*;
use gtk::glib;
use once_cell::sync::Lazy;
use std::borrow::Cow;
use std::cell::RefCell;
use std::rc::Rc;
use url::Url;

use crate::gemini;

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

pub const MARGIN: i32 = 20;

pub static DATA_DIR_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| glib::user_data_dir().join("geopard"));

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

=> gemini://gemini.circumlunar.space/ Gemini project
=> gemini://rawtext.club:1965/~sloum/spacewalk.gmi Spacewalk aggregator
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

pub struct Color(pub u8, pub u8, pub u8);

impl std::fmt::LowerHex for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02x}", self.0)?;
        write!(f, "{:02x}", self.1)?;
        write!(f, "{:02x}", self.2)?;
        Ok(())
    }
}

#[derive(Debug)]
pub enum PageElement {
    Heading(String),
    Quote(String),
    Preformatted(String),
    Text(String),
    Link(String, Option<String>),
    Empty,
}

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryItem {
    pub url: url::Url,
    pub cache: Rc<RefCell<Option<Vec<u8>>>>,
    pub scroll_progress: f64,
}

#[async_trait(?Send)]
pub trait LossyTextRead {
    async fn read_line_lossy(&mut self, mut buf: &mut String) -> std::io::Result<usize>;
}

#[async_trait(?Send)]
impl<T: AsyncBufRead + Unpin> LossyTextRead for T {
    async fn read_line_lossy(&mut self, buf: &mut String) -> std::io::Result<usize> {
        // FIXME:  thread 'main' panicked at 'assertion failed: self.is_char_boundary(new_len)', /build/rustc-1.58.1-src/library/alloc/src/string.rs:1204:13
        // This is safe because we treat buf as a mut Vec to read the data, BUT,
        // we check if it's valid utf8 using String::from_utf8_lossy.
        // If it's not valid utf8, we swap our buf with the newly allocated and
        // safe string returned from String::from_utf8_lossy
        //
        // In the implementation of BufReader::read_line, they talk about some things about
        // panic handling, which I don't understand currently. Whatever...
        unsafe {
            let vec_buf = buf.as_mut_vec();
            let mut n = self.read_until(b'\n', vec_buf).await?;

            let correct_string = String::from_utf8_lossy(vec_buf);
            if let Cow::Owned(valid_utf8_string) = correct_string {
                // Yes, I know this is not good for performance because it requires useless copying.
                // BUT, this code will only be executed when invalid utf8 is found, so i
                // consider this as good enough
                buf.truncate(buf.len() - n); // Remove bad non-utf8 data
                buf.push_str(&valid_utf8_string); // Add correct utf8 data instead
                n = valid_utf8_string.len();
            }
            Ok(n)
        }
    }
}
