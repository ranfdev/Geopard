use crate::common;
use crate::common::{glibctx, DrawCtx, HistoryItem, Link, LossyTextRead, PageElement, RequestCtx};
use crate::gemini;
use crate::window::WindowMsg;
use anyhow::{bail, Context, Result};
use async_fs::File;
use futures::future::RemoteHandle;
use futures::io::BufReader;
use futures::prelude::*;
use futures::task::LocalSpawnExt;
use glib::subclass::prelude::*;
use gtk::gdk::prelude::*;
use gtk::prelude::*;
use gtk::subclass::prelude::*;
use log::{debug, error, info, warn};
use once_cell::sync::Lazy;
use std::cell::RefCell;
use url::Url;

#[derive(Clone, Debug, PartialEq)]
pub enum TabMsg {
    Open(Url),
    AddCache(Vec<u8>),
    Back,
    LineClicked(Link),
    GetUrl,
    OpenNewTab(String),
    CopyUrl(String),
    SetProgress(f64),
    GetProgress,
}

pub mod imp {

    pub use super::*;
    #[derive(Debug, Default)]
    pub struct Tab {
        pub(crate) gemini_client: RefCell<gemini::Client>,
        pub(crate) draw_ctx: RefCell<Option<DrawCtx>>,
        pub(crate) history: RefCell<Vec<HistoryItem>>,
        pub(crate) scroll_win: gtk::ScrolledWindow,
        pub(crate) clamp: adw::Clamp,
        pub(crate) event_ctrlr_click: RefCell<Option<gtk::GestureClick>>,
        pub(crate) req_handle: RefCell<Option<RemoteHandle<()>>>,
        pub(crate) load_progress: f64,
    }

    #[glib::object_subclass]
    impl ObjectSubclass for Tab {
        const NAME: &'static str = "GeopardTab";
        type Type = super::Tab;
        type ParentType = gtk::Widget;

        fn class_init(klass: &mut Self::Class) {
            // The layout manager determines how child widgets are laid out.
            klass.set_layout_manager_type::<gtk::BinLayout>();
        }
    }

    impl ObjectImpl for Tab {
        fn constructed(&self, obj: &Self::Type) {
            self.parent_constructed(obj);

            self.scroll_win.set_vexpand(true);
            obj.set_vexpand(true);

            self.clamp.set_parent(obj);
            self.clamp.set_maximum_size(768);
            self.clamp.set_tightening_threshold(720);
            self.clamp.set_child(Some(&self.scroll_win));

            self.event_ctrlr_click
                .replace(Some(gtk::GestureClick::new()));
            self.gemini_client
                .replace(gemini::ClientBuilder::new().redirect(true).build());
        }

        fn dispose(&self, _obj: &Self::Type) {
            self.clamp.unparent();
        }

