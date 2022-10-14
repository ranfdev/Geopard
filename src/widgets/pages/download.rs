use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, CompositeTemplate, TemplateChild};

mod imp {
    pub use super::*;
    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/com/ranfdev/Geopard/ui/download_page.ui")]
    pub struct Download {
        #[template_child]
        pub label: TemplateChild<gtk::Label>,
        #[template_child]
        pub label_downloaded: TemplateChild<gtk::Label>,
        #[template_child]
        pub progress_bar: TemplateChild<gtk::ProgressBar>,
        #[template_child]
        pub open_btn: TemplateChild<gtk::Button>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Download {
        // `NAME` needs to match `class` attribute of template
        const NAME: &'static str = "Download";
        type Type = super::Download;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Download {}
    impl WidgetImpl for Download {}
    impl BoxImpl for Download {}
}

glib::wrapper! {
    pub struct Download(ObjectSubclass<imp::Download>)
    @extends gtk::Box, gtk::Widget;
}

impl Download {
    pub fn new() -> Self {
        glib::Object::new(&[]).unwrap()
    }
}
impl Default for Download {
    fn default() -> Self {
        Self::new()
    }
}
