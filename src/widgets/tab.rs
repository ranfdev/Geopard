use std::cell::{Cell, Ref, RefCell};
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;

use anyhow::{bail, Context, Result};
use async_fs::File;
use futures::future::RemoteHandle;
use futures::io::BufReader;
use futures::prelude::*;
use futures::task::LocalSpawnExt;
use gemini::CertificateError;
use glib::{clone, Properties};
use gtk::gdk::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::{glib, CompositeTemplate, TemplateChild};
use hypertext::HypertextEvent;
use log::{debug, info};
use once_cell::sync::Lazy;
use url::Url;

use super::pages::{self, hypertext};
use crate::common;
use crate::common::{glibctx, open_file_externally, open_uri_externally};
use crate::lossy_text_read::*;
use crate::session_provider::SessionProvider;

const BYTES_BEFORE_YIELD: usize = 1024 * 10;

#[derive(Clone)]
pub struct HistoryItem {
    pub url: url::Url,
    pub cache: Rc<RefCell<Option<Vec<u8>>>>,
    pub scroll_progress: f64,
}

#[derive(Clone, Debug, glib::Boxed, Default)]
#[boxed_type(name = "GeopardHistoryStatus")]
pub struct HistoryStatus {
    pub(crate) current: usize,
    pub(crate) available: usize,
}

#[derive(Default)]
pub struct History {
    items: Vec<HistoryItem>,
    index: Option<usize>,
}

impl History {
    fn push(&mut self, item: HistoryItem) -> usize {
        let new_index = self.index.map_or(0, |i| i + 1);
        self.index = Some(new_index);
        self.items.truncate(new_index);
        self.items.push(item);
        new_index
    }
    fn index(&self) -> Option<usize> {
        self.index
    }
    fn len(&self) -> usize {
        self.items.len()
    }
    fn current(&self) -> Option<&HistoryItem> {
        self.index.map(|i| &self.items[i])
    }
    fn items(&self) -> &[HistoryItem] {
        &self.items
    }
    fn set_index(&mut self, i: usize) -> bool {
        if (0..self.items.len()).contains(&i) {
            self.index = Some(i);
            true
        } else {
            false
        }
    }
    fn go_back(&mut self) -> bool {
        self.set_index(self.index.unwrap_or(0).saturating_sub(1))
    }
}

pub mod imp {

    pub use super::*;
    #[derive(Properties, CompositeTemplate)]
    #[template(resource = "/com/ranfdev/Geopard/ui/tab.ui")]
    #[properties(wrapper_type = super::Tab)]
    pub struct Tab {
        pub(crate) config: RefCell<crate::config::Config>,
        pub(crate) history: RefCell<History>,
        #[template_child]
        pub(crate) scroll_win: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub(crate) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(crate) clamp: TemplateChild<adw::ClampScrollable>,
        pub(crate) req_handle: RefCell<Option<RemoteHandle<()>>>,
        #[property(get = Self::history_status)]
        pub(crate) history_status: PhantomData<HistoryStatus>,
        #[property(get, set)]
        pub(crate) progress: Cell<f64>,
        #[property(get)]
        pub(crate) title: RefCell<String>,
        #[property(get)]
        pub(crate) url: RefCell<String>,
        #[property(get, set)]
        pub(crate) hover_url: RefCell<String>,
    }

    impl Default for Tab {
        fn default() -> Self {
            Self {
                config: Default::default(),
                history: Default::default(),
                scroll_win: Default::default(),
                stack: Default::default(),
                clamp: Default::default(),
                req_handle: Default::default(),
                history_status: PhantomData,
                progress: Default::default(),
                title: Default::default(),
                url: Default::default(),
                hover_url: Default::default(),
            }
        }
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Tab {
        const NAME: &'static str = "Tab";
        type Type = super::Tab;
        type ParentType = adw::Bin;

        fn class_init(klass: &mut Self::Class) {
            Self::bind_template(klass);
        }
        fn instance_init(obj: &glib::subclass::InitializingObject<Self>) {
            obj.init_template();
        }
    }

