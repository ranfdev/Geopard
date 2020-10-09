use anyhow::{bail, Context, Error};
use async_fs::File;
use futures::io::{BufReader, Cursor};
use futures::prelude::*;
use futures::task::LocalSpawnExt;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::Application;
use once_cell::sync::Lazy;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use url::Url;

mod common;
mod config;
mod gemini;

use common::{glibctx, Color, LossyTextRead, TextRender};

static USER_DATA_PATH: Lazy<std::path::PathBuf> = Lazy::new(|| {
    glib::get_user_data_dir()
        .expect("No user data dir")
        .join("geopard")
});

static USER_CONFIG_PATH: Lazy<std::path::PathBuf> = Lazy::new(|| {
    glib::get_user_config_dir()
        .expect("No user config dir")
        .join("geopard")
});

static FAVORITE_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| USER_DATA_PATH.join("bookmarks.gemini"));

static DOWNLOAD_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| glib::get_user_special_dir(glib::UserDirectory::Downloads).unwrap());

static SETTINGS_PATH: Lazy<std::path::PathBuf> = Lazy::new(|| USER_CONFIG_PATH.join("config.toml"));

static HISTORY_PATH: Lazy<std::path::PathBuf> = Lazy::new(|| USER_DATA_PATH.join("history.gemini"));

static DEFAULT_FAVORITES: &str = r"# Bookmarks
This is a gemini file where you can put all your bookmarks.
You can even edit this file in a text editor. That's how you
should remove bookmarks.

## Default bookmarks:
=> gemini://gemini.circumlunar.space/ Gemini project
=> gemini://rawtext.club:1965/~sloum/spacewalk.gmi Spacewalk aggregator
=> about:help About geopard + help

## Custom bookmarks:
";

static ABOUT_PAGE: &str = std::include_str!("../README.gemini");

const MARGIN: i32 = 20;

#[derive(Debug, PartialEq)]
pub enum Format {
    Gemini,
    Gopher,
}

#[derive(Debug, PartialEq)]
pub struct Cache {
    data: Vec<u8>,
    format: Format,
}

#[derive(Debug, PartialEq)]
pub struct HistoryItem {
    url: url::Url,
    cache: Option<Cache>,
}

pub enum AppWindowMsg {
    Open(String),
    LinkClicked(String),
    PushHistory(HistoryItem),
    ReplaceHistory(HistoryItem),
    BookmarkCurrent,
    UpdateUrlBar,
    Back,
}

struct AppWindow {
    history: Vec<HistoryItem>,
    client: gemini::Client,
    sender: glib::Sender<AppWindowMsg>,
    url_bar: gtk::SearchEntry,
    back_btn: gtk::Button,
    add_bookmark_btn: gtk::Button,
    show_bookmarks_btn: gtk::Button,
    page_ctx: common::Ctx,
    current_req: Option<futures::future::RemoteHandle<()>>,
}
impl AppWindow {
    pub fn new(app: &gtk::Application) -> gtk::ApplicationWindow {
        let config = futures::executor::block_on(async {
            Self::create_base_files().await.unwrap();
            let config: config::Config = toml::from_str(
                &async_fs::read_to_string(&*SETTINGS_PATH)
                    .await
                    .expect(&format!("Failed reading config from {:?}", &*SETTINGS_PATH)),
            )
            .expect("Failed parsing config");
            config
        });
        dbg!(&config);

        let window = gtk::ApplicationWindow::new(app);
        let view = gtk::Box::new(gtk::Orientation::Vertical, 0);
        let header_bar = gtk::HeaderBar::new();
        header_bar.set_title(Some("Geopard"));
        header_bar.set_show_close_button(true);

        let btn_box = gtk::ButtonBox::new(gtk::Orientation::Horizontal);
        btn_box.set_property_layout_style(gtk::ButtonBoxStyle::Expand);
        let back_btn =
            gtk::Button::from_icon_name(Some("go-previous-symbolic"), gtk::IconSize::Button);
        let add_bookmark_btn =
            gtk::Button::from_icon_name(Some("star-new-symbolic"), gtk::IconSize::Button);

        let show_bookmarks_btn =
            gtk::Button::from_icon_name(Some("view-list-symbolic"), gtk::IconSize::Button);

        btn_box.add(&back_btn);
        btn_box.add(&add_bookmark_btn);
        btn_box.add(&show_bookmarks_btn);

        header_bar.pack_start(&btn_box);

        let url_bar = gtk::SearchEntry::new();
        url_bar.set_hexpand(true);

        header_bar.set_custom_title(Some(&url_bar));

        window.set_titlebar(Some(&header_bar));
        window.add(&view);

        window.set_default_size(800, 600);

        let scroll_win = gtk::ScrolledWindow::new::<gtk::Adjustment, gtk::Adjustment>(None, None);
        scroll_win.set_vexpand(true);

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

        scroll_win.add(&text_view);

        let page_ctx = common::Ctx::new(text_view, config);

        view.add(&scroll_win);

        let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_HIGH);

