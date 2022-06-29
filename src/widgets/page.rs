use crate::config;
use anyhow::Context;
use gemini::Event;
use glib::clone;
use glib::subclass::{Signal, SignalType};
use glib::Properties;
use gtk::glib;
use gtk::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gdk, gio};
use once_cell::sync::Lazy;
use std::cell::RefCell;
use std::collections::HashMap;
use url::Url;

#[derive(Debug, Clone)]
pub struct Surface {
    text_view: gtk::TextView,
    config: crate::config::Config,
}

impl Surface {
    pub fn new(config: crate::config::Config) -> Self {
        let text_view = gtk::TextView::builder()
            .top_margin(40)
            .bottom_margin(80)
            .left_margin(20)
            .right_margin(20)
            .hexpand(true)
            .indent(2)
            .editable(false)
            .cursor_visible(false)
            .wrap_mode(gtk::WrapMode::WordChar)
            .build();
        let text_buffer = gtk::TextBuffer::new(None);
        text_view.set_buffer(Some(&text_buffer));

        let mut this = Self { text_view, config };
        this.init_tags();
        this
    }

    pub fn root(&self) -> &gtk::Widget {
        self.text_view.upcast_ref()
    }

    fn init_tags(&mut self) -> gtk::TextTagTable {
        let default_config = &config::DEFAULT_CONFIG;
        let tag_table = self.text_view.buffer().tag_table();
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
        self.text_view
            .buffer()
            .tag_table()
            .lookup("a")
            .unwrap()
            .set_foreground_rgba(Some(color));
    }
    pub fn clear(&mut self) {
        let b = &self.text_view.buffer();
        b.delete(&mut b.start_iter(), &mut b.end_iter());
    }
}

pub enum PageEvent {
    Title(String),
    Link(gtk::TextTag, String),
}

pub enum Title {
    Incomplete(String),
    Complete(String),
}

pub mod imp {
    use super::*;

    #[derive(Default, Properties)]
    #[properties(wrapper_type = super::Page)]
    pub struct Page {
        pub(super) tag_stack: RefCell<Vec<gemini::Tag>>,
        pub(super) links: RefCell<HashMap<gtk::TextTag, String>>,
        pub(super) surface: RefCell<Option<Surface>>,
        #[property(get = Self::title, type = String)]
        pub(super) title: RefCell<Option<Title>>,
        pub(super) url: RefCell<String>,
        #[property(get)]
        pub(super) hover_url: RefCell<String>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Page {
        const NAME: &'static str = "GeopardPage";
        type Type = super::Page;
    }

    impl ObjectImpl for Page {
        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
                vec![
                    Signal::builder(
                        "open",
                        &[SignalType::from(glib::types::Type::STRING)],
                        <()>::static_type().into(),
                    )
                    .build(),
                    Signal::builder(
                        "open-in-new-tab",
                        &[SignalType::from(glib::types::Type::STRING)],
                        <()>::static_type().into(),
                    )
                    .build(),
                    Signal::builder(
                        "open-background-tab",
                        &[SignalType::from(glib::types::Type::STRING)],
                        <()>::static_type().into(),
                    )
                    .build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }

