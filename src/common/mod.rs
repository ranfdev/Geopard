use async_trait::async_trait;
use futures::prelude::*;
use gdk::prelude::*;
use glib::subclass::prelude::*;
use glib::GBoxed;
use gtk::prelude::*;
use log::{debug, info};
use std::borrow::Cow;
use url::Url;

use crate::config;
use crate::gemini;
use crate::tab::TabMsg;

use once_cell::sync::Lazy;

pub static DOWNLOAD_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| glib::get_user_special_dir(glib::UserDirectory::Downloads).unwrap());

pub static ABOUT_PAGE: &str = std::include_str!("../../README.gemini");

pub const MARGIN: i32 = 20;

pub static DATA_DIR_PATH: Lazy<std::path::PathBuf> = Lazy::new(|| {
    glib::get_user_data_dir()
        .expect("No user data dir")
        .join("geopard")
});

pub static CONFIG_DIR_PATH: Lazy<std::path::PathBuf> = Lazy::new(|| {
    glib::get_user_config_dir()
        .expect("No user config dir")
        .join("geopard")
});

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

## Default bookmarks:
=> gemini://gemini.circumlunar.space/ Gemini project
=> gemini://rawtext.club:1965/~sloum/spacewalk.gmi Spacewalk aggregator
=> about:help About geopard + help

## Custom bookmarks:
";

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
pub enum Link {
    Internal(String),
    External(String),
}

impl Link {
    pub fn url(&self) -> &str {
        match self {
            Link::Internal(url) => url,
            Link::External(url) => url,
        }
    }
}

#[derive(Debug, Clone, GBoxed)]
#[gboxed(type_name = "GLink")]
pub struct GLink(Link);

#[derive(Debug, Clone, PartialEq)]
pub struct HistoryItem {
    pub url: url::Url,
    pub cache: Option<()>,
    pub scroll_progress: f64,
}

// This struct contains all the data needed to fetch and render the data of a page
pub struct RequestCtx {
    pub gemini_client: gemini::Client,
    pub draw_ctx: DrawCtx,
    pub in_chan_tx: flume::Sender<TabMsg>,
    pub url: Url,
}

#[derive(Debug, Clone)]
pub struct DrawCtx {
    pub text_view: gtk::TextView,
    pub text_buffer: gtk::TextBuffer,
    pub config: crate::config::Config,
}
impl DrawCtx {
    pub fn new(text_view: gtk::TextView, config: crate::config::Config) -> Self {
        let text_buffer = gtk::TextBuffer::new::<gtk::TextTagTable>(None);
        text_view.set_buffer(Some(&text_buffer));

        let this = Self {
            text_view,
            text_buffer,
            config,
        };
        this.init_tags();
        this
    }

    pub fn init_tags(&self) -> gtk::TextTagTable {
        let default_config = &config::DEFAULT_CONFIG;
        let tag_table = gtk::TextTagTable::new();
        let tag_h1 = DrawCtx::create_tag("h1", {
            self.config
                .fonts
                .heading
                .as_ref()
                .or_else(|| default_config.fonts.heading.as_ref())
                .unwrap()
        });
        tag_h1.set_property_size_points(tag_h1.get_property_size_points() * 1.4);

        let tag_h2 = DrawCtx::create_tag("h2", {
            self.config
                .fonts
                .heading
                .as_ref()
                .or_else(|| default_config.fonts.heading.as_ref())
                .unwrap()
        });
        tag_h1.set_property_size_points(tag_h1.get_property_size_points() * 1.2);

        let tag_h3 = DrawCtx::create_tag(
            "h3",
            self.config
                .fonts
                .heading
                .as_ref()
                .or_else(|| default_config.fonts.heading.as_ref())
                .unwrap(),
        );
        let tag_pre = DrawCtx::create_tag(
            "pre",
            self.config
                .fonts
                .preformatted
                .as_ref()
                .or_else(|| default_config.fonts.preformatted.as_ref())
                .unwrap(),
        );
        let tag_p = DrawCtx::create_tag(
            "p",
            self.config
                .fonts
                .paragraph
                .as_ref()
                .or_else(|| default_config.fonts.paragraph.as_ref())
                .unwrap(),
        );
        let tag_q = DrawCtx::create_tag(
            "q",
            self.config
                .fonts
                .quote
                .as_ref()
                .or_else(|| default_config.fonts.quote.as_ref())
                .unwrap(),
        );
        tag_q.set_property_style(pango::Style::Italic);

        let tag_a = DrawCtx::create_tag(
            "a",
            self.config
                .fonts
                .quote
                .as_ref()
                .or_else(|| default_config.fonts.paragraph.as_ref())
                .unwrap(),
        );

        tag_a.set_property_foreground(Some("blue"));
        tag_a.set_property_underline(pango::Underline::Low);

        tag_table.add(&tag_h1);
        tag_table.add(&tag_h2);
        tag_table.add(&tag_h3);
        tag_table.add(&tag_pre);
        tag_table.add(&tag_q);
        tag_table.add(&tag_p);
        tag_table.add(&tag_a);
        tag_table
    }
    pub fn create_tag(name: &str, config: &crate::config::Font) -> gtk::TextTag {
        gtk::TextTagBuilder::new()
            .family(&config.family)
            .size_points(config.size as f64)
            .weight(config.weight)
            .name(name)
            .build()
    }
    pub fn insert_heading(&self, mut text_iter: &mut gtk::TextIter, line: &str) {
        let n = line.chars().filter(|c| *c == '#').count();
        let line = line.trim_start_matches('#').trim_start();
        let tag_name = match n {
            1 => "h1",
            2 => "h2",
            _ => "h3",
        };

        let start = text_iter.get_offset();

        self.text_buffer.insert(&mut text_iter, &line);
        self.text_buffer.apply_tag_by_name(
            tag_name,
            &self.text_buffer.get_iter_at_offset(start),
            &self.text_buffer.get_end_iter(),
        );
    }

