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
        tag_a.set_line_height(1.4);
        tag_a.set_foreground(Some("blue"));

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
    pub fn clear(&mut self) {
        let b = &self.text_buffer;
        b.delete(&mut b.start_iter(), &mut b.end_iter());
    }
}
