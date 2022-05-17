use crate::config;
use gtk::prelude::*;

#[derive(Debug, Clone)]
pub struct Gemini {
    pub text_view: gtk::TextView,
    pub text_buffer: gtk::TextBuffer,
    pub config: crate::config::Config,
}
impl Gemini {
    pub fn new(text_view: gtk::TextView, config: crate::config::Config) -> Self {
        let text_buffer = gtk::TextBuffer::new(None);
        text_view.set_buffer(Some(&text_buffer));

        let mut this = Self {
            text_view,
            text_buffer,
            config,
        };
        this.init_tags();
        this
    }

    fn init_tags(&mut self) -> gtk::TextTagTable {
        let default_config = &config::DEFAULT_CONFIG;
        let tag_table = self.text_buffer.tag_table();
        let tag_h1 = Self::create_tag("h1", {
            self.config
                .fonts
                .heading
                .as_ref()
                .or(default_config.fonts.heading.as_ref())
                .unwrap()
        });
        tag_h1.set_scale(2.0);
        tag_h1.set_sentence(true);

        let tag_h2 = Self::create_tag("h2", {
            self.config
                .fonts
                .heading
                .as_ref()
                .or(default_config.fonts.heading.as_ref())
                .unwrap()
        });
        tag_h2.set_scale(1.5);
        tag_h1.set_sentence(true);

        let tag_h3 = Self::create_tag(
            "h3",
            self.config
                .fonts
                .heading
                .as_ref()
                .or(default_config.fonts.heading.as_ref())
                .unwrap(),
        );
        tag_h2.set_scale(1.4);
        tag_h1.set_sentence(true);

        let tag_p = Self::create_tag(
            "p",
            self.config
                .fonts
                .paragraph
                .as_ref()
                .or(default_config.fonts.paragraph.as_ref())
                .unwrap(),
        );
        let tag_q = Self::create_tag(
            "q",
            self.config
                .fonts
                .quote
                .as_ref()
                .or(default_config.fonts.quote.as_ref())
                .unwrap(),
        );
        tag_q.set_style(gtk::pango::Style::Italic);

        let tag_a = Self::create_tag(
            "a",
            self.config
                .fonts
                .quote
                .as_ref()
                .or(default_config.fonts.paragraph.as_ref())
                .unwrap(),
        );

        tag_a.set_foreground(Some("blue"));
        tag_a.set_underline(gtk::pango::Underline::Low);

        let tag_pre = Self::create_tag(
            "pre",
            self.config
                .fonts
                .preformatted
                .as_ref()
                .unwrap_or(&config::Fonts::default_preformatted()),
        );
        tag_pre.set_wrap_mode(gtk::WrapMode::None);

        tag_table.add(&tag_h1);
        tag_table.add(&tag_h2);
        tag_table.add(&tag_h3);
        tag_table.add(&tag_q);
        tag_table.add(&tag_p);
        tag_table.add(&tag_a);
        tag_table.add(&tag_pre);
        tag_table
    }
    fn create_tag(name: &str, config: &crate::config::Font) -> gtk::TextTag {
        gtk::builders::TextTagBuilder::new()
            .family(&config.family)
            .weight(config.weight)
            .name(name)
            .build()
    }
    pub fn set_link_color(&self, color: &gtk::gdk::RGBA) {
        self.text_buffer
            .tag_table()
            .lookup("a")
            .unwrap()
            .set_foreground_rgba(Some(color));
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
        let start = text_iter.offset();
        self.text_buffer.insert(text_iter, line);
        self.text_buffer.apply_tag_by_name(
            "pre",
            &self.text_buffer.iter_at_offset(start),
            text_iter,
        );

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
        let start = text_iter.offset();
        let default_config = &config::DEFAULT_CONFIG;

        let config = self
            .config
            .fonts
            .paragraph
            .as_ref()
            .or(default_config.fonts.paragraph.as_ref())
            .unwrap();

        let tag = gtk::builders::TextTagBuilder::new()
            .family(&config.family)
            .weight(config.weight)
            .line_height(1.4)
            .build();

        tag.set_foreground_rgba(
            self.text_view
                .style_context()
                .lookup_color("accent_color")
                .as_ref(),
        );

        Self::set_linkhandler(&tag, link.clone());

        let label = label.unwrap_or(&link);
        self.insert_paragraph(text_iter, label);
        self.insert_paragraph(text_iter, "\n");

        let tag_table = self.text_buffer.tag_table();
        tag_table.add(&tag);

        self.text_buffer
            .apply_tag(&tag, &self.text_buffer.iter_at_offset(start), text_iter);
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
    }
}
