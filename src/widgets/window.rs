use adw::prelude::*;
use adw::subclass::application_window::AdwApplicationWindowImpl;
use anyhow::Context;
use config::APP_ID;
use futures::prelude::*;
use glib::{clone, Properties};
use gtk::gdk;
use gtk::gio;
use gtk::glib;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::TemplateChild;
use log::{error, info, warn};
use std::cell::RefCell;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use url::Url;

use crate::common::{bookmarks_url, glibctx, BOOKMARK_FILE_PATH};
use crate::config;
use crate::self_action;
use crate::widgets::tab::{HistoryStatus, Tab};

const ZOOM_CHANGE_FACTOR: f64 = 1.15;
const ZOOM_MAX_FACTOR: f64 = 5.0;

#[derive(Debug, Clone, Default)]
pub(crate) struct Zoom {
    value: f64,
    provider: gtk::CssProvider,
}

pub mod imp {
    use super::*;
    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[template(resource = "/com/ranfdev/Geopard/ui/window.ui")]
    #[properties(wrapper_type = super::Window)]
    pub struct Window {
        #[template_child]
        pub(crate) url_bar: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(crate) small_url_bar: TemplateChild<gtk::SearchEntry>,
        #[template_child]
        pub(crate) bottom_bar_revealer: TemplateChild<gtk::Revealer>,
        #[template_child]
        pub(crate) url_status: TemplateChild<gtk::Label>,
        #[template_child]
        pub(crate) url_status_box: TemplateChild<gtk::Box>,
        #[template_child]
        pub(crate) header_small: TemplateChild<gtk::WindowHandle>,
        #[template_child]
        pub(crate) squeezer: TemplateChild<adw::Squeezer>,
        #[template_child]
        pub(crate) progress_bar: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub(crate) tab_view: TemplateChild<adw::TabView>,
        #[template_child]
        pub(crate) tab_bar: TemplateChild<adw::TabBar>,
        #[template_child]
        pub(crate) primary_menu_btn: TemplateChild<gtk::MenuButton>,
        pub(crate) config: RefCell<config::Config>,
        pub(crate) progress_animation: RefCell<Option<adw::SpringAnimation>>,
        pub(crate) binded_tab_properties: RefCell<Vec<glib::Binding>>,
        #[property(get, set)]
        pub(crate) url: RefCell<String>,
        #[property(get = Self::progress_animated, set = Self::set_progress_animated)]
        pub(crate) progress: PhantomData<f64>,
        pub(crate) scroll_ctrl: gtk::EventControllerScroll,
        pub(crate) mouse_prev_next_ctrl: gtk::GestureClick,
        pub(crate) action_previous: RefCell<Option<gio::SimpleAction>>,
        pub(crate) action_next: RefCell<Option<gio::SimpleAction>>,
        pub(crate) style_provider: RefCell<gtk::CssProvider>,
        #[property(get, set = Self::set_zoom, type = f64, member = value)]
        pub(crate) zoom: RefCell<Zoom>,
        pub(crate) settings: glib::ConstructRefCell<gio::Settings>,
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
        fn set_zoom(&self, v: f64) {
            let Zoom { value, provider } = &mut *self.zoom.borrow_mut();
            *value = v.clamp(1.0 / ZOOM_MAX_FACTOR, ZOOM_MAX_FACTOR);
            provider.load_from_data(
                format!(
                    "textview {{
                        font-size: {}rem;
                    }}",
                    value
                )
                .as_bytes(),
            );
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
        imp.zoom.borrow_mut().value = 1.0;

