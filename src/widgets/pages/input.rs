use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, CompositeTemplate, TemplateChild};

mod imp {
    pub use super::*;
    #[derive(CompositeTemplate, Default)]
    #[template(resource = "/com/ranfdev/Geopard/ui/input_page.ui")]
    pub struct Input {
        #[template_child]
        pub label: TemplateChild<gtk::Label>,
        #[template_child]
        pub entry: TemplateChild<gtk::Entry>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Input {
        // `NAME` needs to match `class` attribute of template
        const NAME: &'static str = "Input";
        type Type = super::Input;
        type ParentType = gtk::Box;

        fn class_init(klass: &mut Self::Class) {
            klass.bind_template();
        }

        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Input {}
    impl WidgetImpl for Input {}
    impl BoxImpl for Input {}
}

glib::wrapper! {
    pub struct Input(ObjectSubclass<imp::Input>)
    @extends gtk::Box, gtk::Widget;
}

impl Input {
    pub fn new() -> Self {
        glib::Object::new()
    }
}
impl Default for Input {
    fn default() -> Self {
        Self::new()
    }
}
