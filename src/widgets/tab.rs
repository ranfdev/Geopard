use anyhow::{bail, Context, Result};
use async_fs::File;
use futures::future::RemoteHandle;
use futures::io::BufReader;
use futures::prelude::*;
use futures::task::LocalSpawnExt;
use glib::{clone, Properties};
use gtk::gdk;
use gtk::gdk::prelude::*;
use gtk::gio;
use gtk::glib;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use gtk::CompositeTemplate;
use gtk::TemplateChild;
use log::{debug, error, info};
use once_cell::sync::Lazy;
use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::marker::PhantomData;
use std::pin::Pin;
use std::rc::Rc;
use url::Url;

use crate::common;
use crate::common::glibctx;
use crate::lossy_text_read::*;
use crate::text_extensions::Gemini as GeminiTextExt;
use crate::widgets;
use gemini::PageElement;

#[derive(Debug, Clone, PartialEq)]
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

pub mod imp {

    pub use super::*;
    #[derive(Debug, Default, Properties, CompositeTemplate)]
    #[template(resource = "/com/ranfdev/Geopard/ui/tab.ui")]
    #[properties(wrapper_type = super::Tab)]
    pub struct Tab {
        pub(crate) gemini_client: RefCell<gemini::Client>,
        pub(crate) gemini_text_ext: RefCell<Option<GeminiTextExt>>,
        pub(crate) history: RefCell<Vec<HistoryItem>>,
        pub(crate) current_hi: Cell<Option<usize>>,
        #[template_child]
        pub(crate) scroll_win: TemplateChild<gtk::ScrolledWindow>,
        #[template_child]
        pub(crate) text_view: TemplateChild<gtk::TextView>,
        #[template_child]
        pub(crate) stack: TemplateChild<gtk::Stack>,
        #[template_child]
        pub(crate) clamp: TemplateChild<adw::Clamp>,
        pub(crate) left_click_ctrl: RefCell<Option<gtk::GestureClick>>,
        pub(crate) right_click_ctrl: RefCell<Option<gtk::GestureClick>>,
        pub(crate) motion_ctrl: RefCell<Option<gtk::EventControllerMotion>>,
        pub(crate) req_handle: RefCell<Option<RemoteHandle<()>>>,
        pub(crate) links: RefCell<HashMap<gtk::TextTag, String>>,
        #[property(get = Self::history_status)]
        pub(crate) history_status: PhantomData<HistoryStatus>,
        #[property(get, set)]
        pub(crate) progress: Cell<f64>,
        #[property(get)]
        pub(crate) title: RefCell<String>,
        #[property(get)]
        pub(crate) url: RefCell<String>,
        #[property(get)]
        pub(crate) hover_url: RefCell<String>,
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
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.left_click_ctrl
                .replace(Some(gtk::GestureClick::builder().button(1).build()));
            self.right_click_ctrl
                .replace(Some(gtk::GestureClick::builder().button(3).build()));
            self.motion_ctrl
                .replace(Some(gtk::EventControllerMotion::new()));
            self.gemini_client
                .replace(gemini::ClientBuilder::new().redirect(true).build());
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
                vec![glib::subclass::Signal::builder(
                    "open-background-tab",
                    &[],
                    <()>::static_type().into(),
                )
                .build()]
            });
            SIGNALS.as_ref()
        }

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
            self.derived_set_property(obj, id, value, pspec).unwrap()
        }

        fn property(&self, obj: &Self::Type, id: usize, pspec: &glib::ParamSpec) -> glib::Value {
            self.derived_property(obj, id, pspec).unwrap()
        }
    }
    impl Tab {
        fn history_status(&self) -> HistoryStatus {
            HistoryStatus {
                current: self.current_hi.get().unwrap_or(0),
                available: self.history.borrow().len(),
            }
        }
    }
    impl WidgetImpl for Tab {}
    impl adw::subclass::bin::BinImpl for Tab {}
}
glib::wrapper! {
    pub struct Tab(ObjectSubclass<imp::Tab>)
        @extends gtk::Widget;
}