        this.setup_css_providers();
        this.squeezer_changed();
        this.setup_settings();
        this.setup_zoom_popover_item();
        this.setup_actions();
        this.setup_signals();
        this.open_in_new_tab(bookmarks_url().as_str());
        this
    }
    fn setup_settings(&self) {
        let imp = self.imp();
        let settings = gio::Settings::new(APP_ID);
        settings.bind("zoom", self, "zoom").build();
        imp.settings.replace(Some(settings));
    }
    fn setup_css_providers(&self) {
        let imp = self.imp();
        gtk::StyleContext::add_provider_for_display(
            &gdk::Display::default().unwrap(),
            &*imp.style_provider.borrow(),
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );

        gtk::StyleContext::add_provider_for_display(
            &gdk::Display::default().unwrap(),
            &imp.zoom.borrow().provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    fn setup_actions(&self) {
        let imp = self.imp();

        let action_previous = self_action!(self, "previous", previous);
        let action_next = self_action!(self, "next", next);
        imp.action_previous.borrow_mut().replace(action_previous);
        imp.action_next.borrow_mut().replace(action_next);

        self_action!(self, "new-tab", new_tab);
        self_action!(self, "show-bookmarks", show_bookmarks);
        self_action!(self, "bookmark-current", bookmark_current);
        self_action!(self, "close-tab", close_tab);
        self_action!(self, "focus-url-bar", focus_url_bar);
        self_action!(self, "shortcuts", present_shortcuts);
        self_action!(self, "about", present_about);
        self_action!(self, "focus-previous-tab", focus_previous_tab);
        self_action!(self, "focus-next-tab", focus_next_tab);
        self_action!(self, "donate", donate);
        self_action!(self, "zoom-in", zoom_in);
        self_action!(self, "zoom-out", zoom_out);
        self_action!(self, "reset-zoom", reset_zoom);

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
    fn setup_signals(&self) {
        let imp = self.imp();

        self.add_controller(&imp.scroll_ctrl);
        self.add_controller(&imp.mouse_prev_next_ctrl);
        imp.scroll_ctrl
            .set_propagation_phase(gtk::PropagationPhase::Capture);
        imp.scroll_ctrl
            .set_flags(gtk::EventControllerScrollFlags::VERTICAL);
        imp.scroll_ctrl.connect_scroll(
            clone!(@weak self as this => @default-panic, move |ctrl, _, y| {
                let up = y < 0.0;
                if let Some(true) = ctrl.current_event().map(|e| e.modifier_state()).map(|m| m == gdk::ModifierType::CONTROL_MASK) {
                    if up {
                      this.zoom_in();
                    } else {
                      this.zoom_out();
                    }
                    gtk::Inhibit(true)
                } else {
                    this.imp().bottom_bar_revealer.set_reveal_child(up && this.is_small_screen());
                    gtk::Inhibit(false)
                }
            }),
        );
        imp.mouse_prev_next_ctrl.set_button(0);
        imp.mouse_prev_next_ctrl.connect_pressed(
            clone!(@weak self as this => @default-panic, move |ctrl, _, _, _| {
                match ctrl.current_button() {
                    8 => {
                        this.previous();
                    },
                    9 => {
                        this.next();
                    },
                    _ => {},
                }
            }),
        );

        self.connect_local(
            "notify::url",
            false,
            clone!(@weak self as this => @default-panic, move |_| {
                this.set_special_color_from_hash();
                let bar = this.active_url_bar();
                if bar.focus_child().is_none() {
                    bar.set_text(&this.url());
                }
                None
            }),
        );

        imp.tab_view.connect_selected_page_notify(
            clone!(@weak self as this => @default-panic, move |tab_view| {
              this.page_switched(tab_view);
            }),
        );
        imp.squeezer.connect_visible_child_notify(
            clone!(@weak self as this => @default-panic, move |_sq| {
                this.squeezer_changed();
            }),
        );
        imp.url_bar
            .connect_activate(clone!(@weak self as this => @default-panic, move |_sq| {
                this.open_omni(this.imp().url_bar.text().as_str());
            }));
        imp.small_url_bar.connect_activate(
            clone!(@weak self as this => @default-panic, move |_sq| {
                this.open_omni(this.imp().small_url_bar.text().as_str());
            }),
        );

        adw::StyleManager::default().connect_dark_notify(
            clone!(@weak self as this => @default-panic, move |_| {
                this.set_special_color_from_hash();
            }),
        );

        let ctrl = gtk::EventControllerMotion::new();
        imp.url_status_box.add_controller(&ctrl);
        let url_status_box_clone = imp.url_status_box.clone();
        ctrl.connect_motion(move |_, _, _| {
            url_status_box_clone.set_visible(false);
        });

        let ctrl = gtk::EventControllerKey::new();
        ctrl.set_propagation_limit(gtk::PropagationLimit::None);
        ctrl.set_propagation_phase(gtk::PropagationPhase::Capture);
        self.add_controller(&ctrl);
        ctrl.connect_key_pressed(
            clone!(@weak self as this => @default-panic, move |_, key, _, modif| {
              let action = match (modif.contains(gdk::ModifierType::CONTROL_MASK), key) {
                (true, gdk::Key::ISO_Left_Tab) => Some("win.focus-previous-tab"),
                (true, gdk::Key::Tab) => Some("win.focus-next-tab"),
                _ => None,
              };
              action
                  .map(|a| WidgetExt::activate_action(&this, a, None))
                  .map(|_| gtk::Inhibit(true))
                  .unwrap_or(gtk::Inhibit(false))
            }),
        );
    }
    fn setup_zoom_popover_item(&self) {
        let imp = self.imp();

        let popover: gtk::PopoverMenu = imp.primary_menu_btn.popover().unwrap().downcast().unwrap();
        let zoom_box = gtk::Box::builder()
            .spacing(12)
            .margin_start(18)
            .margin_end(18)
            .build();

        zoom_box.append(
            &gtk::Button::builder()
                .icon_name("zoom-out-symbolic")
                .action_name("win.zoom-out")
                .css_classes(vec!["flat".into(), "circular".into()])
                .build(),
        );

        let value_btn = gtk::Button::with_label("100%");
        value_btn.set_hexpand(true);
        self.bind_property("zoom", &value_btn, "label")
            .flags(glib::BindingFlags::SYNC_CREATE)
            .transform_to(|_, v| {
                let zoom: f64 = v.get().unwrap();
                Some(format!("{:3}%", (zoom * 100.0) as usize).to_value())
            })
            .build();
        value_btn.set_action_name(Some("win.reset-zoom"));
        value_btn.add_css_class("flat");
        value_btn.add_css_class("body");
        value_btn.add_css_class("numeric");

        zoom_box.append(&value_btn);
        zoom_box.append(
            &gtk::Button::builder()
                .icon_name("zoom-in-symbolic")
                .css_classes(vec!["flat".into(), "circular".into()])
                .action_name("win.zoom-in")
                .build(),
        );
        popover.add_child(&zoom_box, "zoom");
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

        // Unset the focus from the url_bar
        if let Some(r) = tab_view.root() {
            r.set_focus(None::<&gtk::Widget>)
        }

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
                tab.bind_property("hover-url", &*imp.url_status, "label")
                    .build(),
                tab.bind_property("hover-url", &*imp.url_status_box, "visible")
                    .transform_to(|_, v| {
                        let v: &str = v.get().unwrap();
                        Some((!v.is_empty()).to_value())
                    })
                    .build(),
            ]);
        };
    }
    fn new_tab(&self) {
        self.show_bookmarks();
        self.active_url_bar().grab_focus();
    }
    fn show_bookmarks(&self) {
        let imp = self.imp();
        let p = self.add_tab();
        imp.tab_view.set_selected_page(&p);
        self.inner_tab(&p).spawn_open_url(bookmarks_url());
    }
    fn close_tab(&self) {
        let imp = self.imp();
        imp.tab_view
            .close_page(&imp.tab_view.page(&self.current_tab()));
        if imp.tab_view.n_pages() == 0 {
            std::process::exit(0); // TODO: maybe there's a better way for gtk apps...
        }
    }
    fn active_url_bar(&self) -> &gtk::SearchEntry {
        let imp = self.imp();
        if self.is_small_screen() {
            &*imp.small_url_bar
        } else {
            &*imp.url_bar
        }
    }
    fn focus_url_bar(&self) {
        self.active_url_bar().grab_focus();
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
    fn open_omni(&self, v: &str) {
        let url = Url::parse(v).or_else(|_| {
            if v.contains('.') && v.split('.').all(|s| s.chars().all(char::is_alphanumeric)) {
                Url::parse(&format!("gemini://{}", v))
            } else {
                Url::parse(&format!("gemini://geminispace.info/search?{}", v))
            }
        });
        match url {
            Ok(url) => self.current_tab().spawn_open_url(url),
            Err(e) => error!("Failed to open from omni bar: {}", e),
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
    fn set_special_color_from_hash(&self) {
        let imp = self.imp();
        let url = imp.url.borrow();
        let url = if let Ok(domain) = Url::parse(&url) {
            if let Some(domain) = domain.domain() {
                domain.to_string()
            } else {
                url.to_string()
            }
        } else {
            url.to_string()
        };
        let hash = {
            let mut s = std::collections::hash_map::DefaultHasher::new();
            url.hash(&mut s);
            s.finish()
        };
        let hue = hash % 360;
        let stylesheet = if adw::StyleManager::default().is_dark() {
            format!(
                "
                    @define-color view_bg_color hsl({hue}, 20%, 8%);
                    @define-color view_fg_color hsl({hue}, 100%, 98%);
                    @define-color window_bg_color hsl({hue}, 20%, 8%);
                    @define-color window_fg_color hsl({hue}, 100%, 98%);
                    @define-color headerbar_bg_color hsl({hue}, 80%, 10%);
                    @define-color headerbar_fg_color hsl({hue}, 100%, 98%);
                "
            )
        } else {
            format!(
                "
                    @define-color view_bg_color hsl({hue}, 100%, 99%);
                    @define-color view_fg_color hsl({hue}, 100%, 12%);
                    @define-color window_bg_color hsl({hue}, 100%, 99%);
                    @define-color window_fg_color hsl({hue}, 100%, 12%);
                    @define-color headerbar_bg_color hsl({hue}, 100%, 96%);
                    @define-color headerbar_fg_color hsl({hue}, 100%, 12%);
                    "
            )
        };

        imp.style_provider
            .borrow()
            .load_from_data(stylesheet.as_bytes());
        // FIXME: Should add a method on `Tab`...
        self.current_tab()
            .set_link_color(&self.style_context().lookup_color("accent_color").unwrap());
    }

    fn is_small_screen(&self) -> bool {
        let imp = self.imp();
        imp.squeezer
            .visible_child()
            .and_then(|child| child.downcast().ok())
            .map(|w: gtk::WindowHandle| w == self.imp().header_small.get())
            .unwrap_or(false)
    }
    fn squeezer_changed(&self) {
        let imp = self.imp();
        let is_small = self.is_small_screen();
        imp.bottom_bar_revealer.set_reveal_child(is_small);
        if is_small {
            imp.tab_bar.add_css_class("inline");
        } else {
            imp.tab_bar.remove_css_class("inline");
        }
    }
    fn present_shortcuts(&self) {
        gtk::Builder::from_resource("/com/ranfdev/Geopard/ui/shortcuts.ui");
    }
    fn present_about(&self) {
        self.open_url_str("about://help");
    }
    fn donate(&self) {
        gtk::show_uri(
            None::<&gtk::Window>,
            "https://github.com/sponsors/ranfdev",
            0,
        );
    }
    fn focus_next_tab(&self) {
        let imp = self.imp();
        imp.tab_view.select_next_page();
    }
    fn focus_previous_tab(&self) {
        let imp = self.imp();
        imp.tab_view.select_previous_page();
    }

    fn zoom_in(&self) {
        self.set_zoom(&(self.zoom() * ZOOM_CHANGE_FACTOR));
    }
    fn zoom_out(&self) {
        self.set_zoom(&(self.zoom() * 1.0 / ZOOM_CHANGE_FACTOR));
    }
    fn reset_zoom(&self) {
        self.set_zoom(&1.0);
    }
}
