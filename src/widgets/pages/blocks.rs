use std::collections::HashMap;
use std::rc::Rc;

use adw::prelude::*;
use adw::subclass::prelude::*;
use anyhow::Context;
use futures::pin_mut;
use futures::prelude::*;
use futures::stream::Peekable;
use gemini::{Event, Tag};
use gtk::{glib, pango};
use url::Url;

use crate::widgets::pages::hypertext;

pub trait BlockSignalHandlers {
    fn open(&self, url: &str) {
        println!("Opening {}", url);
    }
    fn open_in_new_tab(&self, url: &str) {
        println!("Opening in new tab {}", url);
    }
    fn update_hover_url(&self, url: &str) {
        println!("Updating hover url {}", url);
    }
    fn show_menu(&self) {
        println!("Showing menu");
    }
}

pub struct BlockCtx {
    pub signals: Rc<dyn BlockSignalHandlers>,
    pub count: u32,
}
pub trait Block: std::fmt::Debug {
    fn activate_search(&self, text: &str) -> Box<dyn Iterator<Item = ()>>;
    fn clear_search(&self);
    fn widget(&self) -> &gtk::Widget;
    fn text(&self) -> String;
}

#[derive(Debug)]
pub struct Title {
    widget: gtk::Label,
    level: u8,
}

impl Title {
    pub fn new(ctx: &BlockCtx, level: u8, text: &str) -> Self {
        let widget = gtk::Label::new(Some(&text));
        widget.set_xalign(0.0);
        widget.set_wrap_mode(pango::WrapMode::WordChar);
        widget.set_wrap(true);
        widget.add_css_class("animate-slide-left-right");
        widget.add_css_class(&format!("delay-{}", ctx.count * 10));
        widget.add_css_class(&format!("h{level}"));
        Self { widget, level }
    }
}

impl Block for Title {
    fn activate_search(&self, text: &str) -> Box<dyn Iterator<Item = ()>> {
        Box::new(std::iter::empty())
    }
    fn clear_search(&self) {
        // Do nothing
    }
    fn widget(&self) -> &gtk::Widget {
        self.widget.upcast_ref()
    }
    fn text(&self) -> String {
        self.widget.text().to_string()
    }
}

#[derive(Debug)]
pub struct Text {
    widget: gtk::Label,
}

impl Text {
    pub fn new(ctx: &BlockCtx, text: &str) -> Self {
        let widget = gtk::Label::new(Some(&text));
        widget.set_xalign(0.0);
        widget.set_wrap_mode(pango::WrapMode::WordChar);
        widget.set_wrap(true);
        widget.set_hexpand(true);
        widget.add_css_class(&format!("delay-{}", ctx.count * 10));
        widget.add_css_class("animate-slide-left-right");

        Self { widget }
    }
}

impl Block for Text {
    fn activate_search(&self, text: &str) -> Box<dyn Iterator<Item = ()>> {
        Box::new(std::iter::empty())
    }
    fn clear_search(&self) {
        // Do nothing
    }
    fn widget(&self) -> &gtk::Widget {
        self.widget.upcast_ref()
    }
    fn text(&self) -> String {
        self.widget.text().to_string()
    }
}

#[derive(Debug)]
pub struct Link {
    widget: gtk::Button,
}

impl Link {
    pub fn new(ctx: &BlockCtx, url: &str, label: Option<&str>) -> Self {
        let label = gtk::Label::new(label.or(Some(url)));
        label.set_wrap_mode(pango::WrapMode::WordChar);
        label.set_wrap(true);
        let widget = gtk::Button::new();
        widget.set_child(Some(&label));
        widget.add_css_class("link");
        widget.add_css_class("text-button");
        widget.add_css_class("animate-slide-left-right");
        widget.add_css_class(&format!("delay-{}", ctx.count * 10));
        widget.set_hexpand(true);
        widget.set_halign(gtk::Align::Start);
        let url = url.to_string();
        let signals = ctx.signals.clone();
        widget.connect_clicked(move |_| signals.open(&url));
        Self { widget }
    }
}

impl Block for Link {
    fn activate_search(&self, text: &str) -> Box<dyn Iterator<Item = ()>> {
        Box::new(std::iter::empty())
    }
    fn clear_search(&self) {
        // Do nothing
    }
    fn widget(&self) -> &gtk::Widget {
        self.widget.upcast_ref()
    }
    fn text(&self) -> String {
        String::from("")
    }
}