    impl ObjectImpl for Tab {
        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<glib::subclass::Signal>> =
                Lazy::new(|| vec![glib::subclass::Signal::builder("open-background-tab").build()]);
            SIGNALS.as_ref()
        }

        fn properties() -> &'static [glib::ParamSpec] {
            Self::derived_properties()
        }

        fn set_property(&self, id: usize, value: &glib::Value, pspec: &glib::ParamSpec) {
            self.derived_set_property(id, value, pspec)
        }

        fn property(&self, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(id, pspec)
        }
    }
    impl Tab {
        fn history_status(&self) -> HistoryStatus {
            let history = self.history.borrow();
            HistoryStatus {
                current: history.index().unwrap_or(0),
                available: history.len(),
            }
        }
    }
    impl WidgetImpl for Tab {}
    impl adw::subclass::bin::BinImpl for Tab {}
}
glib::wrapper! {
    pub struct Tab(ObjectSubclass<imp::Tab>)
        @extends gtk::Widget, adw::Bin;
}

impl Tab {
    pub fn new(config: crate::config::Config) -> Self {
        let this: Self = glib::Object::new();
        let imp = this.imp();

        imp.config.replace(config);

        this
    }

    fn session(&self) -> crate::session_provider::SessionProvider {
        SessionProvider::from_tree(self).unwrap()
    }

    pub fn spawn_open_url(&self, url: Url) {
        let imp = self.imp();

        // If there's an in flight request, the related history item (the last one)
        // must be removed
        {
            let req = imp.req_handle.take();
            if let Some(req) = req {
                // if the request isn't ready, it's still in flight
                if req.now_or_never().is_none() {
                    imp.history.borrow_mut().go_back();
                }
            }
        }

        let body: Rc<RefCell<Option<Vec<u8>>>> = Rc::new(RefCell::new(None));
        let body_weak = Rc::downgrade(&body);

        self.add_to_history(HistoryItem {
            url: url.clone(),
            cache: body,
            scroll_progress: 0.0,
        });

        let this = self.clone();
        let fut = async move {
            let data = this.open_url(url).await;
            *body_weak.upgrade().unwrap().borrow_mut() = data;
        };
        self.spawn_request(fut);
    }
    fn add_to_history(&self, mut item: HistoryItem) -> usize {
        let imp = self.imp();

        item.scroll_progress = imp.scroll_win.vadjustment().value();
        {
            imp.history.borrow_mut().push(item);
        }

        self.notify_history_status();
        self.log_history_position();
        imp.history.borrow().index().unwrap()
    }
    fn clear_stack_widgets(&self) {
        let imp = self.imp();
        let pages = imp.stack.pages();
        let mut iter = pages.iter::<gtk::StackPage>();
        let first_page = iter.next().unwrap().unwrap();
        imp.stack.set_visible_child(&first_page.child());
        for page in iter.skip(1) {
            imp.stack.remove(&page.unwrap().child());
        }
    }
    fn spawn_request(&self, fut: impl Future<Output = ()> + 'static) {
        let imp = self.imp();
        self.clear_stack_widgets();
        imp.req_handle
            .replace(Some(glibctx().spawn_local_with_handle(fut).unwrap()));
    }
    fn open_url(&self, url: Url) -> impl Future<Output = Option<Vec<u8>>> {
        let imp = self.imp();

        self.set_progress(0.0);
        *imp.title.borrow_mut() = url.to_string();
        self.notify_title();
        *imp.url.borrow_mut() = url.to_string();
        self.notify_url();

        let this = self.clone();
        let fut = async move {
            let cache = match this.send_request(url.clone()).await {
                Ok(Some(cache)) => {
                    info!("Page loaded, can be cached ({})", url.clone());
                    Some(cache)
                }
                Ok(_) => {
                    info!("Page loaded ({})", &url);
                    None
                }
                Err(e) => {
                    this.display_error(e);
                    None
                }
            };
            this.set_progress(1.0);
            cache
        };
        self.set_progress(0.3);
        fut
    }
    fn open_history(&self, item: HistoryItem) -> Pin<Box<dyn Future<Output = ()>>> {
        let HistoryItem { url, cache, .. } = item;

        let cache = cache.borrow();
        match &*cache {
            Some(cache) => Box::pin(self.open_cached(url, cache.clone())),
            None => Box::pin(self.open_url(url).map(|_| {})),
        }
    }
    fn open_cached(&self, url: Url, cache: Vec<u8>) -> impl Future<Output = ()> {
        let imp = self.imp();

        imp.progress.set(0.0);
        self.notify_progress();

        *imp.title.borrow_mut() = url.to_string();
        self.notify_title();

        *imp.url.borrow_mut() = url.to_string();
        self.notify_url();

        let this = self.clone();
        async move {
            let buf = BufReader::new(&*cache);
            let res = this.display_gemini(buf).await;
            match res {
                Ok(_) => {
                    info!("Loaded {} from cache", &url);
                }
                Err(e) => this.display_error(e),
            }
        }
    }
    fn log_history_position(&self) {
        let i = self.imp().history.borrow().index();
        info!("history position: {i:?}");
    }
    pub fn previous(&self) -> bool {
        self.move_in_history(-1)
    }
    pub fn next(&self) -> bool {
        self.move_in_history(1)
    }
    pub fn reload(&self) {
        let imp = self.imp();

        if let Some(h) = imp.history.borrow_mut().current() {
            self.spawn_request(self.open_history(h.clone()));
        }
    }

