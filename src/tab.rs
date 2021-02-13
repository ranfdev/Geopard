use crate::common;
use crate::common::{glibctx, DrawCtx, HistoryItem, Link, LossyTextRead, PageElement, RequestCtx};
use crate::component::{new_component_id, Component};
use crate::gemini;
use crate::window::WindowMsg;
use anyhow::{bail, Context, Result};
use async_fs::File;
use futures::future::RemoteHandle;
use futures::io::BufReader;
use futures::prelude::*;
use futures::task::LocalSpawnExt;
use gdk::prelude::*;
use gtk::prelude::*;
use log::{debug, error, info, warn};
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

pub struct Tab {
    gemini_client: gemini::Client,
    draw_ctx: DrawCtx,
    history: Vec<HistoryItem>,
    req_handle: Option<RemoteHandle<()>>,
    in_chan_tx: flume::Sender<TabMsg>,
    in_chan_rx: flume::Receiver<TabMsg>,
    out_chan: flume::Sender<WindowMsg>,
    scroll_win: gtk::ScrolledWindow,
    load_progress: f64,
    id: usize,
}

impl Tab {
    pub fn new(config: crate::config::Config, out_chan: flume::Sender<WindowMsg>) -> Self {
        use common::MARGIN;
        let text_view = gtk::TextViewBuilder::new()
            .top_margin(MARGIN)
            .left_margin(MARGIN)
            .right_margin(MARGIN)
            .bottom_margin(MARGIN)
            .indent(2)
            .editable(false)
            .cursor_visible(false)
            .wrap_mode(gtk::WrapMode::WordChar)
            .build();

        let (in_chan_tx, in_chan_rx) = flume::unbounded();

        let draw_ctx = DrawCtx::new(text_view.clone(), config);
        let gemini_client = gemini::ClientBuilder::new().redirect(true).build();
        let req_handle = None;
        let history = vec![];

        let scroll_win = gtk::ScrolledWindow::new::<gtk::Adjustment, gtk::Adjustment>(None, None);
        scroll_win.set_vexpand(true);
        scroll_win.add(&text_view);

        Self {
            draw_ctx,
            gemini_client,
            req_handle,
            history,
            in_chan_rx,
            in_chan_tx,
            out_chan,
            scroll_win,
            load_progress: 0.0,
            id: new_component_id(),
        }
    }
    pub fn build_request_ctx(&self, url: Url) -> RequestCtx {
        RequestCtx {
            draw_ctx: self.draw_ctx.clone(),
            in_chan_tx: self.in_chan_tx.clone(),
            gemini_client: self.gemini_client.clone(),
            url,
        }
    }
    pub fn run(mut self) -> Component<gtk::ScrolledWindow, TabMsg> {
        let in_chan_tx = self.in_chan_tx.clone();
        let in_chan_rx = self.in_chan_rx.clone();
        let widget = self.widget().clone();
        let id = self.id;
        self.bind_signals();
        let handle_msgs = async move {
            while let Ok(msg) = in_chan_rx.recv_async().await {
                match self.handle_msg(msg).await {
                    Ok(()) => {}
                    Err(e) => error!("While handling message: {}", e),
                }
            }
        };
        let handle = glibctx().spawn_local_with_handle(handle_msgs).unwrap();

        Component::new(id, widget, in_chan_tx, handle)
    }
    pub async fn handle_msg(&mut self, msg: TabMsg) -> Result<()> {
        debug!("Msg: {:?}", &msg);
        match msg {
            TabMsg::Open(url) => self.spawn_open(url),
            TabMsg::Back => match self.back() {
                Ok(()) => info!("Went back"),
                Err(e) => warn!("{}", e),
            },
            TabMsg::LineClicked(link) => self.handle_click(&link)?,
            TabMsg::GetUrl => {
                let url = &self.history.last().context("No items in history")?.url;

                self.out_chan
                    .send(WindowMsg::UpdateUrlBar(self.id, url.clone()))
                    .unwrap();
            }
            TabMsg::SetProgress(n) => {
                self.load_progress = n;
                self.out_chan
                    .send(WindowMsg::SetProgress(self.id, self.load_progress))
                    .unwrap();
            }
            TabMsg::GetProgress => {
                self.out_chan
                    .send(WindowMsg::SetProgress(self.id, self.load_progress))
                    .unwrap();
            }
            TabMsg::AddCache(cache) => {
                if let Some(item) = self.history.last_mut() {
                    item.cache = Some(cache)
                }
            }
            TabMsg::OpenNewTab(link) => {
                let url = self.parse_link(&link)?;
                self.out_chan.send(WindowMsg::OpenNewTab(url)).unwrap();
            }
            TabMsg::CopyUrl(link) => {
                let url = self.parse_link(&link)?;
                self.draw_ctx
                    .text_view
                    .get_clipboard(&gdk::SELECTION_CLIPBOARD)
                    .set_text(url.as_str());
            }
        }
        Ok(())
    }
    fn handle_right_click(
        text_view: &gtk::TextView,
        widget: &gtk::Widget,
        in_chan: flume::Sender<TabMsg>,
    ) {
        if let Some(menu) = widget.dynamic_cast_ref::<gtk::Menu>() {
            let (x, y) = Self::get_coords_in_widget(text_view);

            if let Ok(handler) = Self::extract_linkhandler(text_view, (x as f64, y as f64)) {
                let url = handler.url();
                Self::extend_textview_menu(&menu, url.to_owned(), in_chan.clone());
            }
        }
    }
    fn spawn_req(&mut self, fut: impl Future<Output = ()> + 'static) {
        self.req_handle = Some(glibctx().spawn_local_with_handle(fut).unwrap());
    }
    pub fn spawn_open(&mut self, url: Url) {
        let scroll_progress = self.scroll_win.get_vadjustment().unwrap().get_value();
        if let Some(item) = self.history.last_mut() {
            item.scroll_progress = scroll_progress;
        }
        self.history.push(HistoryItem {
            url: url.clone(),
            cache: None,
            scroll_progress: 0.0,
        });
        let mut req_ctx = self.build_request_ctx(url.clone());
        self.out_chan
            .send(WindowMsg::UpdateUrlBar(self.id, url.clone()))
            .unwrap();
        self.in_chan_tx.send(TabMsg::SetProgress(0.5)).unwrap();

        let draw_ctx = self.draw_ctx.clone();
        let in_chan_tx = self.in_chan_tx.clone();
        let fut = async move {
            match Self::open_url(&mut req_ctx).await {
                Ok(Some(cache)) => {
                    in_chan_tx.send(TabMsg::AddCache(cache)).unwrap();
                    info!("Page loaded and cached ({})", url.clone());
                }
                Ok(_) => {
                    info!("Page loaded ({})", url.clone());
                }
                Err(e) => {
                    Self::display_error(draw_ctx, e);
                }
            }
            in_chan_tx.send(TabMsg::SetProgress(0.0)).unwrap();
        };
        self.spawn_req(fut);
    }
    fn spawn_open_history(&mut self, item: HistoryItem) {
        let HistoryItem { url, cache, .. } = item;
        match cache {
            Some(cache) => self.spawn_open_cached(url, cache),
            None => self.spawn_open(url),
        }
    }
    fn spawn_open_cached(&mut self, url: Url, cache: Vec<u8>) {
        self.history.push(HistoryItem {
            url: url.clone(),
            cache: None,
            scroll_progress: 0.0,
        });

        let mut draw_ctx = self.draw_ctx.clone();
        let in_chan_tx = self.in_chan_tx.clone();
        let fut = async move {
            let buf = BufReader::new(cache.as_slice());
            draw_ctx.clear();
            let res = Self::display_gemini(&mut draw_ctx, buf).await;
            match res {
                Ok(cache) => {
                    info!("Loaded {} from cache", &url);
                    in_chan_tx.send(TabMsg::AddCache(cache)).unwrap();
                }
                Err(e) => Self::display_error(draw_ctx.clone(), e),
            }
        };
        self.spawn_req(fut);
    }
    pub fn back(&mut self) -> Result<()> {
        if self.history.len() <= 1 {
            bail!("Already at last item in history");
        }
        self.history.pop();
        match self.history.pop() {
            Some(item) => self.spawn_open_history(item),
            None => unreachable!(),
        }
        Ok(())
    }
    fn display_error(mut ctx: DrawCtx, error: anyhow::Error) {
        error!("{:?}", error);
        glibctx().spawn_local(async move {
            let error_text = format!("Geopard experienced an error:\n {}", error);
            let buffered = BufReader::new(error_text.as_bytes());
            Self::display_text(&mut ctx, buffered)
                .await
                .expect("Error while showing error in the text_view. This can't happen");
        })
    }
    pub fn handle_click(&mut self, handler: &Link) -> Result<()> {
        match handler {
            Link::Internal(link) => {
                let url = self.parse_link(&link)?;
                self.spawn_open(url);
            }
            Link::External(link) => {
                let url = self.parse_link(&link)?;
                gtk::show_uri(None, url.as_str(), 0)?;
            }
        }
        Ok(())
    }
    pub fn widget(&self) -> &gtk::ScrolledWindow {
        &self.scroll_win
    }
    fn get_coords_in_widget<T: IsA<gtk::Widget>>(widget: &T) -> (i32, i32) {
        let seat = gdk::Display::get_default()
            .unwrap()
            .get_default_seat()
            .unwrap();
        let device = seat.get_pointer().unwrap();
        let (_window, x, y, _) = WidgetExt::get_window(widget)
            .unwrap()
            .get_device_position(&device);
        (x, y)
    }
    fn extend_textview_menu(menu: &gtk::Menu, url: String, sender: flume::Sender<TabMsg>) {
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
    }
    fn bind_signals(&mut self) {
        let in_chan_tx = self.in_chan_tx.clone();
        self.draw_ctx
            .text_view
            .connect_event_after(move |text_view, e| {
                let event_is_click = (e.get_event_type() == gdk::EventType::ButtonRelease
                    || e.get_event_type() == gdk::EventType::TouchEnd)
                    && e.get_button() == Some(1);

                let has_selection = text_view.get_buffer().unwrap().get_has_selection();

                if event_is_click && !has_selection {
                    match Self::extract_linkhandler(text_view, e.get_coords().unwrap()) {
                        Ok(handler) => in_chan_tx.send(TabMsg::LineClicked(handler)).unwrap(),
                        Err(e) => warn!("{}", e),
                    }
                }
            });
        let in_chan = self.in_chan_tx.clone();
        self.draw_ctx
            .text_view
            .connect_populate_popup(move |text_view, widget| {
                Self::handle_right_click(text_view, widget, in_chan.clone());
            });
    }
    fn extract_linkhandler(text_view: &gtk::TextView, (x, y): (f64, f64)) -> Result<Link> {
        info!("Extracting linkhandler from clicked text");
        let (x, y) =
            text_view.window_to_buffer_coords(gtk::TextWindowType::Widget, x as i32, y as i32);

        let iter = text_view
            .get_iter_at_location(x as i32, y as i32)
            .context("Can't get text iter where clicked")?;

        for tag in iter.get_tags() {
            if let Some(url) = DrawCtx::get_linkhandler(&tag) {
                return Ok(url);
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
                Self::display_input(
                    &mut req.draw_ctx,
                    req.url.clone(),
                    &meta,
                    req.in_chan_tx.clone(),
                );
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
        let current_url = self
            .history
            .last()
            .expect("History item not found")
            .url
            .clone();
        let link_url = Url::options().base_url(Some(&current_url)).parse(link)?;
        Ok(link_url)
    }

    fn get_download_path(url: &Url) -> anyhow::Result<std::path::PathBuf> {
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
        let d_path = Self::get_download_path(&url)?;

        let mut buffer = Vec::with_capacity(8192);
        buffer.extend_from_slice(&[0; 8192]);

        let mut read = 0;
        let mut text_iter = ctx.text_buffer.get_end_iter();
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
                    debug!("lines {}", ctx.text_buffer.get_line_count());
                    let mut old_line_iter = ctx
                        .text_buffer
                        .get_iter_at_line(ctx.text_buffer.get_line_count() - 2);
                    ctx.text_buffer
                        .delete(&mut old_line_iter, &mut ctx.text_buffer.get_end_iter());
                    ctx.insert_paragraph(
                        &mut old_line_iter,
                        &format!("downloaded\t {}KB\n", read / 1000),
                    );
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    continue;
                }
                Err(e) => return Err(e.into()),
            }
        }
        let mut text_iter = ctx.text_buffer.get_end_iter();
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
            let mut text_iter = draw_ctx.text_buffer.get_end_iter();
            draw_ctx.insert_paragraph(&mut text_iter, &line);
            draw_ctx.insert_paragraph(&mut text_iter, "\n");
        }
    }

    fn display_input(ctx: &mut DrawCtx, url: Url, msg: &str, sender: flume::Sender<TabMsg>) {
        let text_buffer = &ctx.text_buffer;

        let mut iter = text_buffer.get_end_iter();
        ctx.insert_paragraph(&mut iter, &msg);
        ctx.insert_paragraph(&mut iter, "\n");

        let anchor = text_buffer
            .create_child_anchor(&mut text_buffer.get_end_iter())
            .unwrap();
        let text_input = gtk::Entry::new();
        text_input.set_hexpand(true);
        ctx.text_view.add_child_at_anchor(&text_input, &anchor);
        text_input.show();

        text_input.connect_activate(move |text_input| {
            let query = text_input.get_text().to_string();
            let mut url = url.clone();
            url.set_query(Some(&query));
            sender.send(TabMsg::Open(url)).unwrap();
        });
    }

    fn display_url_confirmation(ctx: &mut DrawCtx, url: &Url) {
        let mut text_iter = ctx.text_buffer.get_end_iter();
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
        let mut text_iter = draw_ctx.text_buffer.get_end_iter();

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