#[derive(Debug)]
pub struct SourceView {
    widget: sourceview5::View,
}

impl SourceView {
    pub fn new(text: &str) -> Self {
        let widget = sourceview5::View::new();
        widget.buffer().set_text(text);
        Self { widget }
    }
}

impl Block for SourceView {
    fn activate_search(&self, text: &str) -> Box<dyn Iterator<Item = ()>> {
        Box::new(std::iter::empty())
    }
    fn clear_search(&self) {
        // Do nothing
    }
    fn widget(&self) -> &gtk::Widget {
        self.widget.upcast_ref()
    }
    fn text(&self) -> String {
        let start = self.widget.buffer().start_iter();
        let end = self.widget.buffer().end_iter();
        self.widget.buffer().text(&start, &end, true).to_string()
    }
}

#[derive(Debug)]
pub struct BlockQuote {
    widget: gtk::Label,
}

impl BlockQuote {
    pub fn new(ctx: &BlockCtx, text: &str) -> Self {
        let widget = gtk::Label::new(Some(text));
        widget.set_wrap_mode(pango::WrapMode::WordChar);
        widget.set_wrap(true);
        widget.add_css_class("block-quote");
        Self { widget }
    }
}

impl Block for BlockQuote {
    fn activate_search(&self, text: &str) -> Box<dyn Iterator<Item = ()>> {
        Box::new(std::iter::empty())
    }
    fn clear_search(&self) {
        // Do nothing
    }
    fn widget(&self) -> &gtk::Widget {
        self.widget.upcast_ref()
    }
    fn text(&self) -> String {
        self.widget.text().to_string()
    }
}

#[derive(Debug)]
pub struct UnorderedList {
    widget: gtk::ListBox,
}

impl UnorderedList {
    pub fn new(ctx: &BlockCtx) -> Self {
        let widget = gtk::ListBox::new();
        Self { widget }
    }
    pub fn append(&self, text: &str) {
        let widget = gtk::Label::new(Some(text));
        widget.set_wrap_mode(pango::WrapMode::WordChar);
        widget.set_wrap(true);
        self.widget.append(&widget);
    }
}

impl Block for UnorderedList {
    fn activate_search(&self, text: &str) -> Box<dyn Iterator<Item = ()>> {
        Box::new(std::iter::empty())
    }
    fn clear_search(&self) {
        // Do nothing
    }
    fn widget(&self) -> &gtk::Widget {
        self.widget.upcast_ref()
    }
    fn text(&self) -> String {
        let mut text = String::new();

        text
    }
}

pub struct Gemini {
    surface: hypertext::Surface,
    tag_stack: Vec<gemini::Tag>,
    links: HashMap<gtk::TextTag, String>,
}

