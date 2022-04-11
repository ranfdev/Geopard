use crate::common::MARGIN;
use crate::config;

use glib::IsA;
use gtk::glib;
use gtk::prelude::*;
use log::{debug, info};

#[derive(Debug, Clone)]
pub struct DrawCtx {
    pub text_view: gtk::TextView,
    pub text_buffer: gtk::TextBuffer,
    pub config: crate::config::Config,
}
impl DrawCtx {
    pub fn new(text_view: gtk::TextView, config: crate::config::Config) -> Self {
        let text_buffer = gtk::TextBuffer::new(None);
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
        tag_h1.set_size_points(tag_h1.size_points() * 1.4);

        let tag_h2 = DrawCtx::create_tag("h2", {
            self.config
                .fonts
                .heading
                .as_ref()
                .or_else(|| default_config.fonts.heading.as_ref())
                .unwrap()
        });
        tag_h1.set_size_points(tag_h1.size_points() * 1.2);

        let tag_h3 = DrawCtx::create_tag(
            "h3",
            self.config
                .fonts
                .heading
                .as_ref()
                .or_else(|| default_config.fonts.heading.as_ref())
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
        tag_q.set_style(gtk::pango::Style::Italic);

        let tag_a = DrawCtx::create_tag(
            "a",
            self.config
                .fonts
                .quote
                .as_ref()
                .or_else(|| default_config.fonts.paragraph.as_ref())
                .unwrap(),
        );

        tag_a.set_foreground(Some("blue"));
        tag_a.set_underline(gtk::pango::Underline::Low);

        tag_table.add(&tag_h1);
        tag_table.add(&tag_h2);
        tag_table.add(&tag_h3);
        tag_table.add(&tag_q);
        tag_table.add(&tag_p);
        tag_table.add(&tag_a);
        tag_table
    }
    pub fn create_tag(name: &str, config: &crate::config::Font) -> gtk::TextTag {
        gtk::builders::TextTagBuilder::new()
            .family(&config.family)
            .size_points(config.size as f64)
            .weight(config.weight)
            .name(name)
            .build()
    }
    pub fn insert_heading(&self, text_iter: &mut gtk::TextIter, line: &str) {
        let n = line.chars().filter(|c| *c == '#').count();
        let line = line.trim_start_matches('#').trim_start();
        let tag_name = match n {
            1 => "h1",
            2 => "h2",
            _ => "h3",
        };

        let start = text_iter.offset();

        self.text_buffer.insert(text_iter, line);
        self.text_buffer.apply_tag_by_name(
            tag_name,
            &self.text_buffer.iter_at_offset(start),
            &self.text_buffer.end_iter(),
        );
    }

    pub fn insert_quote(&self, text_iter: &mut gtk::TextIter, line: &str) {
        let start = text_iter.offset();
        self.text_buffer.insert(text_iter, line);
        self.text_buffer
            .apply_tag_by_name("q", &self.text_buffer.iter_at_offset(start), text_iter);
    }

    pub fn insert_preformatted(&self, text_iter: &mut gtk::TextIter, line: &str) {
        let nested_view = {
            let text_view = gtk::TextView::new();
            let text_buffer = text_view.buffer();

            let tag_pre = DrawCtx::create_tag(
                "pre",
                self.config
                    .fonts
                    .preformatted
                    .as_ref()
                    .unwrap_or(&config::Fonts::default_preformatted()),
            );
            tag_pre.set_wrap_mode(gtk::WrapMode::None);

            text_buffer.tag_table().add(&tag_pre);
            text_buffer.insert(&mut text_buffer.end_iter(), line);
            text_buffer.apply_tag_by_name(
                "pre",
                text_buffer.iter_at_line_index(0, 0).as_ref().unwrap(),
                &text_buffer.end_iter(),
            );
            text_view
        };

        let scrolled_window = gtk::ScrolledWindow::new();
        scrolled_window.set_child(Some(&nested_view));
        scrolled_window.set_vscrollbar_policy(gtk::PolicyType::Never);
        self.insert_widget(text_iter, &scrolled_window);

        let text_view = self.text_view.clone();
        self.text_view.connect_width_request_notify(move |_| {
            scrolled_window.set_width_request(text_view.allocated_width() - MARGIN * 2)
        });
        self.text_buffer.insert(text_iter, "\n");
    }
    pub fn insert_paragraph(&self, text_iter: &mut gtk::TextIter, line: &str) {
        let start = text_iter.offset();
        self.text_buffer.insert(text_iter, line);
        self.text_buffer
            .apply_tag_by_name("p", &self.text_buffer.iter_at_offset(start), text_iter);
    }
    pub fn insert_link(
        &mut self,
        text_iter: &mut gtk::TextIter,
        link: String,
        label: Option<&str>,
    ) {
        debug!("Inserting link");
        let start = text_iter.offset();
        let default_config = &config::DEFAULT_CONFIG;

        let config = self
            .config
            .fonts
            .paragraph
            .as_ref()
            .or_else(|| default_config.fonts.paragraph.as_ref())
            .unwrap();

        let tag = gtk::builders::TextTagBuilder::new()
            .family(&config.family)
            .size_points(config.size as f64)
            .weight(config.weight)
            .build();

        tag.set_foreground(Some("#1c71d8"));
        tag.set_underline(gtk::pango::Underline::Low);

        Self::set_linkhandler(&tag, link.clone());

        let label = label.unwrap_or(&link);
        info!("Setted url {:?} to tag", Self::linkhandler(&tag));
        debug!("Link set successfully");
        self.insert_paragraph(text_iter, label);
        self.insert_paragraph(text_iter, "\n");

        let tag_table = self.text_buffer.tag_table();
        tag_table.add(&tag);

        self.text_buffer
            .apply_tag(&tag, &self.text_buffer.iter_at_offset(start), text_iter);
    }
    pub fn insert_widget(&self, text_iter: &mut gtk::TextIter, widget: &impl IsA<gtk::Widget>) {
        let anchor = self.text_buffer.create_child_anchor(text_iter);
        self.text_view.add_child_at_anchor(widget, &anchor);
    }

    fn set_linkhandler(tag: &gtk::TextTag, l: String) {
        unsafe {
            tag.set_data("linkhandler", l);
        }
    }
    pub fn linkhandler(tag: &gtk::TextTag) -> Option<&String> {
        unsafe {
            let handler: Option<std::ptr::NonNull<String>> = tag.data("linkhandler");
            handler.map(|n| n.as_ref())
        }
    }
    pub fn clear(&mut self) {
        let b = &self.text_buffer;
        b.delete(&mut b.start_iter(), &mut b.end_iter());

        self.text_buffer = gtk::TextBuffer::new(Some(&self.init_tags()));
        self.text_view.set_buffer(Some(&self.text_buffer));
    }
}