        fn signals() -> &'static [glib::subclass::Signal] {
            static SIGNALS: Lazy<Vec<glib::subclass::Signal>> = Lazy::new(|| {
                vec![
                    glib::subclass::Signal::builder(
                        "open-background-tab",
                        &[],
                        <()>::static_type().into(),
                    )
                    .build(),
                    glib::subclass::Signal::builder(
                        "title-changed",
                        &[String::static_type().into()],
                        <()>::static_type().into(),
                    )
                    .build(),
                    glib::subclass::Signal::builder(
                        "url-changed",
                        &[String::static_type().into()],
                        <()>::static_type().into(),
                    )
                    .build(),
                ]
            });
            SIGNALS.as_ref()
        }
    }
    impl WidgetImpl for Tab {}
}
glib::wrapper! {
    pub struct Tab(ObjectSubclass<imp::Tab>)
        @extends gtk::Widget;
}
impl Tab {
    pub fn new(config: crate::config::Config) -> Self {
        let mut this: Self = glib::Object::new(&[]).unwrap();
        let imp = this.imp();
        use common::MARGIN;
        let text_view = gtk::builders::TextViewBuilder::new()
            .top_margin(MARGIN)
            .left_margin(MARGIN)
            .right_margin(MARGIN)
            .bottom_margin(MARGIN)
            .indent(2)
            .editable(false)
            .cursor_visible(false)
            .wrap_mode(gtk::WrapMode::WordChar)
            .build();
        text_view.add_controller(imp.event_ctrlr_click.borrow().as_ref().unwrap());

        imp.scroll_win.set_child(Some(&text_view));
        imp.draw_ctx
            .replace(Some(DrawCtx::new(text_view.clone(), config)));

        this.bind_signals();
        this
    }
    pub fn child(&self) -> &gtk::ScrolledWindow {
        let imp = self.imp();
        &imp.scroll_win
    }
    pub fn build_request_ctx(&self, url: Url) -> RequestCtx {
        let imp = self.imp();
        RequestCtx {
            draw_ctx: imp.draw_ctx.borrow().clone().unwrap(),
            gemini_client: imp.gemini_client.borrow().clone(),
            url,
        }
    }
    pub fn url(&self) -> Result<url::Url> {
        let imp = self.imp();
        Ok(imp
            .history
            .borrow_mut()
            .last()
            .context("No items in history")?
            .url
            .clone())
    }
    pub fn add_cache(&self, cache: Vec<u8>) {
        let imp = self.imp();
        if let Some(item) = imp.history.borrow_mut().last_mut() {
            item.cache = Some(cache)
        }
    }
    pub fn open_new_tab(&self, link: &str) -> Result<()> {
        let url = self.parse_link(&link)?;
        Ok(())
    }
    fn handle_right_click(text_view: &gtk::TextView, widget: &gtk::Widget) {
        if let Some(menu) = widget.dynamic_cast_ref::<gtk::PopoverMenu>() {
            // let (x, y) = Self::coords_in_widget(text_view);

            // if let Ok(handler) = Self::extract_linkhandler(text_view, (x as f64, y as f64)) {
            //     let url = handler.url();
            //     /*FIXME: Self::extend_textview_menu(&menu, url.to_owned(), in_chan.clone());*/
            // }
        }
    }
    fn spawn_req(&self, fut: impl Future<Output = ()> + 'static) {
        let imp = self.imp();
        imp.req_handle
            .replace(Some(glibctx().spawn_local_with_handle(fut).unwrap()));
    }
    pub fn spawn_open(&self, url: Url) {
        let imp = self.imp();

        let scroll_progress = imp.scroll_win.vadjustment().value();
        let mut history = imp.history.borrow_mut();
        if let Some(item) = history.last_mut() {
            item.scroll_progress = scroll_progress;
        }
        history.push(HistoryItem {
            url: url.clone(),
            cache: None,
            scroll_progress: 0.0,
        });
        let mut req_ctx = self.build_request_ctx(url.clone());

        let this = self.clone();
        let fut = async move {
            match Self::open_url(&mut req_ctx).await {
                Ok(Some(cache)) => {
                    this.add_cache(cache);
                    info!("Page loaded and cached ({})", url.clone());
                }
                Ok(_) => {
                    info!("Page loaded ({})", url.clone());
                }
                Err(e) => {
                    Self::display_error(&mut req_ctx.draw_ctx, e);
                }
            }
            this.emit_by_name_with_values("title-changed", &[url.to_string().to_value()]);
            this.emit_by_name_with_values("url-changed", &[url.to_string().to_value()]);
        };
        self.spawn_req(fut);
    }
    fn spawn_open_history(&self, item: HistoryItem) {
        let HistoryItem { url, cache, .. } = item;
        match cache {
            Some(cache) => self.spawn_open_cached(url, cache),
            None => self.spawn_open(url),
        }
    }
    fn spawn_open_cached(&self, url: Url, cache: Vec<u8>) {
        let imp = self.imp();
        imp.history.borrow_mut().push(HistoryItem {
            url: url.clone(),
            cache: None,
            scroll_progress: 0.0,
        });

        let mut draw_ctx = imp.draw_ctx.borrow().clone().unwrap();
        let this = self.clone();
        let fut = async move {
            let buf = BufReader::new(cache.as_slice());
            draw_ctx.clear();
            let res = Self::display_gemini(&mut draw_ctx, buf).await;
            match res {
                Ok(cache) => {
                    info!("Loaded {} from cache", &url);
                    this.add_cache(cache);
                }
                Err(e) => Self::display_error(&mut draw_ctx, e),
            }
        };
        self.spawn_req(fut);
    }
    pub fn back(&self) -> Result<()> {
        let imp = self.imp();
        let item = {
            let mut history = imp.history.borrow_mut();
            if history.len() <= 1 {
                bail!("Already at last item in history");
            }
            history.pop();
            history.pop()
        };
        match item {
            Some(item) => self.spawn_open_history(item),
            None => unreachable!(),
        }
        Ok(())
    }
    fn display_error(ctx: &mut DrawCtx, error: anyhow::Error) {
        error!("{:?}", error);
        let error_text = format!("Geopard experienced an error:\n {:?}", error);
        ctx.insert_paragraph(&mut ctx.text_buffer.end_iter(), &error_text);
    }
    pub fn handle_click(&self, buttoni: i32, x: f64, y: f64) -> Result<()> {
        dbg!(x, y);
        let imp = self.imp();
        let draw_ctx = imp.draw_ctx.borrow();
        let text_view = &draw_ctx.as_ref().unwrap().text_view;
        let has_selection = text_view.buffer().has_selection();
        if has_selection {
            return Ok(());
        }
        let handler = Self::extract_linkhandler(&draw_ctx.as_ref().unwrap(), x, y)?;
        match handler {
            Link::Internal(link) => {
                let url = self.parse_link(&link)?;
                self.spawn_open(url);
            }
            Link::External(link) => {
                let url = self.parse_link(&link)?;
                gtk::show_uri(None::<&gtk::Window>, url.as_str(), 0);
            }
        }
        Ok(())
    }
    /* FIXME: fn extend_textview_menu(menu: &gtk::Menu, url: String, sender: flume::Sender<TabMsg>) {
        let copy_link_item = gtk::MenuItem::with_label("Copy link");
        let open_in_tab_item = gtk::MenuItem::with_label("Open in new tab");
        let url_clone = url.clone();
        let sender_clone = sender.clone();
        copy_link_item.connect_activate(move |_| {
            sender_clone
                .send(TabMsg::CopyUrl(url_clone.clone()))
                .unwrap();
        });
        open_in_tab_item.connect_activate(move |_| {
            sender.send(TabMsg::OpenNewTab(url.clone())).unwrap();
        });
        menu.prepend(&copy_link_item);
        menu.prepend(&open_in_tab_item);
        menu.show_all();
    }*/
    fn bind_signals(&self) {
        let imp = self.imp();
        let draw_ctx = imp.draw_ctx.borrow().clone().unwrap();
        let this = self.clone();
        imp.event_ctrlr_click
            .borrow()
            .as_ref()
            .unwrap()
            .connect_pressed(move |_gsclick, buttoni, x, y| {
                match this.handle_click(buttoni, x, y) {
                    Err(e) => info!("{}", e),
                    _ => {}
                };
            });

        /*        .GestureClick
        .connect_event_after(move |text_view, e| {
            let event_is_click = (e.event_type() == gdk::EventType::ButtonRelease
                || e.event_type() == gdk::EventType::TouchEnd)
                && e.button() == Some(1);

            let has_selection = text_view.buffer().unwrap().has_selection();

            if event_is_click && !has_selection {
                match Self::extract_linkhandler(text_view, e.coords().unwrap()) {
                    Ok(handler) => in_chan_tx.send(TabMsg::LineClicked(handler)).unwrap(),
                    Err(e) => warn!("{}", e),
                }
            }
        });*/
        /* FIXME: self.draw_ctx
        .text_view
        .connect_populate_popup(move |text_view, widget| {
            Self::handle_right_click(text_view, widget, in_chan.clone());
        });*/
    }
    fn extract_linkhandler(draw_ctx: &DrawCtx, x: f64, y: f64) -> Result<Link> {
        info!("Extracting linkhandler from clicked text");
        let text_view = &draw_ctx.text_view;
        let (x, y) =
            text_view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);
        let iter = text_view
            .iter_at_location(x as i32, y as i32)
            .context("Can't get text iter where clicked")?;

