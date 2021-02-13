use crate::common::{bookmarks_url, glibctx, BOOKMARK_FILE_PATH};
use crate::component::{new_component_id, Component};
use crate::config;
use crate::tab::{Tab, TabMsg};
use anyhow::Context;

use futures::prelude::*;
use futures::task::LocalSpawnExt;
use gtk::prelude::*;
use log::{debug, error, info};
use url::Url;

pub enum WindowMsg {
    Open(url::Url),
    OpenNewTab(url::Url),
    AddTab,
    UrlBarActivated,
    CloseTab(usize),
    SwitchTab(usize),
    UpdateUrlBar(usize, Url),
    BookmarkCurrent,
    Back,
    SetProgress(usize, f64),
}

pub struct Window {
    sender: flume::Sender<WindowMsg>,
    url_bar: gtk::SearchEntry,
    back_btn: gtk::Button,
    add_bookmark_btn: gtk::Button,
    show_bookmarks_btn: gtk::Button,
    tabs: Vec<Component<gtk::ScrolledWindow, TabMsg>>,
    current_tab: usize,
    notebook: gtk::Notebook,
    config: config::Config,
    add_tab_btn: gtk::Button,
}
impl Window {
    pub fn new(
        app: &gtk::Application,
        config: config::Config,
    ) -> Component<gtk::ApplicationWindow, WindowMsg> {
        let window = gtk::ApplicationWindow::new(app);
        let view = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let header_bar = gtk::HeaderBar::new();
        header_bar.set_title(Some("Geopard"));
        header_bar.set_show_close_button(true);

        let btn_box = gtk::ButtonBox::new(gtk::Orientation::Horizontal);
        btn_box.set_property_layout_style(gtk::ButtonBoxStyle::Expand);
        let back_btn =
            gtk::Button::from_icon_name(Some("go-previous-symbolic"), gtk::IconSize::Button);
        let add_bookmark_btn =
            gtk::Button::from_icon_name(Some("star-new-symbolic"), gtk::IconSize::Button);

        let show_bookmarks_btn =
            gtk::Button::from_icon_name(Some("view-list-symbolic"), gtk::IconSize::Button);

        btn_box.add(&back_btn);
        btn_box.add(&add_bookmark_btn);
        btn_box.add(&show_bookmarks_btn);

        header_bar.pack_start(&btn_box);

        let url_bar = gtk::SearchEntry::new();
        url_bar.set_hexpand(true);

        header_bar.set_custom_title(Some(&url_bar));

        window.set_titlebar(Some(&header_bar));
        window.add(&view);

        window.set_default_size(800, 600);

        let notebook = gtk::Notebook::new();
        let add_tab_btn =
            gtk::Button::from_icon_name(Some("document-new-symbolic"), gtk::IconSize::Menu);

        notebook.set_action_widget(&add_tab_btn, gtk::PackType::End);
        notebook.set_scrollable(true);

        view.add(&notebook);
        add_tab_btn.show_all();

        let (sender, receiver): (flume::Sender<WindowMsg>, flume::Receiver<WindowMsg>) =
            flume::unbounded();
        let tabs = vec![];

        let mut this = Self {
            url_bar,
            back_btn,
            add_bookmark_btn,
            show_bookmarks_btn,
            sender: sender.clone(),
            current_tab: 0,
            tabs,
            notebook,
            config,
            add_tab_btn,
        };

        this.bind_signals();
        this.add_tab();

        let receiver: flume::Receiver<WindowMsg> = receiver;
        let handle = glibctx()
            .spawn_local_with_handle(async move {
                while let Ok(msg) = receiver.recv_async().await {
                    this.handle_msg(msg);
                }
            })
            .unwrap();

        Component::new(new_component_id(), window, sender, handle)
    }

