use std::cell::{Ref, RefCell};
use std::collections::HashSet;
use std::rc::Rc;

use adw::subclass::prelude::BinImpl;
use gemini::known_hosts::KnownHostsRepo;
use gemini::{CertificateError, ClientBuilder};
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{gio, glib};

use crate::common;

#[derive(Debug, Clone)]
pub struct CertificateValidator {
    overridden_hosts: Rc<RefCell<HashSet<String>>>,
    known_hosts_file: Rc<RefCell<gemini::known_hosts::KnownHostsFile>>,
}

impl CertificateValidator {
    pub fn new(file: std::fs::File) -> Self {
        Self {
            overridden_hosts: Default::default(),
            known_hosts_file: Rc::new(RefCell::new(gemini::known_hosts::KnownHostsFile::new(file))),
        }
    }
    pub fn validate(&self, host: &str, sha: &gio::TlsCertificate) -> Result<(), CertificateError> {
        if self.overridden_hosts.borrow().contains(host) {
            return Ok(());
        }
        gemini::known_hosts::validate(&mut *self.known_hosts_file.borrow_mut(), host, sha)
    }
    pub fn override_trust(&self, host: &str) {
        self.overridden_hosts.borrow_mut().insert(host.to_owned());
    }
    pub fn remove_known(&self, host: &str) {
        self.known_hosts_file.borrow_mut().remove(host);
    }
}

pub mod imp {

    use super::*;

    #[derive(Debug, Default)]
    pub struct SessionProvider {
        pub(crate) validator: Rc<RefCell<Option<CertificateValidator>>>,
        pub(crate) client: RefCell<gemini::Client>,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for SessionProvider {
        const NAME: &'static str = "GeopardSessionProvider";
        type Type = super::SessionProvider;
        type ParentType = adw::Bin;
    }

    impl ObjectImpl for SessionProvider {
        fn constructed(&self) {
            self.parent_constructed();
            let cr = CertificateValidator::new(
                std::fs::OpenOptions::new()
                    .read(true)
                    .create(true)
                    .append(true)
                    .open(&*common::KNOWN_HOSTS_PATH)
                    .unwrap(),
            );
            let cr_clone = cr.clone();
            let client = ClientBuilder::new()
                .redirect(true)
                .validator(move |host: &str, sha: &gio::TlsCertificate| {
                    cr_clone.validate(host, sha)
                })
                .build();
            self.validator.replace(Some(cr));
            self.client.replace(client);
        }
    }
    impl WidgetImpl for SessionProvider {}
    impl BinImpl for SessionProvider {}
}

glib::wrapper! {
    pub struct SessionProvider(ObjectSubclass<imp::SessionProvider>)
        @extends gtk::Widget, adw::Bin;
}

impl Default for SessionProvider {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionProvider {
    pub fn new() -> Self {
        let this: SessionProvider = glib::Object::new();
        this
    }
    pub fn from_tree(w: &impl IsA<gtk::Widget>) -> Option<Self> {
        w.ancestor(SessionProvider::static_type())
            .expect("Failed to get SessionProvider from context")
            .downcast::<SessionProvider>()
            .ok()
    }
    pub fn client(&self) -> Ref<gemini::Client> {
        self.imp().client.borrow()
    }
    pub fn validator(&self) -> Ref<CertificateValidator> {
        Ref::map(self.imp().validator.borrow(), |v| v.as_ref().unwrap())
    }
}
