use crate::common::{bookmarks_url, glibctx, BOOKMARK_FILE_PATH};
use crate::component::{new_component_id, Component};
use crate::config;
use crate::tab::{Tab, TabMsg};
use anyhow::Context;

use adw::prelude::*;
use futures::prelude::*;
use futures::task::LocalSpawnExt;
use gtk::prelude::*;
use log::{debug, error, info, warn};
use url::Url;

pub enum WindowMsg {
    Open(url::Url),
    OpenNewTab(url::Url),
    AddTab,
    UrlBarActivated,
    SwitchTab(Tab),
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
    tabs: Vec<adw::TabPage>,
    current_tab: usize,
    tab_view: adw::TabView,
    config: config::Config,
    add_tab_btn: gtk::Button,
}
impl Window {
    pub fn new(
        app: &adw::Application,
        config: config::Config,
    ) -> Component<adw::ApplicationWindow, WindowMsg> {
        let window = adw::ApplicationWindow::new(app);
        let view = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let header_bar = gtk::HeaderBar::new();
        header_bar.set_show_title_buttons(true);

        let btn_box = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        let back_btn = gtk::Button::from_icon_name("go-previous-symbolic");
        let add_bookmark_btn = gtk::Button::from_icon_name("star-new-symbolic");
        let show_bookmarks_btn = gtk::Button::from_icon_name("view-list-symbolic");
        let add_tab_btn = gtk::Button::from_icon_name("document-new-symbolic");

        btn_box.append(&back_btn);
        btn_box.append(&add_tab_btn);

        header_bar.pack_start(&btn_box);
        header_bar.pack_end(&add_bookmark_btn);
        header_bar.pack_end(&show_bookmarks_btn);

        let url_bar = gtk::SearchEntry::new();
        url_bar.set_hexpand(true);

        header_bar.set_title_widget(Some(&url_bar));

        view.append(&header_bar);

        window.set_content(Some(&view));
        window.set_default_size(800, 600);

        let tab_bar = adw::TabBar::new();
        let tab_view = adw::TabView::new();
        tab_bar.set_view(Some(&tab_view));

        view.append(&tab_bar);
        view.append(&tab_view);

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
            tab_view,
            config,
            add_tab_btn,
        };

        this.bind_signals();
        this.add_tab();
        this.open_url(bookmarks_url());

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

    fn add_tab(&mut self) {
        let tab = Tab::new(self.config.clone());
        let tab_view = self.tab_view.clone();
        tab.connect_local("title-changed", false, move |values| {
            let title: String = values[1].get().unwrap();
            let gtab: Tab = values[0].get().unwrap();
            let page = tab_view.page(&gtab);
            page.set_title(&title);
            None
        });
        let url_bar = self.url_bar.clone();
        tab.connect_local("url-changed", false, move |values| {
            let title: String = values[1].get().unwrap();
            url_bar.set_text(&title);
            None
        });

        let w = self.tab_view.append(&tab);
        self.tabs.push(w.clone());
        self.current_tab = self.tabs.len() - 1;
        self.tab_view.set_selected_page(&w);
        self.open_url(bookmarks_url());
    }

    fn handle_msg(&mut self, msg: WindowMsg) {
        match msg {
            WindowMsg::Open(url) => self.open_url(url),
            WindowMsg::OpenNewTab(url) => self.msg_open_new_tab(url),
            WindowMsg::Back => self.msg_back(),
            WindowMsg::SwitchTab(tab) => self.msg_switch_tab(tab),
            WindowMsg::AddTab => self.msg_add_tab(),
            WindowMsg::BookmarkCurrent => self.msg_bookmark_current(),
            WindowMsg::UrlBarActivated => self.msg_url_bar_activated(),
            WindowMsg::SetProgress(tab_id, n) => self.msg_set_progress(tab_id, n),
        }
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
    fn current_tab(&self) -> Tab {
        self.tab_view
            .selected_page()
            .unwrap()
            .child()
            .downcast()
            .unwrap()
    }
    fn msg_set_progress(&mut self, tab_id: usize, progress: f64) {
        // FIXME: self.url_bar.set_progress_fraction(progress);
    }
    fn msg_open_new_tab(&mut self, url: Url) {
        let new_tab = self.add_tab();
    }
    fn open_url(&mut self, url: Url) {
        self.current_tab().spawn_open(url);
    }
    fn msg_back(&mut self) {
        match self.current_tab().back() {
            Err(e) => warn!("{}", e),
            Ok(_) => info!("went back"),
        }
    }
    fn msg_switch_tab(&mut self, tab: Tab) {
        self.url_bar.set_text(tab.url().unwrap().as_str());
    }
    fn msg_add_tab(&mut self) {
        self.add_tab();
    }
    fn msg_bookmark_current(&mut self) {
        let url = self.url_bar.text().to_string();
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
        let url = Url::parse(self.url_bar.text().as_str());
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
    //        &gdk::Screen::default().unwrap(),
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
        self.tab_view.connect_selected_page_notify(move |tab_view| {
            let sender_clone = sender_clone.clone();
            let tab: Tab = tab_view
                .selected_page()
                .unwrap()
                .child()
                .downcast()
                .unwrap();

            sender_clone.send(WindowMsg::SwitchTab(tab)).unwrap();
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
