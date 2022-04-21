use adw::prelude::*;
use adw::subclass::application_window::AdwApplicationWindowImpl;
use anyhow::Context;
use futures::prelude::*;
use glib::clone;
use glib_macros::Properties;
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::subclass::prelude::*;
use log::{error, info, warn};
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use url::Url;
use gtk::prelude::*;
use gtk::CompositeTemplate;
use gtk::TemplateChild;

use crate::common::{bookmarks_url, glibctx, BOOKMARK_FILE_PATH};
use crate::config;
use crate::tab::HistoryStatus;
use crate::tab::Tab;
use crate::{self_action, view};

pub mod imp {
    use super::*;
    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[template(resource = "/com/ranfdev/Geopard/ui/window.ui")]
    pub struct Window {
        #[template_child]
        pub(crate) url_bar: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(crate) small_url_bar: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(crate) bottom_bar_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub(crate) header_small: TemplateChild<adw::HeaderBar>,
        #[template_child]
        pub(crate) squeezer: TemplateChild<adw::Squeezer>,
        #[template_child]
        pub(crate) progress_bar: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub(crate) tab_view: TemplateChild<adw::TabView>,
        pub(crate) config: RefCell<config::Config>,
        pub(crate) progress_animation: RefCell<Option<adw::SpringAnimation>>,
        pub(crate) binded_tab_properties: RefCell<Vec<glib::Binding>>,
        #[prop(get, set)]
        pub(crate) url: RefCell<String>,
        #[prop(get = Self::progress_animated, set = Self::set_progress_animated)]
        pub(crate) progress: PhantomData<f64>,
        pub(crate) scroll_ctrl: gtk::EventControllerScroll,
        pub(crate) action_previous: RefCell<Option<gio::SimpleAction>>,
        pub(crate) action_next: RefCell<Option<gio::SimpleAction>>,
    }

    impl Window {
        fn progress_animated(&self) -> f64 {
            self.progress_animation
                .borrow()
                .as_ref()
                .map(|a| a.value_to())
                .unwrap_or(0.0)
        }
        fn set_progress_animated(&self, progress: f64) {
            if let Some(animation) = self.progress_animation.borrow().as_ref() {
                animation.pause()
            }
            if progress == 0.0 {
                self.progress_bar.set_fraction(0.0);
                return;
            }
            let progress_bar = self.progress_bar.clone();
            let animation = adw::SpringAnimation::new(
                &*self.progress_bar,
                self.progress_bar.fraction(),
                progress,
                &adw::SpringParams::new(1.0, 1.0, 100.0),
                &adw::CallbackAnimationTarget::new(Some(Box::new(move |v| {
                    progress_bar.set_fraction(v);
                    progress_bar.set_opacity(1.0 - v);
                }))),
            );
            animation.play();
            self.progress_animation.replace(Some(animation));
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Window {
        const NAME: &'static str = "GeopardWindow";
        type Type = super::Window;
        type ParentType = adw::ApplicationWindow;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }


        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }

    }

    impl ObjectImpl for Window {
        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(
            &self,
            obj: &Self::Type,
            id: usize,
            value: &glib::Value,
            pspec: &glib::ParamSpec,
        ) {
            Self::derived_set_property(self, obj, id, value, pspec).unwrap();
        }

        fn property(&self, obj: &Self::Type, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            Self::derived_property(self, obj, id, pspec).unwrap()
        }
    }
    impl WidgetImpl for Window {}
    impl WindowImpl for Window {}
    impl ApplicationWindowImpl for Window {}
    impl AdwApplicationWindowImpl for Window {}
}
glib::wrapper! {
    pub struct Window(ObjectSubclass<imp::Window>)
    @extends adw::ApplicationWindow, gtk::Window, gtk::Widget,
    @implements gio::ActionMap, gio::ActionGroup;
}