        for tag in iter.tags() {
            if let Some(link) = DrawCtx::linkhandler(&tag) {
                return Ok(link.clone());
            }
        }

        Err(anyhow::Error::msg("Clicked text doesn't have a link tag"))
    }
    async fn open_file_url(req: &mut RequestCtx) -> Result<()> {
        let path = req
            .url
            .to_file_path()
            .map_err(|_| anyhow::Error::msg("Can't convert link to file path"))?;
        let file = File::open(&path).await?;
        let lines = BufReader::new(file);
        match path.extension().map(|x| x.to_str()) {
            Some(Some("gmi")) | Some(Some("gemini")) => {
                Self::display_gemini(&mut req.draw_ctx, lines).await?;
            }
            _ => {
                Self::display_text(&mut req.draw_ctx, lines).await?;
            }
        }
        Ok(())
    }
    async fn open_url(mut req: &mut RequestCtx) -> Result<Option<Vec<u8>>> {
        req.draw_ctx.clear();
        match req.url.scheme() {
            "about" => {
                let reader = futures::io::BufReader::new(common::ABOUT_PAGE.as_bytes());
                Self::display_gemini(&mut req.draw_ctx, reader).await?;
                Ok(None)
            }
            "file" => {
                Self::open_file_url(&mut req).await?;
                Ok(None)
            }
            "gemini" => Self::open_gemini_url(&mut req).await,
            _ => {
                Self::display_url_confirmation(&mut req.draw_ctx, &req.url);
                Ok(None)
            }
        }
    }
    async fn open_gemini_url(req: &mut RequestCtx) -> anyhow::Result<Option<Vec<u8>>> {
        let res: gemini::Response = req.gemini_client.fetch(req.url.as_str()).await?;

        use gemini::Status::*;
        let meta = res.meta().to_owned();
        let status = res.status();
        debug!("Status: {:?}", &status);
        let res = match status {
            Input(_) => {
                Self::display_input(&mut req.draw_ctx, req.url.clone(), &meta);
                None
            }
            Success(_) => {
                let body = res.body().context("Body not found")?;
                let buffered = futures::io::BufReader::new(body);
                if meta.find("text/gemini").is_some() {
                    let res = Self::display_gemini(&mut req.draw_ctx, buffered).await?;
                    Some(res)
                } else if meta.find("text").is_some() {
                    Self::display_text(&mut req.draw_ctx, buffered).await?;
                    None
                } else {
                    Self::display_download(&mut req.draw_ctx, req.url.clone(), buffered).await?;
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
        let current_url = imp
            .history
            .borrow_mut()
            .last()
            .expect("History item not found")
            .url
            .clone();
        let link_url = Url::options().base_url(Some(&current_url)).parse(link)?;
        Ok(link_url)
    }

    fn download_path(url: &Url) -> anyhow::Result<std::path::PathBuf> {
        let file_name = url
            .path_segments()
            .context("Can't divide url in segments")?
            .last()
            .context("Can't get last url segment")?;

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
        ctx: &mut DrawCtx,
        url: Url,
        mut stream: T,
    ) -> anyhow::Result<()> {
        let d_path = Self::download_path(&url)?;

        let mut buffer = Vec::with_capacity(8192);
        buffer.extend_from_slice(&[0; 8192]);

        let mut read = 0;
        let mut text_iter = ctx.text_buffer.end_iter();
        ctx.insert_paragraph(
            &mut text_iter,
            &format!("writing to {:?}\n", d_path.as_os_str()),
        );
        ctx.insert_paragraph(
            &mut text_iter,
            "to interrupt the download, leave this page\n",
        );
        ctx.insert_paragraph(&mut text_iter, "downloaded\t KB\n");

        let mut file = File::create(&d_path).await?;
        loop {
            match stream.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => {
                    file.write_all(&buffer[..n]).await?;
                    read += n;
                    debug!("lines {}", ctx.text_buffer.line_count());
                    let mut old_line_iter = ctx
                        .text_buffer
                        .iter_at_line(ctx.text_buffer.line_count() - 2);
                    ctx.text_buffer
                        .delete(&mut old_line_iter.unwrap(), &mut ctx.text_buffer.end_iter());
                    ctx.insert_paragraph(
                        &mut old_line_iter.unwrap(),
                        &format!("downloaded\t {}KB\n", read / 1000),
                    );
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
        let mut text_iter = ctx.text_buffer.end_iter();
        ctx.insert_paragraph(&mut text_iter, "download finished!\n");
        let downloaded_file_url = format!("file://{}", d_path.as_os_str().to_str().unwrap());
        ctx.insert_link(
            &mut text_iter,
            Link::External(downloaded_file_url),
            Some("open with default program"),
        );

        Ok(())
    }
    async fn display_text(
        draw_ctx: &mut DrawCtx,
        mut stream: impl AsyncBufRead + Unpin,
    ) -> anyhow::Result<()> {
        let mut line = String::with_capacity(1024);
        loop {
            line.clear();
            let n = stream.read_line_lossy(&mut line).await?;
            if n == 0 {
                break Ok(());
            }
            let text_iter = &mut draw_ctx.text_buffer.end_iter();
            draw_ctx.insert_paragraph(text_iter, &line);
            draw_ctx.insert_paragraph(text_iter, "\n");
        }
    }

    fn display_input(ctx: &mut DrawCtx, url: Url, msg: &str) {
        let text_buffer = &ctx.text_buffer;

        let mut iter = text_buffer.end_iter();
        ctx.insert_paragraph(&mut iter, &msg);
        ctx.insert_paragraph(&mut iter, "\n");

        let anchor = text_buffer.create_child_anchor(&mut text_buffer.end_iter());
        let text_input = gtk::Entry::new();
        text_input.set_hexpand(true);
        ctx.text_view.add_child_at_anchor(&text_input, &anchor);
        text_input.show();

        text_input.connect_activate(move |text_input| {
            let query = text_input.text().to_string();
            let mut url = url.clone();
            url.set_query(Some(&query));
            // sender.send(TabMsg::Open(url)).unwrap();
        });
    }

    fn display_url_confirmation(ctx: &mut DrawCtx, url: &Url) {
        let mut text_iter = ctx.text_buffer.end_iter();
        ctx.insert_paragraph(
            &mut text_iter,
            "Geopard doesn't support this url scheme. 
If you want to open the following link in an external application, \
click on the link below\n",
        );

        ctx.insert_link(
            &mut text_iter,
            Link::External(url.to_string()),
            Some(url.as_str()),
        );
    }
    async fn display_gemini<T: AsyncBufRead + Unpin>(
        draw_ctx: &mut DrawCtx,
        mut reader: T,
    ) -> anyhow::Result<Vec<u8>> {
        let mut parser = gemini::Parser::new();
        let mut text_iter = draw_ctx.text_buffer.end_iter();

        let mut data = String::with_capacity(1024);
        let mut total = 0;
        let mut n;

        loop {
            n = reader.read_line_lossy(&mut data).await?;
            if n == 0 {
                break;
            }
            let line = &data[total..];
            let token = parser.parse_line(line);
            total += n;
            match token {
                PageElement::Text(line) => {
                    draw_ctx.insert_paragraph(&mut text_iter, &line);
                }
                PageElement::Heading(line) => {
                    draw_ctx.insert_heading(&mut text_iter, &line);
                }
                PageElement::Quote(line) => {
                    draw_ctx.insert_quote(&mut text_iter, &line);
                }
                PageElement::Preformatted(line) => {
                    draw_ctx.insert_preformatted(&mut text_iter, &line);
                }
                PageElement::Empty => {
                    draw_ctx.insert_paragraph(&mut text_iter, "\n");
                }
                PageElement::Link(url, label) => {
                    draw_ctx.insert_link(&mut text_iter, Link::Internal(url), label.as_deref());
                }
            }
        }
        Ok(data.into_bytes())
    }
}