    fn gen_tab_label(&self, id: usize, url: Url) -> gtk::Box {
        let tab_box = gtk::Box::new(gtk::Orientation::Horizontal, 0);
        let tab_label = gtk::Label::new(Some(url.as_str()));
        tab_label.set_hexpand(true);
        tab_label.set_ellipsize(pango::EllipsizeMode::Start);
        tab_label.set_width_chars(12);
        let tab_action =
            gtk::Button::from_icon_name(Some("window-close-symbolic"), gtk::IconSize::Menu);

        let style_ctx = tab_action.get_style_context();
        style_ctx.add_class("flat");
        style_ctx.add_class("small-button");

        tab_box.add(&tab_label);
        tab_box.pack_end(&tab_action, false, false, 5);
        tab_box.show_all();

        let sender = self.sender.clone();
        tab_action.connect_clicked(move |_| {
            let sender = sender.clone();
            sender.send(WindowMsg::CloseTab(id)).unwrap();
        });

        tab_box
    }
    fn add_tab(&mut self) -> flume::Sender<TabMsg> {
        let tab = Tab::new(self.config.clone(), self.sender.clone());
        let handler = tab.run();
        let sender = handler.chan();
        let widget = handler.widget().clone();
        self.tabs.push(handler);

        let label = gtk::Label::new(Some("tab"));
        self.notebook.append_page(&widget, Some(&label));

        self.notebook.show_all();
        sender.send(TabMsg::Open(bookmarks_url())).unwrap();
        sender
    }

    async fn append_bookmark(url: &str) -> anyhow::Result<()> {
        let mut file = async_fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open(&*BOOKMARK_FILE_PATH)
            .await
            .context("Opening bookmark.gemini")?;

        let line_to_write = format!("=> {}\n", url);
        file.write_all(line_to_write.as_bytes())
            .await
            .context("Writing url to favourite.gemini")?;

        file.flush().await?;
        Ok(())
    }
    fn current_tab(&self) -> &Component<gtk::ScrolledWindow, TabMsg> {
        &self.tabs[self.current_tab]
    }
    fn handle_msg(&mut self, msg: WindowMsg) {
        match msg {
            WindowMsg::Open(url) => self.msg_open(url),
            WindowMsg::OpenNewTab(url) => self.msg_open_new_tab(url),
            WindowMsg::Back => self.msg_back(),
            WindowMsg::UpdateUrlBar(tab_id, url) => self.msg_update_url_bar(tab_id, url),
            WindowMsg::SwitchTab(n) => self.msg_switch_tab(n),
            WindowMsg::CloseTab(widget) => self.msg_close_tab(widget),
            WindowMsg::AddTab => self.msg_add_tab(),
            WindowMsg::BookmarkCurrent => self.msg_bookmark_current(),
            WindowMsg::UrlBarActivated => self.msg_url_bar_activated(),
            WindowMsg::SetProgress(tab_id, n) => self.msg_set_progress(tab_id, n),
        }
    }
    fn msg_set_progress(&mut self, tab_id: usize, progress: f64) {
        if self.current_tab().id() == tab_id {
            self.url_bar.set_progress_fraction(progress);
        }
    }
    fn msg_open_new_tab(&mut self, url: Url) {
        let new_tab = self.add_tab();
        new_tab.send(TabMsg::Open(url)).unwrap();
    }
    fn msg_open(&mut self, url: Url) {
        let chan = self.tabs[self.current_tab].chan();
        chan.send(TabMsg::Open(url)).unwrap();
    }
    fn msg_back(&mut self) {
        let chan = self.tabs[self.current_tab].chan();
        chan.send(TabMsg::Back).unwrap();
    }
    fn tab_by_id(&self, id: usize) -> Option<&Component<gtk::ScrolledWindow, TabMsg>> {
        self.tabs.iter().find(|t| t.id() == id)
    }
    fn msg_update_url_bar(&mut self, tab_id: usize, url: Url) {
        if self.current_tab().id() == tab_id {
            self.url_bar.set_text(url.as_str());
        }
        if let Some(tab) = self.tab_by_id(tab_id) {
            let tab_widget = tab.widget();
            self.notebook
                .set_tab_label(tab_widget, Some(&self.gen_tab_label(tab_id, url)));
        }
    }
    fn msg_switch_tab(&mut self, n: usize) {
        self.current_tab = n;
        self.notebook.set_current_page(Some(n as u32));
        let chan = self.tabs[self.current_tab].chan();
        chan.send(TabMsg::GetUrl).unwrap();
        chan.send(TabMsg::GetProgress).unwrap();
    }
    fn msg_close_tab(&mut self, tab_id: usize) {
        let tab = self.tabs.iter().position(|tab| tab.id() == tab_id);
        self.notebook.remove_page(tab.map(|x| x as u32));
        self.tabs.remove(tab.unwrap());

        if self.tabs.is_empty() {
            self.msg_add_tab()
        }

        self.current_tab = self.tabs.len() - 1;
    }
    fn msg_add_tab(&mut self) {
        self.add_tab();
        self.sender.send(WindowMsg::SwitchTab(self.tabs.len() - 1)).unwrap();
    }
    fn msg_bookmark_current(&mut self) {
        let url = self.url_bar.get_text().to_string();
        let sender = self.sender.clone();
        glibctx().spawn_local(async move {
            match Self::append_bookmark(&url).await {
                Ok(_) => info!("{} saved to bookmarks", url),
                Err(e) => error!("{}", e),
            }
            sender.send(WindowMsg::AddTab).unwrap();
        });
    }
    fn msg_url_bar_activated(&mut self) {
        let url = Url::parse(self.url_bar.get_text().as_str());
        match url {
            Ok(url) => self.sender.send(WindowMsg::Open(url)).unwrap(),
            Err(e) => error!("Failed to parse url from urlbar: {:?}", e),
        }
    }
    //TODO: Reintroduce colors
    //fn set_special_color_from_hash(&self, hash: u64) {
    //    let color1 = Color(
    //        (hash & 255) as u8,
    //        (hash >> 8 & 255) as u8,
    //        (hash >> 16 & 255) as u8,
    //    );