    async fn open_file_url(&self, url: Url) -> Result<()> {
        let path = url
            .to_file_path()
            .map_err(|_| anyhow::Error::msg("Can't convert link to file path"))?;

        let this = self.clone();
        let file = File::open(&path).await?;
        let lines = BufReader::new(file);
        match path.extension().map(|x| x.to_str()) {
            Some(Some("gmi")) | Some(Some("gemini")) => {
                this.display_gemini(lines).await?;
            }
            _ => {
                this.display_text(lines).await?;
            }
        }
        Ok(())
    }
    async fn send_request(&self, url: Url) -> Result<Option<Vec<u8>>> {
        match url.scheme() {
            "about" => {
                let mut about = common::ABOUT_PAGE.to_owned();
                use std::fmt::Write;
                write!(
                    &mut about,
                    "\n## Metadata\nApp ID: {}\nVersion: {}",
                    crate::config::APP_ID,
                    crate::config::VERSION
                )
                .unwrap();
                let reader = futures::io::BufReader::new(about.as_bytes());
                self.display_gemini(reader).await?;
                Ok(None)
            }
            "file" => {
                self.open_file_url(url).await?;
                Ok(None)
            }
            "gemini" => self.open_gemini_url(url).await,
            _ => {
                self.display_url_confirmation(&url);
                Ok(None)
            }
        }
    }
    async fn open_gemini_url(&self, url: Url) -> anyhow::Result<Option<Vec<u8>>> {
        let res = self.session().client().fetch(url.as_str()).await;
        let res = match res {
            Ok(res) => res,
            Err(gemini::Error::Tls(CertificateError::BadIdentity)) => {
                self.display_mitm_error();
                return Ok(None);
            }
            Err(gemini::Error::Tls(e)) => {
                self.display_tls_error(e);
                return Ok(None);
            }
            Err(e) => return Err(e.into()),
        };

        use gemini::Status::*;
        let meta = res.meta().to_owned();
        let status = res.status();
        debug!("Status: {:?}", &status);

        let this = self.clone();
        let res = match status {
            Input(_) => {
                self.display_input(url.clone(), &meta);
                None
            }
            Success(_) => {
                let body = res.body().context("Body not found")?;
                let buffered = futures::io::BufReader::new(body);

                if meta.contains("text/gemini") {
                    let res = this.display_gemini(buffered).await?;
                    Some(res)
                } else if meta.contains("text") {
                    self.display_text(buffered).await?;
                    None
                } else {
                    self.display_download(url.clone(), buffered).await?;
                    None
                }
            }
            Redirect(_) => bail!("Redirected more than 5 times"),
            TempFail(_) => bail!("Temporary server failure"),
            PermFail(_) => bail!("Permanent server failure"),
            CertRequired(_) => bail!("A certificate is required to access this page"),
        };
        Ok(res)
    }