impl Gemini {
    pub fn new() -> Self {
        Self {
            surface: hypertext::Surface::new(crate::config::DEFAULT_CONFIG.clone()),
            tag_stack: Vec::new(),
            links: HashMap::new(),
        }
    }
    pub fn render<'e>(
        &mut self,
        base_url: &url::Url,
        tokens: impl Iterator<Item = gemini::Event<'e>>,
    ) -> anyhow::Result<()> {
        for ev in tokens {
            let parent_tag = self.tag_stack.last();
            match ev {
                Event::Start(t) => {
                    let buffer = self.surface.text_view.buffer();
                    match &t {
                        gemini::Tag::Item => {
                            buffer.insert(&mut buffer.end_iter(), " •  ");
                        }
                        gemini::Tag::Link(url, label) => {
                            let link_char = if let Ok(true) = self
                                .parse_link(base_url, url)
                                .map(|url| ["gemini", "about"].contains(&url.scheme()))
                            {
                                "⇒"
                            } else {
                                "⇗"
                            };
                            let label = format!("{link_char} {}", label.as_deref().unwrap_or(url));
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
                            self.links.insert(tag.clone(), url.clone());
                        }
                        _ => {}
                    }
                    self.tag_stack.push(t);
                }
                Event::End => {
                    let buffer = self.surface.text_view.buffer();
                    let parent_tag = parent_tag.context("Missing parent tag")?;
                    match parent_tag {
                        gemini::Tag::Paragraph
                        | gemini::Tag::Link(_, _)
                        | gemini::Tag::CodeBlock
                        | gemini::Tag::Heading(_)
                        | gemini::Tag::Item => {
                            buffer.insert(&mut buffer.end_iter(), "\n");
                        }
                        _ => {}
                    }
                    self.tag_stack.pop();
                }
                Event::Text(text) => {
                    let buffer = self.surface.text_view.buffer();
                    match parent_tag.context("Missing parent tag")? {
                        gemini::Tag::CodeBlock => {
                            buffer.insert_with_tags_by_name(&mut buffer.end_iter(), text, &["pre"]);
                        }
                        gemini::Tag::BlockQuote => {
                            buffer.insert_with_tags_by_name(&mut buffer.end_iter(), text, &["q"]);
                        }
                        gemini::Tag::Heading(lvl) if (0..6u8).contains(lvl) => {
                            let tag = format!("h{lvl}");
                            buffer.insert_with_tags_by_name(&mut buffer.end_iter(), text, &[&tag]);
                        }
                        gemini::Tag::Item => {
                            buffer.insert_with_tags_by_name(&mut buffer.end_iter(), text, &["p"]);
                        }
                        _ => buffer.insert_with_tags_by_name(&mut buffer.end_iter(), text, &["p"]),
                    }
                }
                Event::BlankLine => {
                    let buffer = self.surface.text_view.buffer();
                    buffer.insert(&mut buffer.end_iter(), "\n");
                }
            }
        }
        Ok(())
    }
    fn parse_link(&self, base_url: &url::Url, link: &str) -> Result<Url, url::ParseError> {
        let link_url = Url::options().base_url(Some(base_url)).parse(link)?;
        Ok(link_url)
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
            .iter_at_location(x, y)
            .context("Can't get text iter where clicked")?;

        iter.tags()
            .iter()
            .find_map(|x| x.name().is_none().then(|| m.get_key_value(x)).flatten())
            .map(|(k, v)| (k, v.as_str()))
            .ok_or_else(|| anyhow::Error::msg("Clicked text doesn't have a link tag"))
    }
}
/*
impl Block for Gemini {
    fn activate_search(&self, text: &str) -> Box<dyn Iterator<Item = ()>> {
        Box::new(std::iter::empty())
    }
    fn clear_search(&self) {
        // Do nothing
    }
    fn widget(&self) -> &gtk::Widget {
        self.surface.root().upcast_ref()
    }
    fn text(&self) -> String {
        let buffer = self.surface.text_view.buffer();
        let start = buffer.start_iter();
        let end = buffer.end_iter();
        buffer.text(&start, &end, true).to_string()
    }
}*/

mod imp {
    use std::cell::{Cell, RefCell};

    pub use super::*;
    #[derive(Default)]
    pub struct BlocksPage {
        pub last_row: Cell<usize>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BlocksPage {
        // `NAME` needs to match `class` attribute of template
        const NAME: &'static str = "BlocksPage";
        type Type = super::BlocksPage;
        type ParentType = gtk::Grid;
    }

    impl ObjectImpl for BlocksPage {}
    impl WidgetImpl for BlocksPage {}
    impl GridImpl for BlocksPage {}
}

glib::wrapper! {
    pub struct BlocksPage(ObjectSubclass<imp::BlocksPage>)
    @extends gtk::Grid, gtk::Widget;
}

impl BlocksPage {
    pub fn new() -> Self {
        let this: Self = glib::Object::new();
        this.add_css_class("blocks-page");
        this
    }
    pub fn append(&self, child: &(impl Block + ?Sized)) {
        self.attach(child.widget(), 0, self.imp().last_row.get() as i32, 1, 1);
        self.imp().last_row.replace(self.imp().last_row.get() + 1);
    }
}
impl Default for BlocksPage {
    fn default() -> Self {
        Self::new()
    }
}

fn parse_paragraph<'a>(
    events: &mut (impl Iterator<Item = gemini::Event<'a>>),
    ctx: &BlockCtx,
) -> Option<Box<dyn Block>> {
    let mut text = String::new();
    while let Some(ev) = events.next() {
        match ev {
            Event::Start(Tag::Paragraph) => {}
            Event::Text(t) => text.push_str(t),
            Event::End => {
                return Some(Box::new(Text::new(ctx, &text)));
            }
            _ => return None,
        }
    }
    None
}

fn parse_link<'a>(
    events: &mut (impl Iterator<Item = gemini::Event<'a>>),
    ctx: &BlockCtx,
    url: &str,
    label: Option<&str>,
) -> Option<Box<dyn Block>> {
    assert!(matches!(
        events.next().unwrap(),
        Event::Start(Tag::Link(_, _))
    ));
    assert!(matches!(events.next().unwrap(), Event::End));
    Some(Box::new(Link::new(ctx, &url, label.as_deref())))
}
pub fn parse_gemini_to_blocks<'a>(
    events: (impl Iterator<Item = gemini::Event<'a>> + 'a),
    ctx: &'a mut BlockCtx,
) -> impl Iterator<Item = Box<dyn Block>> + 'a {
    let mut events = events.peekable();

    std::iter::from_fn(move || {
        let res = match events.peek() {
            Some(&Event::Start(Tag::Paragraph)) => parse_paragraph(&mut events, &ctx),
            Some(&Event::Start(Tag::Heading(lvl))) => parse_heading(&mut events, &ctx, lvl),
            Some(&Event::Start(Tag::Link(ref url, ref label))) => {
                let url = url.clone();
                let label = label.clone();
                parse_link(&mut events, &ctx, url.as_ref(), label.as_deref())
            }
            Some(&Event::Start(Tag::CodeBlock)) => parse_codeblock(&mut events, &ctx),
            Some(&Event::Start(Tag::BlockQuote)) => parse_blockquote(&mut events, &ctx),
            Some(&Event::Start(Tag::UnorderedList)) => parse_unordered_list(&mut events, &ctx),
            Some(&Event::BlankLine) => None,
            Some(ev @ (&Event::Text(_) | &Event::End | &Event::Start(Tag::Item))) => {
                log::error!(
                    "Unexpected text event, should've been handled by a subparser, got {:?}",
                    ev
                );
                None
            }
            None => None,
        };
        if res.is_some() {
            ctx.count += 1;
        }
        res
    })
}

