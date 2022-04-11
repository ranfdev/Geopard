use adw::prelude::*;
use adw::subclass::application_window::AdwApplicationWindowImpl;
use anyhow::Context;
use futures::prelude::*;
use glib::clone;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::subclass::prelude::*;
use log::{error, info, warn};
use std::cell::RefCell;
use url::Url;

use crate::common::{bookmarks_url, glibctx, BOOKMARK_FILE_PATH};
use crate::config;
use crate::tab::Tab;

pub mod imp {
    use super::*;
    #[derive(Debug, Default)]
    pub struct Window {
        pub(crate) url_bar: gtk::SearchEntry,
        pub(crate) progress_bar: gtk::ProgressBar,
        pub(crate) back_btn: gtk::Button,
        pub(crate) add_bookmark_btn: gtk::Button,
        pub(crate) show_bookmarks_btn: gtk::Button,
        pub(crate) tab_bar: adw::TabBar,
        pub(crate) tab_view: adw::TabView,
        pub(crate) config: RefCell<config::Config>,
        pub(crate) add_tab_btn: gtk::Button,
        pub(crate) progress_animation: RefCell<Option<adw::SpringAnimation>>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "GeopardWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;
    }

    impl ObjectImpl for Window {}
    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}
glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
    @extends adw::ApplicationWindow, gtk::Window,
    @implements gio::ActionMap, gio::ActionGroup;
}

macro_rules! self_action {
    ($self:ident, $name:literal, $method:ident) => {
        {
            let this = &$self;
            let action = gio::SimpleAction::new($name, None);
            action.connect_activate(clone!(@weak this => move |_,_| this.$method()));
            $self.add_action(&action);
        }
    }
}
impl Window {
    pub fn new(app: &adw::Application, config: config::Config) -> Self {
        let this: Self = glib::Object::new(&[("application", app)]).unwrap();
        let imp = this.imp();
        imp.config.replace(config);

        let content = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let header_bar = gtk::HeaderBar::new();
        header_bar.set_show_title_buttons(true);

        imp.back_btn.set_icon_name("go-previous-symbolic");
        imp.add_bookmark_btn.set_icon_name("star-new-symbolic");
        imp.show_bookmarks_btn.set_icon_name("view-list-symbolic");
        imp.add_tab_btn.set_icon_name("tab-new-symbolic");

        header_bar.pack_start(&imp.back_btn);
        header_bar.pack_start(&imp.add_tab_btn);
        header_bar.pack_end(&imp.add_bookmark_btn);
        header_bar.pack_end(&imp.show_bookmarks_btn);

        imp.url_bar.set_hexpand(true);

        header_bar.set_title_widget(Some(&imp.url_bar));

        content.append(&header_bar);

        imp.tab_bar.set_view(Some(&imp.tab_view));
        content.append(&imp.tab_bar);

        imp.progress_bar.add_css_class("osd");
        content.append(&imp.progress_bar);
        content.append(&imp.tab_view);

        this.set_default_size(800, 600);
        this.set_content(Some(&content));

        this.bind_signals();
        this.setup_actions();
        this.add_tab();
        this.open_url(bookmarks_url());
        this
    }