impl Tab {
    pub fn new(config: crate::config::Config) -> Self {
        let this: Self = glib::Object::new(&[]).unwrap();
        let imp = this.imp();
        imp.text_view
            .add_controller(imp.left_click_ctrl.borrow().as_ref().unwrap());
        imp.text_view
            .add_controller(imp.right_click_ctrl.borrow().as_ref().unwrap());
        imp.text_view
            .add_controller(imp.motion_ctrl.borrow().as_ref().unwrap());

        imp.gemini_text_ext
            .replace(Some(GeminiTextExt::new(imp.text_view.clone(), config)));

        this.bind_signals();
        this
    }
    pub fn handle_click(&self, ctrl: &gtk::GestureClick, x: f64, y: f64) -> Result<()> {
        let imp = self.imp();
        let gemini_text_ext = imp.gemini_text_ext.borrow();
        let text_view = &gemini_text_ext.as_ref().unwrap().text_view;
        let has_selection = text_view.buffer().has_selection();
        if has_selection {
            return Ok(());
        }
        let url = {
            let links = imp.links.borrow();
            let (_, link) =
                Self::extract_linkhandler(&*links, gemini_text_ext.as_ref().unwrap(), x, y)?;
            self.parse_link(link)?
        };
        if ctrl
            .current_event()
            .unwrap()
            .modifier_state()
            .contains(gdk::ModifierType::CONTROL_MASK)
        {
            self.activate_action("win.open-in-new-tab", Some(&url.as_str().to_variant()))
                .unwrap()
        } else {
            self.spawn_open_url(url);
        }

        Ok(())
    }
    fn handle_right_click(&self, x: f64, y: f64) -> Result<()> {
        let imp = self.imp();
        let gemini_text_ext = imp.gemini_text_ext.borrow();
        let text_view = &gemini_text_ext.as_ref().unwrap().text_view;

        let link = {
            let links = imp.links.borrow();
            let (_, link) =
                Self::extract_linkhandler(&*links, gemini_text_ext.as_ref().unwrap(), x, y)?;
            self.parse_link(link)?
        };
        let link_variant = link.as_str().to_variant();

        let menu = gio::Menu::new();

        let item = gio::MenuItem::new(Some("Open Link In New Tab"), None);
        item.set_action_and_target_value(Some("win.open-in-new-tab"), Some(&link_variant));

        menu.insert_item(0, &item);
        let item = gio::MenuItem::new(Some("Copy Link"), None);
        item.set_action_and_target_value(Some("win.set-clipboard"), Some(&link_variant));
        menu.insert_item(1, &item);
        text_view.set_extra_menu(Some(&menu));
        Ok(())
    }
    fn handle_motion(&self, x: f64, y: f64) -> Result<()> {
        // May need some debounce?

        let imp = self.imp();
        let gemini_text_ext = imp.gemini_text_ext.borrow();
        let gemini_text_ext = gemini_text_ext.as_ref().unwrap();
        let links = imp.links.borrow();
        let entry = Self::extract_linkhandler(&*links, gemini_text_ext, x, y);

        let link_ref = entry.as_ref().map(|x| x.1).unwrap_or("");
        if link_ref == *imp.hover_url.borrow() {
            return Ok(());
        }

        match link_ref {
            "" => {
                gemini_text_ext.text_view.set_cursor_from_name(Some("text"));
            }
            _ => {
                gemini_text_ext
                    .text_view
                    .set_cursor_from_name(Some("pointer"));
            }
        };

        imp.hover_url.replace(link_ref.to_owned());
        self.emit_hover_url();
        Ok(())
    }
    pub fn spawn_open_url(&self, url: Url) {
        let i = self.add_to_history(HistoryItem {
            url: url.clone(),
            cache: Default::default(),
            scroll_progress: 0.0,
        });
        let cache_space = Rc::downgrade(&self.imp().history.borrow()[i].cache);
        let this = self.clone();
        let fut = async move {
            let cache = this.open_url(url).await;
            cache_space.upgrade().map(|rc| rc.replace(cache));
        };
        self.spawn_request(fut);
    }
    fn add_to_history(&self, item: HistoryItem) -> usize {
        let imp = self.imp();
        let i = {
            let mut history = imp.history.borrow_mut();
            let i = imp.current_hi.get();
            if let Some(i) = i {
                let scroll_progress = imp.scroll_win.vadjustment().value();
                history[i].scroll_progress = scroll_progress;
                history.truncate(i + 1);
            };
            history.push(item);
            let i = history.len() - 1;
            imp.current_hi.replace(Some(i));
            i
        };
        self.emit_history_status();
        self.log_history_position();
        i
    }
    fn clear_stack_widgets(&self) {
        let imp = self.imp();
        let pages = imp.stack.pages();
        let mut iter = pages.iter::<gtk::StackPage>().unwrap();
        let first_page = iter.next().unwrap().unwrap();
        imp.stack.set_visible_child(&first_page.child());
        for page in iter.skip(1) {
            imp.stack.remove(&page.unwrap().child());
        }
    }
    fn spawn_request(&self, fut: impl Future<Output = ()> + 'static) {
        let imp = self.imp();
        self.clear_stack_widgets();
        imp.links.borrow_mut().clear();
        imp.req_handle
            .replace(Some(glibctx().spawn_local_with_handle(fut).unwrap()));
    }
    fn open_url(&self, url: Url) -> impl Future<Output = Option<Vec<u8>>> {
        let imp = self.imp();

        self.set_progress(&0.0);
        *imp.title.borrow_mut() = url.to_string();
        self.emit_title();
        *imp.url.borrow_mut() = url.to_string();
        self.emit_url();

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
            this.set_progress(&1.0);
            cache
        };
        self.set_progress(&0.3);
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
        let mut gemini_text_ext = imp.gemini_text_ext.borrow().clone().unwrap();

