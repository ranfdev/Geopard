use gtk::prelude::*;
use std::collections::HashMap;
use std::rc::Rc;
use std::cell::RefCell;
use async_trait::async_trait;
use std::borrow::Cow;
use crate::config::Fonts;
use futures::prelude::*;

pub trait TextRender<T> {
    fn render(&mut self, token: T);
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

#[derive(Debug, Clone)]
pub enum LinkHandler {
    Internal(String),
    External(String)
}

#[derive(Debug, Clone)]
pub struct Ctx {
    pub text_view: gtk::TextView,
    pub text_buffer: gtk::TextBuffer,
    pub config: crate::config::Config,
    pub links: Rc<RefCell<HashMap<i32, LinkHandler>>>,
}
impl Ctx {
    pub fn new(text_view: gtk::TextView, config: crate::config::Config) -> Self {
        let text_buffer = gtk::TextBuffer::new::<gtk::TextTagTable>(None);
            text_view.set_buffer(Some(&text_buffer));
            println!("SET NEW BUFFER");

        let links = Rc::new(RefCell::new(HashMap::new()));
        let this = Self {
            text_view,
            text_buffer,
            config,
            links
        };
        this.init_tags();
        this
    }

    pub fn init_tags(&self) {
        let tag_table = self.text_buffer.get_tag_table().unwrap();
        let tag_h1 = Ctx::create_tag("h1", {
            let mut cfg = self
                .config
                .fonts
                .heading
                .clone()
                .unwrap_or(Fonts::default_heading());
            cfg.size = (cfg.size as f32 * 1.3) as i32;
            cfg
        });

        let tag_h2 = Ctx::create_tag("h2", {
            let mut cfg = self
                .config
                .fonts
                .heading
                .clone()
                .unwrap_or(Fonts::default_heading());
            cfg.size = (cfg.size as f32 * 1.1) as i32;
            cfg
        });

        let tag_h3 = Ctx::create_tag(
            "h3",
            self.config
                .fonts
                .heading
                .clone()
                .unwrap_or(Fonts::default_heading()),
        );
        let tag_pre = Ctx::create_tag(
            "pre",
            self.config
                .fonts
                .preformatted
                .clone()
                .unwrap_or(Fonts::default_preformatted()),
        );
        let tag_p = Ctx::create_tag(
            "p",
            self.config
                .fonts
                .paragraph
                .clone()
                .unwrap_or(Fonts::default_paragraph()),
        );
        let tag_q = Ctx::create_tag(
            "q",
            self.config
                .fonts
                .quote
                .clone()
                .unwrap_or(Fonts::default_quote()),
        );
        tag_q.set_property_style(pango::Style::Italic);

        let tag_a = Ctx::create_tag(
            "a",
            self.config
                .fonts
                .quote
                .clone()
                .unwrap_or(Fonts::default_paragraph()),
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
        &self.text_buffer.get_tag_table().unwrap().foreach(|t| {
            dbg!(&t.get_property_name().unwrap().to_string());
        });
    }
    pub fn create_tag(name: &str, config: crate::config::Font) -> gtk::TextTag {
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
            3 | _ => "h3",
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
    pub fn insert_internal_link(&mut self, mut text_iter: &mut gtk::TextIter, url: &str, label: Option<&str>) {
        let label = label.unwrap_or(url);
        let link_handler = LinkHandler::Internal(url.to_owned());
        self.links.borrow_mut().insert(text_iter.get_line(), link_handler);

        let start = text_iter.get_offset();
        self.insert_paragraph(&mut text_iter, &label);
        self.insert_paragraph(&mut text_iter, "\n");
        self.text_buffer.apply_tag_by_name(
            "a",
            &self.text_buffer.get_iter_at_offset(start),
            &text_iter,
        );
    }

    pub fn insert_external_link(&mut self, mut text_iter: &mut gtk::TextIter, url: &str, label: Option<&str>) {
        let label = label.unwrap_or(url);
        let link_handler = LinkHandler::External(url.to_owned());
        self.links.borrow_mut().insert(text_iter.get_line(), link_handler);

        let start = text_iter.get_offset();
        self.insert_paragraph(&mut text_iter, &label);
        self.insert_paragraph(&mut text_iter, "\n");
        self.text_buffer.apply_tag_by_name(
            "a",
            &self.text_buffer.get_iter_at_offset(start),
            &text_iter,
        );
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
                // Yes, I know performance this requires useless copying.
                // This code will only be executed when invalid utf8 is found, so i
                // consider this as good enough
                buf.truncate(buf.len() - n); // Remove bad non-utf8 data
                buf.push_str(&valid_utf8_string); // Add correct utf8 data instead
                n = valid_utf8_string.len();
            }
            Ok(n)
        }
    }
}