    pub fn insert_quote(&self, mut text_iter: &mut gtk::TextIter, line: &str) {
        let start = text_iter.get_offset();
        self.text_buffer.insert(&mut text_iter, &line);
        self.text_buffer.apply_tag_by_name(
            "q",
            &self.text_buffer.get_iter_at_offset(start),
            &text_iter,
        );
    }

    pub fn insert_preformatted(&self, mut text_iter: &mut gtk::TextIter, line: &str) {
        let start = text_iter.get_offset();
        self.text_buffer.insert(&mut text_iter, &line);
        self.text_buffer.apply_tag_by_name(
            "pre",
            &self.text_buffer.get_iter_at_offset(start),
            &text_iter,
        );
    }
    pub fn insert_paragraph(&self, mut text_iter: &mut gtk::TextIter, line: &str) {
        let start = text_iter.get_offset();
        self.text_buffer.insert(&mut text_iter, &line);
        self.text_buffer.apply_tag_by_name(
            "p",
            &self.text_buffer.get_iter_at_offset(start),
            &text_iter,
        );
    }
    pub fn insert_link(
        &mut self,
        mut text_iter: &mut gtk::TextIter,
        link: Link,
        label: Option<&str>,
    ) {
        debug!("Inserting link");
        let start = text_iter.get_offset();
        let default_config = &config::DEFAULT_CONFIG;

        let config = self
            .config
            .fonts
            .paragraph
            .as_ref()
            .or_else(|| default_config.fonts.paragraph.as_ref())
            .unwrap();

        let tag = gtk::TextTagBuilder::new()
            .family(&config.family)
            .size_points(config.size as f64)
            .weight(config.weight)
            .build();

        tag.set_property_foreground(Some("blue"));
        tag.set_property_underline(pango::Underline::Low);

        Self::set_linkhandler(&tag, link.clone());

        let label = label.unwrap_or_else(|| link.url());
        info!("Setted url {:?} to tag", Self::get_linkhandler(&tag));
        debug!("Link set successfully");
        self.insert_paragraph(&mut text_iter, &label);
        self.insert_paragraph(&mut text_iter, "\n");

        let tag_table = self.text_buffer.get_tag_table().unwrap();
        tag_table.add(&tag);

        self.text_buffer.apply_tag(
            &tag,
            &self.text_buffer.get_iter_at_offset(start),
            &text_iter,
        );
    }

    fn set_linkhandler(tag: &gtk::TextTag, s: Link) {
        // I hope using a GBoxed type manages memory for me
        // TODO: Ask a glib mantainer if this is true
        unsafe {
            tag.set_data("linkhandler", GLink(s).to_value());
        }
    }
    pub fn get_linkhandler(tag: &gtk::TextTag) -> Option<Link> {
        unsafe {
            let handler: Option<&glib::Value> = tag.get_data("linkhandler");
            handler
        }
        .and_then(|gl| gl.get::<&GLink>().ok())
        .and_then(|l| l.to_owned())
        .map(|l| l.0.clone())
    }
    pub fn clear(&mut self) {
        let b = &self.text_buffer;
        b.delete(&mut b.get_start_iter(), &mut b.get_end_iter());

        self.text_buffer = gtk::TextBuffer::new(Some(&self.init_tags()));
        self.text_view.set_buffer(Some(&self.text_buffer));
    }
}

#[async_trait(?Send)]
pub trait LossyTextRead {
    async fn read_line_lossy(&mut self, mut buf: &mut String) -> std::io::Result<usize>;
}

#[async_trait(?Send)]
impl<T: AsyncBufRead + Unpin> LossyTextRead for T {
    async fn read_line_lossy(&mut self, buf: &mut String) -> std::io::Result<usize> {
        // This is safe because we treat buf as a mut Vec to read the data, BUT,
        // we check if it's valid utf8 using String::from_utf8_lossy.
        // If it's not valid utf8, we swap our buf with the newly allocated and
        // safe string returned from String::from_utf8_lossy
        //
        // In the implementation of BufReader::read_line, they talk about some things about
        // panic handling, which I don't understand currently. Whatever...
        unsafe {
            let mut vec_buf = buf.as_mut_vec();
            let mut n = self.read_until(b'\n', &mut vec_buf).await?;

            let correct_string = String::from_utf8_lossy(&vec_buf);
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