impl Window {
    pub fn new(app: &adw::Application, config: config::Config) -> Self {
        let this: Self = glib::Object::new(&[("application", app)]).unwrap();
        let imp = this.imp();
        imp.config.replace(config);

        this.bind_signals();
        this.squeezer_changed();
        this.setup_actions_signals();
        this.open_in_new_tab(bookmarks_url().as_str());
        this
    }
    fn bind_signals(&self) {
        self.imp().tab_view.connect_selected_page_notify(clone!(@weak self as this => @default-panic, move |tab_view| {
          this.page_switched(tab_view);
        }));
        self.imp().squeezer.connect_visible_child_notify(clone!(@weak self as this => @default-panic, move |_sq| {
            this.squeezer_changed();
        }));
    }
    fn setup_actions_signals(&self) {
        let imp = self.imp();

        let action_previous = self_action!(self, "previous", previous);
        let action_next = self_action!(self, "next", next);
        imp.action_previous.borrow_mut().replace(action_previous);
        imp.action_next.borrow_mut().replace(action_next);

        self_action!(self, "new-tab", add_tab_focused);
        self_action!(self, "show-bookmarks", add_tab_focused);
        self_action!(self, "bookmark-current", bookmark_current);
        self_action!(self, "close-tab", close_tab);
        self_action!(self, "focus-url-bar", focus_url_bar);
        self_action!(self, "shortcuts", present_shortcuts);
        self_action!(self, "about", present_about);
        self_action!(self, "focus-tab-previous", focus_tab_previous);
        self_action!(self, "focus-tab-next", focus_tab_next);

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

        self.add_controller(&imp.scroll_ctrl);
        imp.scroll_ctrl
            .set_propagation_phase(gtk::PropagationPhase::Capture);
        imp.scroll_ctrl
            .set_flags(gtk::EventControllerScrollFlags::VERTICAL);
        imp.scroll_ctrl.connect_scroll(
            clone!(@weak self as this => @default-panic, move |_, _, y| {
                this.imp().bottom_bar_revealer.set_reveal_child(y < 0.0 && this.is_small_screen());
                    gtk::Inhibit(false)
            }),
        );
        self.connect_local(
            "notify::url",
            false,
            clone!(@weak self as this => @default-panic, move |_| {
                let mut s = std::collections::hash_map::DefaultHasher::new();
                let url = this.imp().url.borrow();
                let url = if let Ok(domain) = Url::parse(&url) {
                    if let Some(domain) = domain.domain() {
                        domain.to_string()
                    } else {
                        url.to_string()
                    }
                } else {
                    url.to_string()
                };
                url.hash(&mut s);
                let h = s.finish();
                Self::set_special_color_from_hash(h);
                None
            }),
        );
    }
    fn add_tab(&self) -> adw::TabPage {
        let imp = self.imp();
        let tab = Tab::new(imp.config.borrow().clone());
        let page = imp.tab_view.append(&tab);
        tab.bind_property("title", &page, "title").build();
        page
    }
    fn page_switched(&self, tab_view: &adw::TabView) {
        let imp = self.imp();
        let mut btp = imp.binded_tab_properties.borrow_mut();
        if let Some(page) = tab_view.selected_page() {
            let tab = self.inner_tab(&page);

            btp.drain(0..).for_each(|binding| binding.unbind());
            btp.extend([
                tab.bind_property("url", self, "url")
                    .flags(glib::BindingFlags::SYNC_CREATE)
                    .build(),
                tab.bind_property("progress", self, "progress")
                    .flags(glib::BindingFlags::SYNC_CREATE)
                    .build(),
                tab.bind_property(
                    "history-status",
                    imp.action_next.borrow().as_ref().unwrap(),
                    "enabled",
                )
                .flags(glib::BindingFlags::SYNC_CREATE)
                .transform_to(|_, v| {
                    let v: HistoryStatus = v.get().unwrap();
                    let res = v.current + 1 < v.available;
                    Some(res.to_value())
                })
                .build(),
                tab.bind_property(
                    "history-status",
                    imp.action_previous.borrow().as_ref().unwrap(),
                    "enabled",
                )
                .flags(glib::BindingFlags::SYNC_CREATE)
                .transform_to(|_, v| {
                    let v: HistoryStatus = v.get().unwrap();
                    let res = v.available >= 1 && v.current > 0;
                    Some(res.to_value())
                })
                .build(),
            ]);
        };
    }
    fn add_tab_focused(&self) {
        let imp = self.imp();
        let p = self.add_tab();
        self.inner_tab(&p).spawn_open_url(bookmarks_url());
        imp.tab_view.set_selected_page(&p);
    }
    fn close_tab(&self) {
        let imp = self.imp();
        imp.tab_view
            .close_page(&imp.tab_view.page(&self.current_tab()));
        if imp.tab_view.n_pages() == 0 {
            std::process::exit(0); // TODO: maybe there's a better way for gtk apps...
        }
    }
    fn focus_url_bar(&self) {
        let imp = self.imp();
        if self.is_small_screen() {
            imp.small_url_bar.grab_focus();
        } else {
            imp.url_bar.grab_focus();
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
        let imp = self.imp();
        imp.tab_view
            .selected_page()
            .unwrap()
            .child()
            .downcast()
            .unwrap()
    }
    fn previous(&self) {
        match self.current_tab().previous() {
            Err(e) => warn!("{}", e),
            Ok(_) => info!("went back"),
        }
    }
    fn next(&self) {
        match self.current_tab().next() {
            Err(e) => warn!("{}", e),
            Ok(_) => info!("went forward"),
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
            Ok(url) => self.current_tab().spawn_open_url(url),
            Err(e) => error!(
                "Failed to parse url (will trigger a search in the future): {:?}",
                e
            ),
        }
    }
    fn open_url_str(&self, v: &str) {
        let url = Url::parse(v);
        match url {
            Ok(url) => self.current_tab().spawn_open_url(url),
            Err(e) => error!("Failed to parse url: {:?}", e),
        }
    }
    fn open_in_new_tab(&self, v: &str) {
        let w = self.add_tab();
        let url = Url::parse(v);
        match url {
            Ok(url) => self.inner_tab(&w).spawn_open_url(url),
            Err(e) => error!("Failed to parse url: {:?}", e),
        }
    }
    fn set_clipboard(&self, v: &str) {
        gdk::Display::default().unwrap().clipboard().set_text(v);
    }
    fn inner_tab(&self, tab: &adw::TabPage) -> Tab {
        tab.child().downcast().unwrap()
    }
    fn set_special_color_from_hash(hash: u64) {
        let hue = hash % 360;
        let stylesheet = format!(
            "
            @define-color view_bg_color hsl({hue}, 100%, 99%);
            @define-color view_fg_color hsl({hue}, 100%, 12%);
            @define-color window_bg_color hsl({hue}, 100%, 99%);
            @define-color window_fg_color hsl({hue}, 100%, 12%);
            @define-color headerbar_bg_color hsl({hue}, 100%, 96%);
            @define-color headerbar_fg_color hsl({hue}, 100%, 12%);
            ",
        );
        Self::add_stylesheet(&stylesheet);
    }
    fn add_stylesheet(stylesheet: &str) {
        // TODO: Adding a provider and keeping it in memory forever
        // is a memory leak. Fortunately, it's small. Yes, I should fix this

        let provider = gtk::CssProvider::new();
        provider.load_from_data(stylesheet.as_bytes());
        gtk::StyleContext::add_provider_for_display(
            &gdk::Display::default().unwrap(),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }

    fn is_small_screen(&self) -> bool {
        let imp = self.imp();
        imp.squeezer
            .visible_child()
            .map(|child| child.downcast().ok())
            .flatten()
            .map(|w: adw::HeaderBar| w == self.imp().header_small.get())
            .unwrap_or(false)
    }
    fn squeezer_changed(&self) {
        let imp = self.imp();
        imp.bottom_bar_revealer
            .set_reveal_child(self.is_small_screen());
    }
    fn present_shortcuts(&self) {
        gtk::Builder::from_resource("/com/ranfdev/Geopard/ui/shortcuts.ui");
    }
    fn present_about(&self) {}
    fn focus_tab_next(&self) {
        let imp = self.imp();
        imp.tab_view.select_next_page();
    }
    fn focus_tab_previous(&self) {
        let imp = self.imp();
        imp.tab_view.select_previous_page();
    }
}
