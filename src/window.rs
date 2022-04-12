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
    @extends adw::ApplicationWindow, gtk::Window, gtk::Widget,
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
        imp.add_tab_btn.set_icon_name("tab-new-symbolic");

        let menu_button = gtk::MenuButton::new();
        menu_button.set_primary(true);
        menu_button.set_icon_name("open-menu");
        menu_button.set_menu_model(Some(&Self::build_menu_common()));

        header_bar.pack_start(&imp.back_btn);
        header_bar.pack_start(&imp.add_tab_btn);
        header_bar.pack_end(&menu_button);

        imp.url_bar.set_hexpand(true);

        let bar_clamp = adw::Clamp::new();
        bar_clamp.set_child(Some(&imp.url_bar));
        bar_clamp.set_maximum_size(768);
        bar_clamp.set_tightening_threshold(720);
        header_bar.set_title_widget(Some(&bar_clamp));

        content.append(&header_bar);

        let overlay = gtk::Overlay::new();
        let content_view = gtk::Box::new(gtk::Orientation::Vertical, 0);
        overlay.set_child(Some(&content_view));

        imp.tab_bar.set_view(Some(&imp.tab_view));
        content_view.append(&imp.tab_bar);
        content_view.append(&imp.tab_view);

        imp.progress_bar.add_css_class("osd");
        imp.progress_bar.set_valign(gtk::Align::Start);
        overlay.add_overlay(&imp.progress_bar);
        content.append(&overlay);

        let bottom_bar = adw::HeaderBar::new();
        let bottom_entry = gtk::SearchEntry::new();
        bottom_entry.set_hexpand(true);
        bottom_bar.set_title_widget(Some(&bottom_entry));
        let bottom_menu = gtk::MenuButton::new();
        let bottom_menu_model = Self::build_menu_common();
        let section = gio::Menu::new();
        section.append(Some("Back"), Some("win.back"));
        section.append(Some("New Tab"), Some("win.new-tab"));
        bottom_menu_model.append_section(None, &section);
        bottom_menu.set_menu_model(Some(&bottom_menu_model));
        bottom_menu.set_icon_name("open-menu");
        bottom_bar.pack_end(&bottom_menu);

        content.append(&bottom_bar);

        this.set_default_size(800, 600);
        this.set_content(Some(&content));

        this.bind_signals();
        this.setup_actions();
        this.open_in_new_tab(bookmarks_url().as_str());
        this
    }

    fn build_menu_common() -> gio::Menu {
        let menu_model = gio::Menu::new();

        let menu_model_bookmarks = gio::Menu::new();
        menu_model_bookmarks.append(Some("All Bookmarks"), Some("win.show-bookmarks"));
        menu_model_bookmarks.append(Some("Add Bookmark"), Some("win.bookmark-current"));
        menu_model.insert_section(0, None, &menu_model_bookmarks);

        let menu_model_about = gio::Menu::new();
        menu_model_about.append(Some("Keyboard Shortcuts"), Some("win.shortcuts"));
        menu_model_about.append(Some("About"), Some("win.about"));
        menu_model_about.append(Some("Donate 💝"), Some("win.donate"));
        menu_model.insert_section(1, None, &menu_model_about);
        menu_model
    }
    fn setup_actions(&self) {
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
    }
    fn add_tab(&self) -> adw::TabPage {
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

        imp.tab_view.append(&tab)
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

    fn bind_signals(&self) {
        let imp = self.imp();
        imp.url_bar.connect_activate(|url_bar| {
            url_bar
                .activate_action("win.open-omni", Some(&url_bar.text().to_variant()))
                .unwrap();
        });
        imp.back_btn.set_action_name(Some("win.back"));
        imp.add_tab_btn.set_action_name(Some("win.new-tab"));
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