    //    let hash = hash >> 24;
    //    let color2 = Color(
    //        (hash & 255) as u8,
    //        (hash >> 8 & 255) as u8,
    //        (hash >> 16 & 255) as u8,
    //    );

    //    let stylesheet = format!(
    //        "
    //        headerbar {{
    //            transition: 500ms;
    //            background: linear-gradient(#{:x}, #{:x});
    //        }}
    //        text {{
    //            transition: 500ms;
    //            background: rgba({},{},{}, 0.05);
    //        }}
    //        ",
    //        color1, color2, color2.0, color2.1, color2.2
    //    );
    //    Self::add_stylesheet(&stylesheet);
    //}
    //fn add_stylesheet(stylesheet: &str) {
    //    // TODO: Adding a provider and keeping it in memory forever
    //    // is a memory leak. Fortunately, it's small

    //    let provider = gtk::CssProvider::new();
    //    provider
    //        .load_from_data(stylesheet.as_bytes())
    //        .expect("Failed loading stylesheet");
    //    gtk::StyleContext::add_provider_for_screen(
    //        &gdk::Screen::get_default().unwrap(),
    //        &provider,
    //        gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
    //    );
    //}

    fn bind_signals(&self) {
        let sender = self.sender.clone();

        let sender_clone = sender.clone();
        self.url_bar.connect_activate(move |_| {
            sender_clone.send(WindowMsg::UrlBarActivated).unwrap();
        });

        let sender_clone = sender.clone();
        self.back_btn.connect_clicked(move |_| {
            sender_clone.send(WindowMsg::Back).unwrap();
        });

        let sender_clone = sender.clone();
        self.add_tab_btn.connect_clicked(move |_| {
            debug!("Clicked add tab...");
            sender_clone.send(WindowMsg::AddTab).unwrap();
        });

        let sender_clone = sender.clone();
        self.notebook
            .connect_switch_page(move |_notebook, _page, n| {
                let sender_clone = sender_clone.clone();
                sender_clone.send(WindowMsg::SwitchTab(n as usize)).unwrap();
            });

        let sender_clone = sender.clone();
        self.add_bookmark_btn.connect_clicked(move |_| {
            sender_clone.send(WindowMsg::BookmarkCurrent).unwrap();
        });

        let sender_clone = sender.clone();
        self.show_bookmarks_btn.connect_clicked(move |_| {
            sender_clone.send(WindowMsg::AddTab).unwrap();
        });
    }
}
