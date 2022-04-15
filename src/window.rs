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
use std::marker::PhantomData;
use url::Url;

use crate::common::{bookmarks_url, glibctx, BOOKMARK_FILE_PATH};
use crate::config;
use crate::tab::{Tab, TabPropertiesExt};
use crate::{self_action, view};

pub mod imp {
    use super::*;
    #[derive(Debug, Default, Properties)]
    pub struct Window {
        pub(crate) url_bar: gtk::SearchEntry,
        pub(crate) small_url_bar: gtk::SearchEntry,
        pub(crate) bottom_bar_revealer: gtk::Revealer,
        pub(crate) header_small: adw::HeaderBar,
        pub(crate) squeezer: adw::Squeezer,
        pub(crate) progress_bar: gtk::ProgressBar,
        pub(crate) tab_view: adw::TabView,
        pub(crate) config: RefCell<config::Config>,
        pub(crate) progress_animation: RefCell<Option<adw::SpringAnimation>>,
        pub(crate) binded_tab_properties: RefCell<Vec<glib::Binding>>,
        #[prop(get, set)]
        pub(crate) url: RefCell<String>,
        #[prop(get = Self::progress_animated, set = Self::set_progress_animated)]
        pub(crate) progress: PhantomData<f64>,
        pub(crate) scroll_ctrl: gtk::EventControllerScroll,
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
                &self.progress_bar,
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

        imp.progress_bar.add_css_class("osd");
        imp.progress_bar.set_valign(gtk::Align::Start);

        let menu_model = Self::build_menu_common();
        view!(
            header_small = (imp.header_small.clone()) {
                set_show_end_title_buttons(false),
                set_title_widget: Some(&(se = (imp.small_url_bar.clone()) {
                    set_hexpand: true,
                    connect_activate: |url_bar| {
                        url_bar
                            .activate_action("win.open-omni", Some(&url_bar.text().to_variant()))
                            .unwrap();
                    },
                    bind "text" this "url",
                })),
            }
            header_bar = adw::HeaderBar {
                pack_start: &(b = gtk::Button {
                    set_icon_name: "go-previous-symbolic",
                    set_action_name: Some("win.back"),
                }),
                pack_start: &(b = gtk::Button {
                    set_icon_name: "tab-new-symbolic",
                    set_action_name: Some("win.new-tab"),
                }),
                pack_end: &(b = gtk::MenuButton {
                    set_icon_name: "open-menu",
                    set_menu_model: Some(&menu_model),
                }),
                set_title_widget: Some(&(c = adw::Clamp {
                    set_child: Some(&(se = (imp.url_bar.clone()) {
                        set_hexpand: true,
                        set_width_request: 360,
                        connect_activate: |url_bar| {
                            url_bar
                                .activate_action("win.open-omni", Some(&url_bar.text().to_variant()))
                                .unwrap();
                        },
                        bind "text" this "url",
                    })),
                    set_maximum_size: 768,
                    set_tightening_threshold: 720,
                })),
            }
            bottom_bar = adw::HeaderBar {
                set_show_end_title_buttons: false,
                set_show_start_title_buttons: false,
                pack_start: &(b = gtk::Button {
                    set_icon_name: "go-previous-symbolic",
                    set_action_name: Some("win.back"),
                }),
                pack_start: &(b = gtk::Button {
                    set_icon_name: "go-next-symbolic",
                    set_action_name: Some("win.next"),
                }),
                set_title_widget: Some(&(b = gtk::Button {
                    set_icon_name: "system-search-symbolic",
                    set_action_name: Some("win.focus-url-bar"),
                })),
                pack_end: &(b = gtk::MenuButton {
                    set_icon_name: "open-menu",
                    set_menu_model: Some(&menu_model),
                }),
                pack_end: &(b = gtk::Button {
                    set_icon_name: "tab-new-symbolic",
                    set_action_name: Some("win.new-tab"),
                }),
            }
            tab_view = (imp.tab_view.clone()) {
                connect_selected_page_notify:
                    clone!(@weak this => move |tab_view| this.page_switched(tab_view)),
            }
            content = gtk::Box {
                set_orientation: gtk::Orientation::Vertical,
                append: &(squeezer = (imp.squeezer.clone()) {
                    set_transition_type: adw::SqueezerTransitionType::Crossfade,
                    add: &header_bar,
                    add: &header_small,
                    connect_visible_child_notify:
                        clone!(@weak this => move |_| this.squeezer_changed()),
                }),
                append: &(tab_bar = adw::TabBar {
                    set_view: Some(&tab_view),
                }),
                append: &(overlay = gtk::Overlay {
                    set_child: Some(&tab_view),
                    add_overlay: &imp.progress_bar,
                    add_overlay: &(b = (imp.bottom_bar_revealer.clone()) {
                        set_transition_type: gtk::RevealerTransitionType::SlideUp,
                        set_child: Some(&bottom_bar),
                        set_valign: gtk::Align::End,
                    }),
                }),
            }
        );