    impl Page {
        pub fn title(&self) -> String {
            match &*self.title.borrow() {
                None => String::new(),
                Some(Title::Incomplete(s)) => s.clone(),
                Some(Title::Complete(s)) => s.clone(),
            }
        }
    }
}
glib::wrapper! {
    pub struct Page(ObjectSubclass<imp::Page>);
}
impl Default for Page {
    fn default() -> Self {
        glib::Object::new(&[]).unwrap()
    }
}
impl Page {
    pub fn new(url: String, surface: Surface) -> Self {
        let text_view = surface.text_view.clone();

        let this: Self = glib::Object::new(&[]).unwrap();
        this.imp().url.replace(url);
        this.imp().surface.replace(Some(surface));

        let left_click_ctrl = gtk::GestureClick::builder().button(1).build();
        let right_click_ctrl = gtk::GestureClick::builder().button(3).build();
        let motion_ctrl = gtk::EventControllerMotion::new();

        text_view.add_controller(&left_click_ctrl);
        text_view.add_controller(&right_click_ctrl);
        text_view.add_controller(&motion_ctrl);

        left_click_ctrl.connect_released(
            clone!(@weak this => @default-panic, move |ctrl, _n_press, x, y| {
                if let Err(e) = this.handle_click(ctrl, x, y) {
                    log::info!("{}", e);
                };
            }),
        );

        right_click_ctrl.connect_pressed(
            clone!(@weak this => @default-panic, move |_ctrl, _n_press, x, y| {
                if let Err(e) = this.handle_right_click(x, y) {
                    log::info!("{}", e);
                };
            }),
        );

        motion_ctrl.connect_motion(clone!(@weak this => @default-panic,move |_ctrl, x, y|  {
            let _ = this.handle_motion(x, y);
        }));

        this
    }
    pub fn render<'e>(
        &self,
        tokens: impl Iterator<Item = gemini::Event<'e>>,
        page_events: &'e mut Vec<PageEvent>,
    ) -> anyhow::Result<()> {
        page_events.clear();
        let mut tag_stack = self.imp().tag_stack.borrow_mut();
        for ev in tokens {
            let parent_tag = tag_stack.last();
            match ev {
                Event::Start(t) => {
                    let buffer = self
                        .imp()
                        .surface
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .text_view
                        .buffer();
                    match &t {
                        gemini::Tag::Item => {
                            buffer.insert(&mut buffer.end_iter(), " •  ");
                        }
                        gemini::Tag::Link(url, label) => {
                            let link_char = if let Ok(true) = self
                                .parse_link(&url)
                                .map(|url| ["gemini", "about"].contains(&url.scheme()))
                            {
                                "⇒"
                            } else {
                                "⇗"
                            };
                            let label = format!("{link_char} {}", label.as_deref().unwrap_or(&url));
                            let tag = {
                                let mut text_iter = buffer.end_iter();
                                let start = text_iter.offset();

                                let tag = gtk::TextTag::new(None);
                                buffer.tag_table().add(&tag);

                                buffer.insert_with_tags_by_name(
                                    &mut text_iter,
                                    &label,
                                    &["p", "a"],
                                );
                                buffer.apply_tag(&tag, &buffer.iter_at_offset(start), &text_iter);

                                tag
                            };
                            self.imp()
                                .links
                                .borrow_mut()
                                .insert(tag.clone(), url.clone());
                            page_events.push(PageEvent::Link(tag, url.clone()));
                        }
                        gemini::Tag::Heading(1) => {
                            let mut title = self.imp().title.borrow_mut();
                            match &*title {
                                None => {
                                    *title = Some(Title::Incomplete(String::new()));
                                }
                                _ => {}
                            }
                        }

                        _ => {}
                    }
                    tag_stack.push(t);
                }
                Event::End => {
                    let buffer = self
                        .imp()
                        .surface
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .text_view
                        .buffer();
                    let parent_tag = parent_tag.context("Missing parent tag")?;
                    match parent_tag {
                        gemini::Tag::Paragraph
                        | gemini::Tag::Link(_, _)
                        | gemini::Tag::CodeBlock
                        | gemini::Tag::Heading(_)
                        | gemini::Tag::Item => {
                            buffer.insert(&mut buffer.end_iter(), "\n");
                            if matches!(parent_tag, gemini::Tag::Heading(1)) {
                                if let Some(Title::Incomplete(title)) = self.imp().title.take() {
                                    page_events.push(PageEvent::Title(title.clone()));
                                    self.imp().title.replace(Some(Title::Complete(title)));
                                }
                            }
                        }
                        _ => {}
                    }
                    tag_stack.pop();
                }
                Event::Text(text) => {
                    let buffer = self
                        .imp()
                        .surface
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .text_view
                        .buffer();
                    match parent_tag.context("Missing parent tag")? {
                        gemini::Tag::CodeBlock => {
                            buffer.insert_with_tags_by_name(&mut buffer.end_iter(), text, &["pre"]);
                        }
                        gemini::Tag::BlockQuote => {
                            buffer.insert_with_tags_by_name(&mut buffer.end_iter(), text, &["q"]);
                        }
                        gemini::Tag::Heading(lvl) if (0..6).contains(lvl) => {
                            let tag = format!("h{lvl}");
                            buffer.insert_with_tags_by_name(&mut buffer.end_iter(), text, &[&tag]);
                            if let Some(Title::Incomplete(title)) =
                                &mut *self.imp().title.borrow_mut()
                            {
                                if lvl == &1 {
                                    title.push_str(text);
                                }
                            }
                        }
                        gemini::Tag::Item => {
                            buffer.insert_with_tags_by_name(&mut buffer.end_iter(), text, &["p"]);
                        }
                        _ => buffer.insert_with_tags_by_name(&mut buffer.end_iter(), text, &["p"]),
                    }
                }
                Event::BlankLine => {
                    let buffer = self
                        .imp()
                        .surface
                        .borrow()
                        .as_ref()
                        .unwrap()
                        .text_view
                        .buffer();
                    buffer.insert(&mut buffer.end_iter(), "\n");
                }
            }
        }
        Ok(())
    }
    pub fn display_error(&self, error: anyhow::Error) {
        log::error!("{:?}", error);

        let status_page = adw::StatusPage::new();
        status_page.set_title("Error");
        status_page.set_description(Some(&error.to_string()));
        status_page.set_icon_name(Some("dialog-error-symbolic"));

        // TODO:
        /* self.stack.add_child(&status_page);
        self.stack.set_visible_child(&status_page); */
    }
    fn parse_link(&self, link: &str) -> Result<Url, url::ParseError> {
        let current_url = Url::parse(self.imp().url.borrow().as_str())?;
        let link_url = Url::options().base_url(Some(&current_url)).parse(link)?;
        Ok(link_url)
    }
    pub fn set_link_color(&self, color: &gtk::gdk::RGBA) {
        self.imp()
            .surface
            .borrow()
            .as_ref()
            .unwrap()
            .text_view
            .buffer()
            .tag_table()
            .lookup("a")
            .unwrap()
            .set_foreground_rgba(Some(color));
    }
    fn extract_linkhandler<'a>(
        m: &'a HashMap<gtk::TextTag, String>,
        text_view: &gtk::TextView,
        x: f64,
        y: f64,
    ) -> anyhow::Result<(&'a gtk::TextTag, &'a str)> {
        let (x, y) =
            text_view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
        let iter = text_view
            .iter_at_location(x as i32, y as i32)
            .context("Can't get text iter where clicked")?;

        iter.tags()
            .iter()
            .find_map(|x| x.name().is_none().then(|| m.get_key_value(x)).flatten())
            .map(|(k, v)| (k, v.as_str()))
            .ok_or_else(|| anyhow::Error::msg("Clicked text doesn't have a link tag"))
    }
    pub fn handle_click(&self, ctrl: &gtk::GestureClick, x: f64, y: f64) -> anyhow::Result<()> {
        let imp = self.imp();
        let surface = imp.surface.borrow();
        let text_view = &surface.as_ref().unwrap().text_view;
        if text_view.buffer().has_selection() {
            return Ok(());
        }
        let url = {
            let links = imp.links.borrow();
            let (_, link) = Self::extract_linkhandler(&*links, text_view, x, y)?;
            self.parse_link(link)?
        };
        if ctrl
            .current_event()
            .unwrap()
            .modifier_state()
            .contains(gdk::ModifierType::CONTROL_MASK)
        {
            self.emit_by_name::<()>("open-in-new-tab", &[&url.as_str()]);
        } else {
            self.emit_by_name::<()>("open", &[&url.as_str()]);
        }

        Ok(())
    }
    fn handle_right_click(&self, x: f64, y: f64) -> anyhow::Result<()> {
        let imp = self.imp();
        let surface = imp.surface.borrow();
        let text_view = &surface.as_ref().unwrap().text_view;

        let link = {
            let links = imp.links.borrow();
            let (_, link) = Self::extract_linkhandler(&*links, text_view, x, y)?;
            self.parse_link(link)?
        };
        let link_variant = link.as_str().to_variant();

        let menu = gio::Menu::new();

        let item = gio::MenuItem::new(Some("Open Link In New Tab"), None);
        item.set_action_and_target_value(Some("win.open-in-new-tab"), Some(&link_variant));

        menu.insert_item(0, &item);
        let item = gio::MenuItem::new(Some("Copy Link"), None);
        item.set_action_and_target_value(Some("win.set-clipboard"), Some(&link_variant));
        menu.insert_item(1, &item);
        text_view.set_extra_menu(Some(&menu));
        Ok(())
    }
    fn handle_motion(&self, x: f64, y: f64) -> anyhow::Result<()> {
        // May need some debounce?

        let imp = self.imp();
        let surface = imp.surface.borrow();
        let text_view = &surface.as_ref().unwrap().text_view;

        let links = imp.links.borrow();
        let entry = Self::extract_linkhandler(&*links, text_view, x, y);

        let link_ref = entry.as_ref().map(|x| x.1).unwrap_or("");
        if link_ref == *imp.hover_url.borrow() {
            return Ok(());
        }

        match link_ref {
            "" => {
                text_view.set_cursor_from_name(Some("text"));
            }
            _ => {
                text_view.set_cursor_from_name(Some("pointer"));
            }
        };

        imp.hover_url.replace(link_ref.to_owned());
        self.emit_hover_url();
        Ok(())
    }
}