    fn setup_actions(&self) {
        self_action!(self, "back", back);
        self_action!(self, "new-tab", add_tab);
        self_action!(self, "show-bookmarks", add_tab);
        self_action!(self, "bookmark-current", bookmark_current);
        self_action!(self, "close-tab", close_tab);
        self_action!(self, "focus-url-bar", focus_url_bar);

        let act_open_page = gio::SimpleAction::new("open-omni", Some(glib::VariantTy::STRING));
        act_open_page.connect_activate(
            clone!(@weak self as this => move |_,v| this.open_omni(v.unwrap().get::<String>().unwrap().as_str())),
        );
        self.add_action(&act_open_page);

        let act_open_url = gio::SimpleAction::new("open-url", Some(glib::VariantTy::STRING));
        act_open_url.connect_activate(
            clone!(@weak self as this => move |_,v| this.open_url_str(v.unwrap().get::<String>().unwrap().as_str())),
        );
        self.add_action(&act_open_url);

        let act_open_in_new_tab =
            gio::SimpleAction::new("open-in-new-tab", Some(glib::VariantTy::STRING));
        act_open_in_new_tab.connect_activate(
            clone!(@weak self as this => move |_,v| this.open_in_new_tab(v.unwrap().get::<String>().unwrap().as_str())),
        );
        self.add_action(&act_open_in_new_tab);

        let act_set_clipboard =
            gio::SimpleAction::new("set-clipboard", Some(glib::VariantTy::STRING));
        act_set_clipboard.connect_activate(
            clone!(@weak self as this => move |_,v| this.set_clipboard(v.unwrap().get::<String>().unwrap().as_str())),
        );
        self.add_action(&act_set_clipboard);
    }
    fn add_tab(&self) {
        let imp = self.imp();
        let tab = Tab::new(imp.config.borrow().clone());
        let tab_view = imp.tab_view.clone();
        tab.connect_local("title-changed", false, move |values| {
            let title: String = values[1].get().unwrap();
            let gtab: Tab = values[0].get().unwrap();
            let page = tab_view.page(&gtab);
            page.set_title(&title);
            None
        });
        let url_bar = imp.url_bar.clone();
        tab.connect_local("url-changed", false, move |values| {
            let title: String = values[1].get().unwrap();
            url_bar.set_text(&title);
            None
        });
        tab.connect_local(
            "progress-changed",
            false,
            clone!(@weak self as this  => @default-panic, move |values| {
                let p: f64 = values[1].get().unwrap();
                this.set_progress(p);
                None
            }),
        );

        let w = imp.tab_view.append(&tab);
        imp.tab_view.set_selected_page(&w);
        self.open_url(bookmarks_url());
    }
    fn close_tab(&self) {
        let imp = self.imp();
        imp.tab_view
            .close_page(&imp.tab_view.page(&self.current_tab()));
    }
    fn focus_url_bar(&self) {
        let imp = self.imp();
        imp.url_bar.grab_focus();
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
        let imp = self.imp();
        imp.tab_view
            .selected_page()
            .unwrap()
            .child()
            .downcast()
            .unwrap()
    }
    fn set_progress(&self, progress: f64) {
        let imp = self.imp();
        info!("progress {}", progress);
        if let Some(animation) = imp.progress_animation.borrow().as_ref() {
            animation.pause();
        }
        if progress == 0.0 {
            imp.progress_bar.set_fraction(0.0);
            return;
        }
        let progress_bar = imp.progress_bar.clone();
        let animation = adw::SpringAnimation::new(
            &imp.progress_bar,
            imp.progress_bar.fraction(),
            progress,
            &adw::SpringParams::new(1.0, 1.0, 100.0),
            &adw::CallbackAnimationTarget::new(Some(Box::new(move |v| {
                progress_bar.set_fraction(v);
                progress_bar.set_opacity(1.0 - v);
            }))),
        );
        animation.play();
        imp.progress_animation.replace(Some(animation));
    }
    fn open_url(&self, url: Url) {
        self.current_tab().spawn_open(url);
    }
    fn back(&self) {
        match self.current_tab().back() {
            Err(e) => warn!("{}", e),
            Ok(_) => info!("went back"),
        }
    }
    fn bookmark_current(&self) {
        let imp = self.imp();
        let url = imp.url_bar.text().to_string();
        glibctx().spawn_local(async move {
            match Self::append_bookmark(&url).await {
                Ok(_) => info!("{} saved to bookmarks", url),
                Err(e) => error!("{}", e),
            }
        });
    }
    // this should also handle search requests
    fn open_omni(&self, v: &str) {
        let url = Url::parse(v);
        match url {
            Ok(url) => self.open_url(url),
            Err(e) => error!("Failed to parse url: {:?}", e),
        }
    }
    fn open_url_str(&self, v: &str) {
        let url = Url::parse(v);
        match url {
            Ok(url) => self.open_url(url),
            Err(e) => error!("Failed to parse url: {:?}", e),
        }
    }
    fn open_in_new_tab(&self, v: &str) {
        self.add_tab();
        self.open_url_str(v);
    }
    fn set_clipboard(&self, v: &str) {
        gdk::Display::default().unwrap().clipboard().set_text(v);
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
        let imp = self.imp();
        imp.url_bar.connect_activate(|url_bar| {
            url_bar
                .activate_action("win.open-omni", Some(&url_bar.text().to_variant()))
                .unwrap();
        });
        imp.back_btn.set_action_name(Some("win.back"));
        imp.add_tab_btn.set_action_name(Some("win.new-tab"));
        imp.add_bookmark_btn
            .set_action_name(Some("win.bookmark-current"));
        imp.show_bookmarks_btn
            .set_action_name(Some("win.show-bookmarks"))
    }
}