        let mut this = Self {
            url_bar,
            page_ctx,
            back_btn,
            add_bookmark_btn,
            show_bookmarks_btn,
            history: vec![],
            client: gemini::ClientBuilder::new().redirect(true).build(),
            sender: sender.clone(),
            current_req: None,
        };

        this.bind_signals();

        receiver.attach(None, move |msg| this.handle_msg(msg));

        let bookmarks_url = format!("file://{}", FAVORITE_PATH.to_str().unwrap());
        sender.send(AppWindowMsg::Open(bookmarks_url)).unwrap();

        window
    }

    async fn create_base_files() -> anyhow::Result<()> {
        if !USER_DATA_PATH.exists() {
            async_fs::create_dir_all(&*USER_DATA_PATH)
                .await
                .context("Failed to create geopard data dir")?;
        }

        if !USER_CONFIG_PATH.exists() {
            async_fs::create_dir_all(&*USER_CONFIG_PATH)
                .await
                .context("Failed to create geopard config dir")?;
        }

        if !FAVORITE_PATH.exists() {
            File::create(&*FAVORITE_PATH)
                .await
                .context("Failed to create favorite.gemini")?;
            async_fs::write(&*FAVORITE_PATH, DEFAULT_FAVORITES)
                .await
                .context("Failed writing default bookmarks")?;
        }

        if !HISTORY_PATH.exists() {
            File::create(&*HISTORY_PATH)
                .await
                .context("Failed to create history.gemini")?;
        }

        if !SETTINGS_PATH.exists() {
            File::create(&*SETTINGS_PATH)
                .await
                .context("Failed to create config.toml")?;
            async_fs::write(
                &*SETTINGS_PATH,
                toml::to_string(&*config::DEFAULT_CONFIG).unwrap(),
            )
            .await
            .context("Failed writing example config")?;
        }

        Ok(())
    }
    async fn favorite(url: &str) -> anyhow::Result<()> {
        let mut file = async_fs::OpenOptions::new()
            .write(true)
            .append(true)
            .open(&*FAVORITE_PATH)
            .await
            .context("Opening favorite.gemini")?;

        let line_to_write = format!("=> {}\n", url);
        file.write_all(line_to_write.as_bytes())
            .await
            .context("Writing url to favourite.gemini")?;

        file.flush().await?;
        Ok(())
    }
    fn complete_url(url: &mut Url) {
        if url.scheme().is_empty() {
            url.set_scheme("gemini").unwrap();
        }
    }
    fn parse_link(&self, link: &str) -> Result<Url, url::ParseError> {
        let current_url = self.history.last().unwrap().url.clone();
        let mut link_url = Url::options().base_url(Some(&current_url)).parse(link)?;

        Self::complete_url(&mut link_url);
        Ok(link_url)
    }
    fn handle_msg(&mut self, msg: AppWindowMsg) -> glib::Continue {
        use AppWindowMsg::*;
        match msg {
            Open(url) => {
                let url = Url::parse(&url);
                match url {
                    Ok(url) => self.open_url(url),
                    Err(e) => {
                        Self::clear(&self.page_ctx);
                        Self::display_error(self.page_ctx.clone(), e.into());
                    }
                }
            }
            LinkClicked(url) => {
                let url = self.parse_link(&url);
                match url {
                    Ok(url) => self.open_url(url),
                    Err(e) => {
                        Self::clear(&self.page_ctx);
                        Self::display_error(self.page_ctx.clone(), e.into());
                    }
                }
            }
            PushHistory(item) => {
                self.history.push(item);
            }
            ReplaceHistory(item) => {
                self.history.pop();
                self.history.push(item);
            }
            UpdateUrlBar => {
                let HistoryItem { url, cache: _ } = &self.history.last().unwrap();
                let mut hasher = DefaultHasher::new();
                url.host().hash(&mut hasher);
                let hash = hasher.finish();

                if self.page_ctx.config.colors {
                    self.set_special_color_from_hash(hash);
                }

                println!("HASH IS {:x}", hash);
                self.url_bar.set_text(url.as_str());
            }
            Back => {
                self.back();
            }
            BookmarkCurrent => {
                let url = self.history.last().unwrap().url.to_string();
                let ctx = self.page_ctx.clone();
                glibctx().spawn_local(async move {
                    Self::favorite(&url)
                        .await
                        .unwrap_or_else(|e| Self::display_error(ctx, e));
                });
            }
        }
        glib::Continue(true)
    }
    fn set_special_color_from_hash(&self, hash: u64) {
        let color1 = Color(
            (hash & 255) as u8,
            (hash >> 8 & 255) as u8,
            (hash >> 16 & 255) as u8,
        );

        let hash = hash >> 24;
        let color2 = Color(
            (hash & 255) as u8,
            (hash >> 8 & 255) as u8,
            (hash >> 16 & 255) as u8,
        );

        let stylesheet = format!(
            "
                    headerbar {{
                        transition: 500ms;
                        background: linear-gradient(#{:x}, #{:x});
                    }}
                    text {{
                        transition: 500ms;
                        background: rgba({},{},{}, 0.05);
                    }}
                    ",
            color1, color2, color2.0, color2.1, color2.2
        );
        Self::add_stylesheet(&stylesheet);
    }
    fn add_stylesheet(stylesheet: &str) {
        // TODO: Adding a provider and keeping it in memory forever
        // is a memory leak. Fortunately, it's small

        let provider = gtk::CssProvider::new();
        provider
            .load_from_data(stylesheet.as_bytes())
            .expect("Failed loading stylesheet");
        gtk::StyleContext::add_provider_for_screen(
            &gdk::Screen::get_default().unwrap(),
            &provider,
            gtk::STYLE_PROVIDER_PRIORITY_APPLICATION,
        );
    }
    fn back(&mut self) {
        dbg!(self
            .history
            .iter()
            .map(|item| item.url.clone())
            .collect::<Vec<Url>>());
        if self.history.len() > 1 {
            // remove current url
            self.history.pop();
        }
        match self.history.last() {
            Some(HistoryItem {
                url: _,
                cache: Some(cache),
            }) => self.open_cached(cache),
            Some(HistoryItem { url, cache: None }) => {
                let url = url.clone();
                self.history.pop();
                self.open_url(url.clone())
            }
            _ => println!("Can't go back, end of history"),
        }
    }
    fn open_cached(&self, cache: &Cache) {
        let sender = self.sender.clone();
        sender.send(AppWindowMsg::UpdateUrlBar).unwrap();
        let ctx = self.page_ctx.clone();

        Self::clear(&self.page_ctx);
        glibctx().block_on(async move {
            if let Cache {
                data,
                format: Format::Gemini,
            } = cache
            {
                let reader = BufReader::new(Cursor::new(&data));
                Self::display_gemini(ctx, reader).await.unwrap();
            }
        })
    }

    fn display_input(ctx: common::Ctx, url: Url, msg: &str, sender: glib::Sender<AppWindowMsg>) {
        let text_buffer = &ctx.text_buffer;

        let mut iter = text_buffer.get_end_iter();
        ctx.insert_paragraph(&mut iter, &msg);
        ctx.insert_paragraph(&mut iter, "\n");

        let anchor = text_buffer
            .create_child_anchor(&mut text_buffer.get_end_iter())
            .unwrap();
        let text_input = gtk::Entry::new();
        ctx.text_view.add_child_at_anchor(&text_input, &anchor);
        text_input.show();

        text_input.connect_activate(move |text_input| {
            let query = text_input.get_text().to_string();
            let mut url = url.clone();
            url.set_query(Some(&query));
            sender.send(AppWindowMsg::Open(url.to_string())).unwrap();
        });
    }
    async fn open_file_url(ctx: common::Ctx, url: Url) -> anyhow::Result<Option<Cache>> {
        let path = url.to_file_path().unwrap();
        let file = File::open(&path).await?;
        let lines = BufReader::new(file);
        match path.extension().map(|x| x.to_str()) {
            Some(Some("gmi")) | Some(Some("gemini")) => {
                Self::display_gemini(ctx, lines).await?;
            }
            _ => {
                Self::display_text(ctx, lines).await?;
            }
        }
        Ok(None)
    }
    fn open_url(&mut self, url: Url) {
        // Drop (and stop) old request asap
        self.current_req = None;

        println!("Good url: {}", url);
        let sender = self.sender.clone();
        let client = self.client.clone();
        let url_bar = self.url_bar.clone();
        let ctx = self.page_ctx.clone();

        sender
            .send(AppWindowMsg::PushHistory(HistoryItem {
                url: url.clone(),
                cache: None,
            }))
            .unwrap();
        sender.send(AppWindowMsg::UpdateUrlBar).unwrap();

        Self::clear(&ctx);

        let ctx_clone = ctx.clone();
        let handler = glibctx().spawn_local_with_handle(async move {
            let sender_clone = sender.clone();
            let url = url.clone();
            let cache = match url.scheme() {
                "about" => {
                    let lines = BufReader::new(ABOUT_PAGE.as_bytes());
                    Self::display_gemini(ctx_clone, lines).await.map(|_| None)
                }
                "file" => {
                    Self::while_loading(&url_bar, Self::open_file_url(ctx_clone, url.clone())).await
                }
                "gemini" => {
                    Self::while_loading(
                        &url_bar,
                        Self::open_gemini_url(ctx_clone, url.clone(), client, sender_clone.clone()),
                    )
                    .await
                }
                _ => {
                    println!("Scheme not supported");
                    Self::display_url_confirmation(ctx_clone, url.clone());
                    Ok(None)
                }
            }
            .unwrap_or_else(|e| {
                Self::display_error(ctx, e);
                None
            });
            sender
                .send(AppWindowMsg::ReplaceHistory(HistoryItem { url, cache }))
                .unwrap();
        });
        self.current_req = Some(handler.unwrap());
    }
    async fn while_loading<F: Future<Output = T>, T>(search_entry: &gtk::SearchEntry, f: F) -> T {
        // Gemini doesn't give any hint about the page size,
        // so this progress is fake. It only means something
        // is still loading
        search_entry.set_progress_fraction(0.5);
        let res = f.await;
        // Set to zero to hide the progress bar
        search_entry.set_progress_fraction(0.0);
        res
    }

    async fn open_gemini_url(
        ctx: common::Ctx,
        url: Url,
        client: gemini::Client,
        sender: glib::Sender<AppWindowMsg>,
    ) -> anyhow::Result<Option<Cache>> {
        let res: gemini::Response = client
            .fetch(url.as_str())
            .await
            .map_err(|e| Error::from(e))?;

        use gemini::Status::*;
        let meta = res.meta().to_owned();
        let status = res.status();
        let result = match status {
            Input(_) => {
                Self::display_input(ctx, url.clone(), &meta, sender);
                Ok(None)
            }
            Success(_) => {
                let body = res.body().unwrap();
                let buffered = BufReader::new(body);
                if meta.find("text/gemini").is_some() {
                    Self::display_gemini(ctx, buffered).await.map(|c| Some(c))
                } else if meta.find("text").is_some() {
                    Self::display_text(ctx, buffered).await.map(|_| None)
                } else {
                    Self::display_download(ctx, url.clone(), buffered)
                        .await
                        .map(|_| None)
                }
            }
            Redirect(_) => bail!("Redirected more than 5 times"),
            TempFail(_) => bail!("Temporary server failure"),
            PermFail(_) => bail!("Permanent server failure"),
            CertRequired(_) => bail!("A certificate is required to access this page"),
        };
        result
    }
    fn get_download_path(url: &Url) -> Result<std::path::PathBuf, Error> {
        let file_name = url
            .path_segments()
            .context("Can't divide url in segments")?
            .last()
            .context("Can't get last url segment")?;

        let mut file_name = std::path::PathBuf::from(file_name);
        loop {
            let mut d_path = DOWNLOAD_PATH.join(&file_name);
            if d_path.exists() {
                let mut name_no_ext = file_name
                    .file_stem()
                    .context("Can't get file_stem (filename without ext)")?
                    .to_owned();
                let empty_os_string = &std::ffi::OsString::from("");
                let ext = d_path.extension().unwrap_or(empty_os_string);
                file_name = {
                    name_no_ext.push("_new_.");
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
        mut ctx: common::Ctx,
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
            &format!("Writing to {:?}\n", d_path.as_os_str()),
        );
        ctx.insert_paragraph(
            &mut text_iter,
            "To interrupt the download, leave this page\n",
        );
        ctx.insert_paragraph(&mut text_iter, "Downloaded\t Kb\n");

        let mut file = File::create(&d_path).await?;
        loop {
            match stream.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => {
                    file.write_all(&buffer[..n]).await?;
                    read += n;
                    println!("Lines {}", ctx.text_buffer.get_line_count());
                    let mut old_line_iter = ctx
                        .text_buffer
                        .get_iter_at_line(ctx.text_buffer.get_line_count() - 2);
                    ctx.text_buffer
                        .delete(&mut old_line_iter, &mut ctx.text_buffer.get_end_iter());
                    ctx.insert_paragraph(
                        &mut old_line_iter,
                        &format!("Downloaded\t {}\n", read / 1000),
                    );
                }
                Err(e) if e.kind() == std::io::ErrorKind::Interrupted => {
                    continue;
                }
                Err(e) => {
                    Err(e)?;
                }
            }
        }
        let mut text_iter = ctx.text_buffer.get_end_iter();
        ctx.insert_paragraph(&mut text_iter, "Download finished!\n");
        let downloaded_file_url = format!("file://{}", d_path.as_os_str().to_str().unwrap());
        ctx.insert_external_link(
            &mut text_iter,
            &downloaded_file_url,
            Some("Open with default program"),
        );

        Ok(())
    }
    fn display_url_confirmation(mut ctx: common::Ctx, url: Url) {
        let mut text_iter = ctx.text_buffer.get_end_iter();
        ctx.insert_paragraph(
            &mut text_iter,
            "Geopard doesn't support this url scheme. 
If you want to open the following link in an external application,
click on the link below\n",
        );

        ctx.insert_external_link(&mut text_iter, url.as_str(), Some(url.as_str()));
    }

    async fn display_gemini<T: AsyncBufRead + Unpin>(
        ctx: common::Ctx,
        mut reader: T,
    ) -> anyhow::Result<Cache> {
        let mut parser = gemini::Parser::new();
        let mut render_engine = gemini::Renderer::new(ctx);

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
            render_engine.render(token);
        }
        Ok(Cache {
            data: data.into_bytes(),
            format: Format::Gemini,
        })
    }

    fn display_error(ctx: common::Ctx, error: anyhow::Error) {
        println!("{:?}", error);
        glibctx().spawn_local(async move {
            let error_text = format!("Geopard experienced an error:\n {}", error);
            let buffered = BufReader::new(error_text.as_bytes());
            Self::display_text(ctx, buffered)
                .await
                .expect("Error while showing error in the text_view. This can't happen");
        })
    }
    fn clear(ctx: &common::Ctx) {
        let b = &ctx.text_buffer;
        b.delete(&mut b.get_start_iter(), &mut b.get_end_iter());
    }
    async fn display_text(
        ctx: common::Ctx,
        mut stream: impl AsyncBufRead + Unpin,
    ) -> anyhow::Result<()> {
        let mut line = String::with_capacity(1024);
        loop {
            line.clear();
            let n = stream.read_line_lossy(&mut line).await?;
            if n == 0 {
                break Ok(());
            }
            let mut text_iter = ctx.text_buffer.get_end_iter();
            ctx.insert_paragraph(&mut text_iter, &line);
            ctx.insert_paragraph(&mut text_iter, "\n");
        }
    }
    fn bind_signals(&self) {
        let sender = self.sender.clone();
        let search_entry_clone = self.url_bar.clone();

        let sender_clone = sender.clone();
        self.url_bar.connect_activate(move |_| {
            let entry_text = search_entry_clone.get_text().to_string();
            sender_clone.send(AppWindowMsg::Open(entry_text)).unwrap();
        });

        let sender_clone = sender.clone();
        self.url_bar.connect_focus_out_event(move |_, _| {
            sender_clone.send(AppWindowMsg::UpdateUrlBar).unwrap();
            glib::signal::Inhibit(false)
        });

        let sender_clone = sender.clone();
        self.back_btn.connect_clicked(move |_| {
            sender_clone.send(AppWindowMsg::Back).unwrap();
        });

        let sender_clone = sender.clone();
        self.add_bookmark_btn.connect_clicked(move |_| {
            sender_clone.send(AppWindowMsg::BookmarkCurrent).unwrap();
        });

        let sender_clone = sender.clone();
        self.show_bookmarks_btn.connect_clicked(move |_| {
            let page_to_open = format!("file://{}", FAVORITE_PATH.to_str().unwrap());
            sender_clone.send(AppWindowMsg::Open(page_to_open)).unwrap();
        });

        let sender_clone = sender.clone();
        let page_ctx = self.page_ctx.clone();
        self.page_ctx
            .text_view
            .connect_event_after(move |text_view, e| {
                if e.get_event_type() != gdk::EventType::ButtonRelease
                    && e.get_event_type() != gdk::EventType::TouchEnd
                {
                    return;
                }

                if text_view.get_buffer().unwrap().get_has_selection() {
                    return;
                }

                let (x, y) = e.get_coords().unwrap();
                let (x, y) = text_view.window_to_buffer_coords(
                    gtk::TextWindowType::Widget,
                    x as i32,
                    y as i32,
                );
                let url = text_view
                    .get_iter_at_location(x as i32, y as i32)
                    .and_then(|iter| {
                        for tag in iter.get_tags() {
                            if tag.get_property_name().map(|s| s.to_string()).as_deref()
                                == Some("a")
                            {
                                dbg!("clicked link at line ", &iter.get_line());
                                return page_ctx
                                    .links
                                    .borrow()
                                    .get(&iter.get_line())
                                    .map(|s| s.to_owned());
                            }
                        }
                        None
                    });

                use common::LinkHandler;
                match url {
                    Some(LinkHandler::Internal(url)) => {
                        sender_clone.send(AppWindowMsg::LinkClicked(url)).unwrap()
                    }
                    Some(LinkHandler::External(url)) => {
                        gtk::show_uri(None, url.as_str(), 0).unwrap()
                    }
                    _ => {}
                }
            });
    }
}

fn main() {
    gtk::init().unwrap();

    let application = Application::new(
        Some("com.ranfdev.app.geopard"),
        gio::ApplicationFlags::FLAGS_NONE,
    )
    .expect("Failed to init gtk app");

    let app_clone = application.clone();
    application.connect_activate(move |_| {
        let app_window = AppWindow::new(&app_clone);
        app_window.show_all();
        app_window.present();
    });

    let ret = application.run(&std::env::args().collect::<Vec<String>>());
    std::process::exit(ret);
}
