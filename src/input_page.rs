use glib::subclass::prelude::*;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::TemplateChild;

mod imp {
    pub use super::*;
    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/com/ranfdev/Geopard/ui/input_page.ui")]
    pub struct InputPage {
        #[template_child]
        pub label: TemplateChild<gtk::Label>,
        #[template_child]
        pub entry: TemplateChild<gtk::Entry>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for InputPage {
        // `NAME` needs to match `class` attribute of template
        const NAME: &'static str = "InputPage";
        type Type = super::InputPage;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for InputPage {}
    impl WidgetImpl for InputPage {}
    impl BoxImpl for InputPage {}
}

glib::wrapper! {
    pub struct InputPage(ObjectSubclass<imp::InputPage>)
    @extends gtk::Box, gtk::Widget;
}

impl InputPage {
    pub fn new() -> Self {
        glib::Object::new(&[]).unwrap()
    }
}