        self.imp().progress.set(0.0);
        self.emit_progress();

        *self.imp().title.borrow_mut() = url.to_string();
        self.emit_title();

        *self.imp().url.borrow_mut() = url.to_string();
        self.emit_url();

        let this = self.clone();
        async move {
            let buf = BufReader::new(&*cache);
            gemini_text_ext.clear();
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
        let i = self.imp().current_hi.get();
        info!("history position: {i:?}");
    }
    pub fn previous(&self) -> Result<()> {
        let imp = self.imp();
        let i = {
            imp.current_hi
                .get()
                .and_then(|i| i.checked_sub(1))
                .context("going back in history")?
        };
        imp.current_hi.replace(Some(i));
        self.log_history_position();
        self.emit_history_status();

        let h = { imp.history.borrow_mut().get(i).cloned() };
        h.map(|x| self.spawn_request(self.open_history(x)))
            .context("retrieving previous item from history")
    }
    pub fn next(&self) -> Result<()> {
        let imp = self.imp();
        let i = {
            imp.current_hi
                .get()
                .map(|i| i + 1)
                .filter(|i| *i < imp.history.borrow().len())
                .context("going forward in history")?
        };
        imp.current_hi.replace(Some(i));
        self.log_history_position();
        self.emit_history_status();

        let h = { imp.history.borrow_mut().get(i).cloned() };
        h.map(|x| self.spawn_request(self.open_history(x)))
            .context("retrieving next item from history")
    }
    pub fn reload(&self) {
        let imp = self.imp();
        let i = imp.current_hi.get().unwrap();

        imp.history.borrow_mut().get(i).map(|h| {
            h.cache.replace(None);
            self.spawn_request(self.open_history(h.clone()));
        });
    }
    pub fn display_error(&self, error: anyhow::Error) {
        let imp = self.imp();
        error!("{:?}", error);

        let status_page = adw::StatusPage::new();
        status_page.set_title("Error");
        status_page.set_description(Some(&error.to_string()));
        status_page.set_icon_name(Some("dialog-error-symbolic"));

        imp.stack.add_child(&status_page);
        imp.stack.set_visible_child(&status_page);
    }
    fn bind_signals(&self) {
        let imp = self.imp();
        let this = self.clone();
        let left_click_ctrl = imp.left_click_ctrl.borrow();
        let left_click_ctrl = left_click_ctrl.as_ref().unwrap();
        left_click_ctrl.connect_released(move |ctrl, _n_press, x, y| {
            if let Err(e) = this.handle_click(ctrl, x, y) {
                info!("{}", e);
            };
        });

        imp.right_click_ctrl
            .borrow()
            .as_ref()
            .unwrap()
            .connect_pressed(
                clone!(@weak self as this => @default-panic, move |_ctrl, _n_press, x, y| {
                    if let Err(e) = this.handle_right_click(x, y) {
                        info!("{}", e);
                    };
                }),
            );

        imp.motion_ctrl.borrow().as_ref().unwrap().connect_motion(
            clone!(@weak self as this => @default-panic,move |_ctrl, x, y|  {
                let _ = this.handle_motion(x, y);
            }),
        );
    }
    fn extract_linkhandler<'a>(
        m: &'a HashMap<gtk::TextTag, String>,
        gemini_text_ext: &GeminiTextExt,
        x: f64,
        y: f64,
    ) -> Result<(&'a gtk::TextTag, &'a str)> {
        let text_view = &gemini_text_ext.text_view;
        let (x, y) =
            text_view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
        let iter = text_view
            .iter_at_location(x as i32, y as i32)
            .context("Can't get text iter where clicked")?;

        iter.tags()
            .iter()
            .find_map(|x| x.name().is_none().then(|| m.get_key_value(x)).flatten())
            .map(|(k, v)| (k, v.as_str()))
            .ok_or_else(|| anyhow::Error::msg("Clicked text doesn't have a link tag"))
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
        self.imp()
            .gemini_text_ext
            .borrow_mut()
            .as_mut()
            .unwrap()
            .clear();
        match url.scheme() {
            "about" => {
                let mut about = common::ABOUT_PAGE.to_owned();
                about.push_str(&format!(
                    "\n\n## Metadata\n\nApp ID: {}\nVersion: {}",
                    crate::config::APP_ID,
                    crate::config::VERSION
                ));
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
        let imp = self.imp();
        let res: gemini::Response = { imp.gemini_client.borrow().fetch(url.as_str()) }.await?;

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
    fn parse_link(&self, link: &str) -> Result<Url, url::ParseError> {
        let imp = self.imp();
        let current_url = Url::parse(&imp.url.borrow())?;
        let link_url = Url::options().base_url(Some(&current_url)).parse(link)?;
        Ok(link_url)
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

        let page = widgets::DownloadPage::new();
        page.imp().label.set_label(file_name);
        imp.stack.add_child(&page);
        imp.stack.set_visible_child(&page);

        let downloaded_file_url = format!("file://{}", d_path.as_os_str().to_str().unwrap());
        info!("Downloading to {}", downloaded_file_url);
        page.imp().open_btn.connect_clicked(move |_| {
            gtk::show_uri(None::<&gtk::Window>, &downloaded_file_url, 0);
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
        let gemini_text_ext = self.imp().gemini_text_ext.borrow();
        let gemini_text_ext = gemini_text_ext.as_ref().unwrap();
        let mut line = String::with_capacity(1024);
        loop {
            line.clear();
            let n = stream.read_line_lossy(&mut line).await?;
            if n == 0 {
                break Ok(());
            }
            let text_iter = &mut gemini_text_ext.text_buffer.end_iter();
            gemini_text_ext.insert_paragraph(text_iter, &line);
            gemini_text_ext.insert_paragraph(text_iter, "\n");
        }
    }

    fn display_input(&self, url: Url, msg: &str) {
        let imp = self.imp();

        let text_input = widgets::InputPage::new();
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
            gtk::show_uri(None::<&gtk::Window>, url_clone.as_str(), 0);
        });
        child.append(&button);

        status_page.set_child(Some(&child));

        imp.stack.add_child(&status_page);
        imp.stack.set_visible_child(&status_page);
    }
    async fn display_gemini<T: AsyncBufRead + Unpin>(
        &self,
        mut reader: T,
    ) -> anyhow::Result<Vec<u8>> {
        let imp = self.imp();
        let mut gemini_text_ext = imp.gemini_text_ext.borrow().clone().unwrap();

        let mut parser = gemini::Parser::new();
        let mut text_iter = gemini_text_ext.text_buffer.end_iter();

        let mut preformatted = String::new();
        let mut data = String::with_capacity(1024);
        let mut total = 0;
        let mut n;

        let mut title_updated = false;

        loop {
            n = reader.read_line_lossy(&mut data).await?;
            if n == 0 {
                break;
            }
            let line = &data[total..];
            let token = parser.parse_line(line);
            total += n;
            if let PageElement::Preformatted(line) = token {
                preformatted.push_str(&line);
            } else {
                // preformatted text is handled different hoping to add scrollbars for it,
                // in the future, maybe
                if !preformatted.is_empty() {
                    gemini_text_ext.insert_preformatted(&mut text_iter, &preformatted);
                    preformatted.clear();
                }
                match token {
                    PageElement::Text(line) => {
                        gemini_text_ext.insert_paragraph(&mut text_iter, &line);
                    }
                    PageElement::Heading(line) => {
                        gemini_text_ext.insert_heading(&mut text_iter, &line);
                        if !title_updated {
                            title_updated = true;
                            imp.title
                                .replace(line.trim_end().trim_start_matches('#').to_string());
                            self.emit_title();
                        }
                    }
                    PageElement::Quote(line) => {
                        gemini_text_ext.insert_quote(&mut text_iter, &line);
                    }
                    PageElement::Empty => {
                        gemini_text_ext.insert_paragraph(&mut text_iter, "\n");
                    }
                    PageElement::Link(url, label) => {
                        let link_char = if let Ok(true) = self
                            .parse_link(&url)
                            .map(|url| ["gemini", "about"].contains(&url.scheme()))
                        {
                            "⇒"
                        } else {
                            "⇗"
                        };
                        let label = format!("{link_char} {}", label.as_deref().unwrap_or(&url));
                        let tag = gemini_text_ext.insert_link(&mut text_iter, &url, Some(&label));
                        imp.links.borrow_mut().insert(tag, url.clone());
                    }
                    PageElement::Preformatted(_) => unreachable!("handled before"),
                }
            }
        }
        Ok(data.into_bytes())
    }
    pub fn set_link_color(&self, rgba: &gdk::RGBA) {
        self.imp()
            .gemini_text_ext
            .borrow()
            .as_ref()
            .unwrap()
            .set_link_color(rgba);
    }
}