fn parse_unordered_list<'a>(
    events: &mut (impl Iterator<Item = gemini::Event<'a>>),
    ctx: &&mut BlockCtx,
) -> Option<Box<dyn Block>> {
    let mut text = String::new();
    let mut list = UnorderedList::new(ctx);
    let mut inside_item = false;
    while let Some(ev) = events.next() {
        dbg!(&ev);
        match ev {
            Event::Start(Tag::UnorderedList) => {
            }
            Event::Start(Tag::Item) => {
                inside_item = true;
                text.clear();
            }
            Event::Text(t) => text.push_str(t),
            Event::End => {
                if inside_item {
                    inside_item = false;
                    list.append(&text);
                } else {
                    return Some(Box::new(list));
                }
            }
            _ => {}
        }
    }
    None
}

fn parse_blockquote<'a>(
    events: &mut (impl Iterator<Item = gemini::Event<'a>>),
    ctx: &BlockCtx,
) -> Option<Box<dyn Block>> {
    let mut text = String::new();
    while let Some(ev) = events.next() {
        match ev {
            Event::Start(Tag::BlockQuote) => {}
            Event::Text(t) => text.push_str(t),
            Event::End => {
                return Some(Box::new(BlockQuote::new(ctx, &text)));
            }
            _ => return None,
        }
    }
    None
}

fn parse_codeblock<'a>(
    events: &mut (impl Iterator<Item = gemini::Event<'a>>),
    ctx: &BlockCtx,
) -> Option<Box<dyn Block>> {
    let mut text = String::new();
    while let Some(ev) = events.next() {
        match ev {
            Event::Start(Tag::CodeBlock) => {}
            Event::Text(t) => text.push_str(t),
            Event::End => {
                return Some(Box::new(SourceView::new(&text)));
            }
            _ => return None,
        }
    }
    None
}

fn parse_heading<'a>(
    events: &mut (impl Iterator<Item = gemini::Event<'a>>),
    ctx: &BlockCtx,
    lvl: u8,
) -> Option<Box<dyn Block>> {
    let mut text = String::new();
    while let Some(ev) = events.next() {
        match ev {
            Event::Start(Tag::Heading(_)) => {}
            Event::Text(t) => text.push_str(t),
            Event::End => {
                return Some(Box::new(Title::new(ctx, lvl, &text)));
            }
            _ => return None, // ERROR: Unexpected tag
        }
    }
    None
}
/*
Operations I need on the BlockPage:
- Create a new BlockPage
- Add a child anywhere in the grid
    - How do I define the position of the child?
      With column, row, width, height
- Search text
  Every block must provide an activate_search method, highlighting the text found and returning a cursor
  to focus the found text
- Clear search
- Select text
  Every block must provide a text() method to get the entire text of the block
- Clear selection

*/