        this.set_default_size(800, 600);
        this.set_content(Some(&content));
        this.squeezer_changed();

        this.setup_actions_signals();
        this.open_in_new_tab(bookmarks_url().as_str());
        this
    }

    fn build_menu_common() -> gio::Menu {
        view!(
            bookmarks = gio::Menu {
                append(Some("All Bookmarks"), Some("win.show-bookmarks")),
                append(Some("Add Bookmark"), Some("win.bookmark-current")),
            }
            about = gio::Menu {
                append(Some("Keyboard Shortcuts"), Some("win.shortcuts")),
                append(Some("About"), Some("win.about")),
                append(Some("Donate 💝"), Some("win.donate")),
            }
            menu_model = gio::Menu {
                append_section(None, &bookmarks),
                append_section(None, &about),
            }
        );
        menu_model
    }
    fn setup_actions_signals(&self) {
        let imp = self.imp();

        self_action!(self, "back", back);
        self_action!(self, "new-tab", add_tab_focused);
        self_action!(self, "show-bookmarks", add_tab_focused);
        self_action!(self, "bookmark-current", bookmark_current);
        self_action!(self, "close-tab", close_tab);
        self_action!(self, "focus-url-bar", focus_url_bar);
        self_action!(self, "shortcuts", present_shortcuts);
        self_action!(self, "about", present_about);

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
                    return gtk::Inhibit(false);
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
        tab_view.selected_page().map(|page| {
            let tab = self.inner_tab(&page);
            btp.drain(0..).for_each(|binding| binding.unbind());
            btp.extend([
                tab.bind_property("url", self, "url")
                    .flags(glib::BindingFlags::SYNC_CREATE)
                    .build(),
                tab.bind_property("progress", self, "progress")
                    .flags(glib::BindingFlags::SYNC_CREATE)
                    .build(),
            ]);
        });
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

    fn is_small_screen(&self) -> bool {
        let imp = self.imp();
        imp.squeezer
            .visible_child()
            .map(|child| child.downcast().ok())
            .flatten()
            .map(|w: adw::HeaderBar| w == self.imp().header_small)
            .unwrap_or(false)
    }
    fn squeezer_changed(&self) {
        let imp = self.imp();
        imp.bottom_bar_revealer
            .set_reveal_child(self.is_small_screen());
    }
    fn present_shortcuts(&self) {
        let builder = gtk::Builder::from_string(
            r#"
<?xml version="1.0" encoding="UTF-8"?>
<interface>
  <object class="GtkShortcutsWindow" id="shortcuts-geopard">
    <property name="modal">1</property>
    <child>
      <object class="GtkShortcutsSection">
        <property name="section-name">shortcuts</property>
        <child>
          <object class="GtkShortcutsGroup">
            <property name="title" translatable="yes">Tabs</property>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="accelerator">&lt;ctrl&gt;T</property>
                <property name="title" translatable="yes">Open New Tab</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="accelerator">&lt;ctrl&gt;W</property>
                <property name="title" translatable="yes">Close Current Tab</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="accelerator">&lt;ctrl&gt;U</property>
                <property name="title" translatable="yes">View Source</property>
              </object>
            </child>
          </object>
        </child>
        <child>
          <object class="GtkShortcutsGroup">
            <property name="title" translatable="yes">Navigation</property>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="accelerator">&lt;alt&gt;Left</property>
                <property name="direction">ltr</property>
                <property name="title" translatable="yes">Back</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="accelerator">F6</property>
                <property name="title" translatable="yes">Focus Url Bar</property>
              </object>
            </child>
          </object>
        </child>
        <child>
          <object class="GtkShortcutsGroup">
            <property name="view">world</property>
            <property name="title" translatable="yes">Bookmarks</property>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="accelerator">&lt;ctrl&gt;D</property>
                <property name="title" translatable="yes">Bookmark Current Page</property>
              </object>
            </child>
            <child>
              <object class="GtkShortcutsShortcut">
                <property name="accelerator">&lt;ctrl&gt;B</property>
                <property name="title" translatable="yes">Show All Bookmarks</property>
              </object>
            </child>
          </object>
        </child>
      </object>
    </child>
  </object>
</interface>
"#,
        );
        let sw: gtk::ShortcutsWindow = builder.object("shortcuts-geopard").unwrap();
        sw.set_transient_for(Some(self));
        sw.present();
    }
    fn present_about(&self) {}
}
