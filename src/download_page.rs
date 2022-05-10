use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use glib::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::TemplateChild;

mod imp {
    pub use super::*;
    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/com/ranfdev/Geopard/ui/download_page.ui")]
    pub struct DownloadPage {
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
    impl ObjectSubclass for DownloadPage {
        // `NAME` needs to match `class` attribute of template
        const NAME: &'static str = "DownloadPage";
        type Type = super::DownloadPage;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for DownloadPage {}
    impl WidgetImpl for DownloadPage {}
    impl BoxImpl for DownloadPage {}
}

glib::wrapper! {
    pub struct DownloadPage(ObjectSubclass<imp::DownloadPage>)
    @extends gtk::Box, gtk::Widget;
}

impl DownloadPage {
    pub fn new() -> Self {
        glib::Object::new(&[]).unwrap()
    }
}