    fn download_path(file_name: &str) -> anyhow::Result<std::path::PathBuf> {
        let mut file_name = std::path::PathBuf::from(file_name);
        loop {
            let mut d_path = common::DOWNLOAD_PATH.join(&file_name);
            if d_path.exists() {
                let mut name_no_ext = file_name
                    .file_stem()
                    .context("Can't get file_stem (filename without ext)")?
                    .to_owned();
                let empty_os_string = &std::ffi::OsString::from("");
                let ext = d_path.extension().unwrap_or(empty_os_string);
                file_name = {
                    name_no_ext.push("_new.");
                    name_no_ext.push(ext);
                    std::path::PathBuf::from(name_no_ext)
                };
                d_path.set_file_name(&file_name);
            } else {
                break Ok(d_path);
            }
        }
    }
    async fn display_download<T: AsyncRead + Unpin>(
        &self,
        url: Url,
        mut stream: T,
    ) -> anyhow::Result<()> {
        let imp = self.imp();

        let file_name = url
            .path_segments()
            .context("Can't divide url in segments")?
            .last()
            .context("Can't get last url segment")?;
        let d_path = Self::download_path(file_name)?;

        let page = pages::Download::new();
        page.imp().label.set_label(file_name);
        imp.stack.add_child(&page);
        imp.stack.set_visible_child(&page);

        let downloaded_file_url = format!("file://{}", d_path.as_os_str().to_str().unwrap());
        info!("Downloading to {}", downloaded_file_url);
        page.imp().open_btn.connect_clicked(move |_| {
            open_file_externally(&std::path::Path::new(&downloaded_file_url));
        });

        let ext = file_name.split('.').last();
        if let Some(true) = ext.map(|ext| crate::common::STREAMABLE_EXTS.contains(&ext)) {
            page.imp().open_btn.set_opacity(1.0);
        }

        let mut buffer = Vec::with_capacity(8192);
        buffer.extend_from_slice(&[0; 8192]);

        let mut read = 0;
        let mut last_update_time = glib::real_time();
        const THROTTLE_TIME: i64 = 300_000; // 0.3s

        let mut file = File::create(&d_path).await?;
        loop {
            match stream.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => {
                    file.write_all(&buffer[..n]).await?;
                    read += n;

                    let t = glib::real_time();
                    if t - last_update_time > THROTTLE_TIME {
                        page.imp().progress_bar.pulse();
                        page.imp()
                            .label_downloaded
                            .set_text(&format!("{:.2}KB", read as f64 / 1000.0));
                        last_update_time = t;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
        page.imp()
            .label_downloaded
            .set_text(&format!("{:.2}KB", read as f64 / 1000.0));
        page.imp().progress_bar.set_fraction(1.0);
        page.imp().open_btn.set_opacity(1.0);
        page.imp().open_btn.set_label("Open");
        page.imp().open_btn.add_css_class("suggested-action");

        Ok(())
    }
    async fn display_text(&self, mut stream: impl AsyncBufRead + Unpin) -> anyhow::Result<()> {
        let page = self.new_hypertext_page();
        let mut pe = Vec::new();

        page.render(
            [gemini::Event::Start(gemini::Tag::CodeBlock)].into_iter(),
            &mut pe,
        )
        .unwrap();
        let mut line = String::with_capacity(1024);
        let mut total = 0;
        let mut last_yield_at_bytes = 0;

        loop {
            let n = stream.read_line_lossy(&mut line).await?;
            if n == 0 {
                break;
            }
            total += n;

            if let Err(err) = page.render([gemini::Event::Text(&line)].into_iter(), &mut pe) {
                anyhow::bail!("Error while parsing the page: {}", err);
            }

            // Yield control to main thread after every 10KB, to not block the UI
            if total - last_yield_at_bytes >= BYTES_BEFORE_YIELD {
                glib::timeout_future(std::time::Duration::from_millis(1)).await;
                last_yield_at_bytes = total;
            }
            line.clear();
        }
        page.render([gemini::Event::End].into_iter(), &mut pe)
            .unwrap();
        Ok(())
    }

    fn display_input(&self, url: Url, msg: &str) {
        let imp = self.imp();

        let text_input = pages::Input::new();
        imp.stack.add_child(&text_input);
        imp.stack.set_visible_child(&text_input);
        text_input.imp().label.set_label(msg);
        text_input.imp().entry.connect_activate(move |entry| {
            let query = entry.text().to_string();
            let mut url = url.clone();
            url.set_query(Some(&query));
            entry
                .activate_action("win.open-url", Some(&url.to_string().to_variant()))
                .unwrap();
        });
    }

    fn display_url_confirmation(&self, url: &Url) {
        let imp = self.imp();
        let status_page = adw::StatusPage::new();
        status_page.set_title("External Link");
        status_page.set_description(Some(&glib::markup_escape_text(url.as_str())));
        status_page.set_icon_name(Some("web-browser-symbolic"));

        let child = gtk::Box::new(gtk::Orientation::Horizontal, 8);
        child.set_halign(gtk::Align::Center);

        let button = gtk::Button::with_label("Copy");
        button.add_css_class("pill");
        button.set_halign(gtk::Align::Center);
        button.set_action_name(Some("win.set-clipboard"));
        button.set_action_target_value(Some(&url.as_str().to_variant()));
        child.append(&button);

        let button = gtk::Button::with_label("Open");
        button.add_css_class("suggested-action");
        button.add_css_class("pill");
        button.set_halign(gtk::Align::Center);
        let url_clone = url.clone();
        button.connect_clicked(move |_| {
            open_uri_externally(url_clone.as_str());
        });
        child.append(&button);

        status_page.set_child(Some(&child));

        imp.stack.add_child(&status_page);
        imp.stack.set_visible_child(&status_page);
    }
    fn new_hypertext_page(&self) -> pages::Hypertext {
        let imp = self.imp();

        let surface = pages::hypertext::Surface::new(imp.config.borrow().clone());
        imp.clamp.set_child(Some(surface.root()));

        let p = pages::Hypertext::new(self.url(), surface);
        p.bind_property("hover_url", self, "hover_url").build();

        p.connect_local(
            "open",
            false,
            clone!(
                #[weak(rename_to = this)]
                self,
                #[upgrade_or_panic]
                move |s| {
                    let s: String = s[1].get().unwrap();
                    let url = Url::parse(&s);
                    if let Ok(url) = url {
                        this.spawn_open_url(url);
                    } else {
                        log::error!("Invalid url {:?}", url);
                    }
                    None
                }
            ),
        );
        p
    }
    async fn display_gemini<T: AsyncBufRead + Unpin>(
        &self,
        mut reader: T,
    ) -> anyhow::Result<Vec<u8>> {
        let imp = self.imp();

        let mut parser = gemini::Parser::new();
        let mut data = String::with_capacity(1024);
        let mut total = 0;
        let mut last_yield_at_bytes = 0;

        let page = self.new_hypertext_page();
        let mut page_events = vec![];

        loop {
            let res = reader.read_line_lossy(&mut data).await;

            let n = match res {
                Ok(0) => break,
                Ok(n) => n,
                Err(err) => return Err(anyhow::anyhow!(err.to_string())),
            };

            {
                let line = &data[total..];
                let mut tokens = vec![];
                parser.parse_line(line, &mut tokens);
                total += n;

                if let Err(err) = page.render(tokens.drain(0..), &mut page_events) {
                    log::error!("Error while parsing the page: {}", err);
                    break;
                }

                for ev in page_events.drain(0..) {
                    match ev {
                        HypertextEvent::Title(title) => {
                            imp.title.replace(title);
                            self.notify("title");
                        }
                    }
                }
            }

            // Yield control to main thread after every 10KB, to not block the UI
            if total - last_yield_at_bytes >= BYTES_BEFORE_YIELD {
                glib::timeout_future(std::time::Duration::from_millis(1)).await;
                last_yield_at_bytes = total;
            }
        }

        Ok(data.into_bytes())
    }
    pub fn display_error(&self, error: anyhow::Error) {
        let imp = self.imp();

        log::error!("{:?}", error);

        let p = adw::StatusPage::new();
        p.set_title("Error");
        p.set_description(Some(&error.to_string()));
        p.set_icon_name(Some("dialog-error-symbolic"));

        imp.stack.add_child(&p);
        imp.stack.set_visible_child(&p);
    }
    pub fn display_mitm_error(&self) {
        let imp = self.imp();

        let p = adw::StatusPage::new();
        p.set_title("Tls Server Certificate Changed");
        p.set_description(Some(
            "This may be caused by a malicious actor trying to intercept your connection",
        ));
        p.set_icon_name(Some("dialog-error-symbolic"));

        let override_btn = gtk::Button::with_label("Trust New Certificate");
        override_btn.connect_clicked(clone!(
            #[weak(rename_to = this)]
            self,
            move |_| {
                let url = Url::parse(&this.url()).unwrap();

                this.session()
                    .validator()
                    .remove_known(url.host_str().unwrap());
                this.reload();
            }
        ));
        override_btn.set_halign(gtk::Align::Center);
        override_btn.add_css_class("destructive-action");
        override_btn.add_css_class("pill");
        p.set_child(Some(&override_btn));

        imp.stack.add_child(&p);
        imp.stack.set_visible_child(&p);
    }
    pub fn display_tls_error(&self, error: CertificateError) {
        let imp = self.imp();

        let p = adw::StatusPage::new();
        p.set_title("Tls Server Certificate Error");
        p.set_description(Some(&format!(
            "{:?}
        You can override the certificate validity for this session
        and continue",
            &error.to_string()
        )));
        p.set_icon_name(Some("dialog-error-symbolic"));

        let override_btn = gtk::Button::with_label("Continue");
        override_btn.connect_clicked(clone!(
            #[weak(rename_to = this)]
            self,
            move |_| {
                let url = Url::parse(&this.url()).unwrap();

                this.session()
                    .validator()
                    .override_trust(url.host_str().unwrap());
                this.reload();
            }
        ));
        override_btn.set_halign(gtk::Align::Center);
        override_btn.add_css_class("destructive-action");
        override_btn.add_css_class("pill");
        p.set_child(Some(&override_btn));

        imp.stack.add_child(&p);
        imp.stack.set_visible_child(&p);
    }
    pub fn history_items(&self) -> Ref<[HistoryItem]> {
        Ref::map(self.imp().history.borrow(), |x| x.items())
    }
    pub fn move_in_history(&self, offset: isize) -> bool {
        let moved = {
            let mut h = self.imp().history.borrow_mut();
            let new_index = if offset > 0 {
                h.index().unwrap_or(0) + offset as usize
            } else {
                h.index().unwrap_or(0).saturating_sub(offset.unsigned_abs())
            };
            h.index() != Some(new_index) && h.set_index(new_index)
        };
        if moved {
            self.spawn_request(
                self.open_history(self.imp().history.borrow().current().unwrap().clone()),
            );
            self.notify_history_status();
        }
        moved
    }
}
