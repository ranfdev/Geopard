use std::cell::RefCell;
use std::sync::OnceLock;

use adw::prelude::*;
use adw::subclass::prelude::*;

use glib::subclass::{InitializingObject, Signal};
use gtk::{
    glib::{self, clone, Object},
    CompositeTemplate,
};

use crate::bookmarks;

pub mod imp {
    use super::*;

    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/com/ranfdev/Geopard/ui/bookmarks.ui")]
    pub struct BookmarksWindow {
        #[template_child]
        pub toast_overlay: TemplateChild<adw::ToastOverlay>,
        #[template_child]
        pub stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub bookmarks_list: TemplateChild<gtk::ListBox>,
        pub(crate) bookmarks: RefCell<bookmarks::Bookmarks>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for BookmarksWindow {
        const NAME: &'static str = "GeopardBookmarksWindow";
        type Type = super::BookmarksWindow;
        type ParentType = adw::Window;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &InitializingObject<Self>) {
            obj.init_template();
        }
    }
    impl ObjectImpl for BookmarksWindow {
        fn signals() -> &'static [Signal] {
            static SIGNALS: OnceLock<Vec<Signal>> = OnceLock::new();
            SIGNALS.get_or_init(|| {
                vec![Signal::builder("open-bookmark-url")
                    .param_types([str::static_type()])
                    .build()]
            })
        }

        fn constructed(&self) {
            self.parent_constructed();
        }
    }
    impl WidgetImpl for BookmarksWindow {}
    impl WindowImpl for BookmarksWindow {}
    impl AdwWindowImpl for BookmarksWindow {}
}

glib::wrapper! {
    pub struct BookmarksWindow(ObjectSubclass<imp::BookmarksWindow>)
        @extends adw::Window, gtk::Window, gtk::Widget;
}

impl BookmarksWindow {
    pub fn new(app: &gtk::Application, bookmarks: bookmarks::Bookmarks) -> Self {
        let this = Object::builder::<Self>()
            .property("application", app)
            .build();
        let imp = this.imp();
        imp.bookmarks.replace(bookmarks);

        this.setup();

        this
    }

    fn setup(&self) {
        let imp = self.imp();
        // TODO: Set to bookmarks_page if there's at least one bookmark
        imp.stack.set_visible_child_name("bookmarks_page");

        let bookmarks_map = imp.bookmarks.borrow().clone().bookmarks;

        for (_, bookmark) in bookmarks_map.iter() {
            self.add_row(&bookmark.title(), &bookmark.url());
        }
    }

    // TODO: create_new_row -> adw::ActionRow
    fn add_row(&self, title: &str, url: &str) {
        let imp = self.imp();
        let title = title.to_string();
        let url = url.to_string();

        let check_button = gtk::CheckButton::builder()
            .visible(false)
            .css_classes(vec!["selection-mode"])
            .valign(gtk::Align::Center)
            .build();

        let copy_button = gtk::Button::builder()
            .icon_name("edit-copy-symbolic")
            .tooltip_text("Copy URL")
            .css_classes(vec!["flat"])
            .valign(gtk::Align::Center)
            .build();

        copy_button.connect_clicked(clone!(@weak imp => move |_| {
            imp.toast_overlay.add_toast(adw::Toast::new("Copied to clipboard"));
        }));

        let row = adw::ActionRow::builder()
            .activatable(true)
            .title(&title)
            .subtitle(&url)
            .build();
        row.add_prefix(&check_button);
        row.add_suffix(&copy_button);

        row.connect_activated(clone!(@weak self as this => move |_| {
            this.on_row_activated(&url);
        }));

        imp.bookmarks_list.append(&row);
    }

    fn on_row_activated(&self, url: &str) {
        let imp = self.imp().obj();
        imp.emit_by_name::<()>("open-bookmark-url", &[&url]);
    }
}
