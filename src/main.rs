use anyhow::{bail, Context, Error};
use async_std::fs;
use async_std::io::prelude::*;
use async_std::io::{BufRead, BufReader};
use async_std::prelude::*;
use async_trait::async_trait;
use futures::task::LocalSpawnExt;
use gio::prelude::*;
use gtk::prelude::*;
use gtk::Application;
use once_cell::sync::Lazy;
use regex::Regex;
use std::borrow::Cow;
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
use url::Url;

mod gemini_module;
use gemini_module::{Client, ClientBuilder};

static USER_DATA_PATH: Lazy<std::path::PathBuf> = Lazy::new(|| {
    glib::get_user_data_dir()
        .expect("No user data dir path")
        .join("geopard")
});

static FAVORITE_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| USER_DATA_PATH.join("bookmarks.gemini"));

static DOWNLOAD_PATH: Lazy<std::path::PathBuf> =
    Lazy::new(|| glib::get_user_special_dir(glib::UserDirectory::Downloads).unwrap());

static HISTORY_PATH: Lazy<std::path::PathBuf> = Lazy::new(|| USER_DATA_PATH.join("history.gemini"));

static DEFAULT_FAVORITES: &str = r"# Bookmarks
This is a gemini file where you can put all your bookmarks.
You can even edit this file in a text editor. That's how you
should remove bookmarks.

## Default bookmarks:
=> gemini://gemini.circumlunar.space/ Gemini project
=> gemini://rawtext.club:1965/~sloum/spacewalk.gmi Spacewalk aggregator

## Custom bookmarks:
";

static ABOUT_PAGE: &str = std::include_str!("../README.gemini");

static R_GEMINI_LINK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^=>\s*(?P<href>\S*)\s*(?P<label>.*)").unwrap());

fn glibctx() -> glib::MainContext {
    glib::MainContext::default()
}
#[async_trait(?Send)]
trait LossyTextRead {
    async fn read_line_lossy(&mut self, mut buf: &mut String) -> std::io::Result<usize>;
}

#[async_trait(?Send)]
impl<T: BufRead + Unpin> LossyTextRead for T {
    async fn read_line_lossy(&mut self, buf: &mut String) -> std::io::Result<usize> {
        // This is safe because we treat buf as a mut Vec to read the data, BUT,
        // we check if it's valid utf8 using String::from_utf8_lossy.
        // If it's not valid utf8, we swap our buf with the newly allocated and
        // safe string returned from String::from_utf8_lossy
        //
        // In the implementation of BufReader::read_line, they talk about some things about
        // panic handling, which I don't understand currently. Whatever...
        unsafe {
            let mut vec_buf = buf.as_mut_vec();
            let n = self.read_until(b'\n', &mut vec_buf).await?;

            let correct_string = String::from_utf8_lossy(&vec_buf);
            if let Cow::Owned(valid_utf8_string) = correct_string {
                // Yes, I know performance this requires useless copying.
                // This code will only be executed when invalid utf8 is found, so i
                // consider this as good enough
                buf.push_str(&valid_utf8_string);
            }
            Ok(n)
        }
    }
}

struct Color(u8, u8, u8);

impl std::fmt::LowerHex for Color {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{:02x}", self.0)?;
        write!(f, "{:02x}", self.1)?;
        write!(f, "{:02x}", self.2)?;
        Ok(())
    }
}

//#[derive(thiserror::Error, Debug)]
//enum Error {
//    #[error("{self:?}")]
//    PathError(#[from] url::ParseError),
//    #[error("{self:?}")]
//    IOError(#[from] std::io::Error),
//    #[error("{self:?}")]
//    FetchFailed(#[from] gemini_module::Error),
//    #[error("{self:?}")]
//    BadStatus {
//        url: Url,
//        status: gemini_module::Status,
//        meta: String,
//        description: String,
//    },
//}

type HistoryItem = Url; // I want to add some cache to every HistoryItem.
                        // For now I'm saving just the url

enum AppWindowMsg {
    Open(String),
    LinkClicked(String),
    PushHistory(Url),
    BookmarkCurrent,
    UpdateUrlBar,
    Back,
}

struct AppWindow {
    history: Vec<HistoryItem>,
    client: Client,
    sender: glib::Sender<AppWindowMsg>,
    url_bar: gtk::SearchEntry,
    back_btn: gtk::Button,
    add_bookmark_btn: gtk::Button,
    show_bookmarks_btn: gtk::Button,
    text_view: gtk::TextView,
    current_req: Option<futures::future::RemoteHandle<()>>,
}
impl AppWindow {
    pub fn new(app: &gtk::Application) -> gtk::ApplicationWindow {
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
            .top_margin(20)
            .left_margin(20)
            .right_margin(20)
            .bottom_margin(20)
            .indent(2)
            .editable(false)
            .cursor_visible(false)
            .wrap_mode(gtk::WrapMode::WordChar)
            .build();

        scroll_win.add(&text_view);

        view.add(&scroll_win);

        let (sender, receiver) = glib::MainContext::channel(glib::PRIORITY_HIGH);

        let mut this = Self {
            url_bar,
            back_btn,
            add_bookmark_btn,
            show_bookmarks_btn,
            text_view,
            history: vec![],
            client: ClientBuilder::new().redirect(true).build(),
            sender: sender.clone(),
            current_req: None,
        };

        this.create_base_files().unwrap();
        this.bind_signals();

        receiver.attach(None, move |msg| this.handle_msg(msg));

        let bookmarks_url = format!("file://{}", FAVORITE_PATH.to_str().unwrap());
        sender.send(AppWindowMsg::Open(bookmarks_url)).unwrap();

        window
    }

    fn create_base_files(&self) -> anyhow::Result<()> {
        if !USER_DATA_PATH.exists() {
            std::fs::create_dir(&*USER_DATA_PATH).context("Failed to create geopard data dir")?;
        }

        if !FAVORITE_PATH.exists() {
            std::fs::File::create(&*FAVORITE_PATH).context("Failed to create favorite.gemini")?;
            std::fs::write(&*FAVORITE_PATH, DEFAULT_FAVORITES)
                .context("Failed writing default bookmarks")?;
        }

        if !HISTORY_PATH.exists() {
            std::fs::File::create(&*HISTORY_PATH).context("Failed to create history.gemini")?;
        }

        Ok(())
    }
    async fn favorite(url: &str) -> anyhow::Result<()> {
        let mut file = fs::OpenOptions::new()
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
        let current_url = self.history.last().unwrap().clone();
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
                        Self::clear(&self.text_view);
                        Self::display_error(e.into(), self.text_view.clone());
                    }
                }
            }
            LinkClicked(url) => {
                let url = self.parse_link(&url);
                match url {
                    Ok(url) => self.open_url(url),
                    Err(e) => {
                        Self::clear(&self.text_view);
                        Self::display_error(e.into(), self.text_view.clone());
                    }
                }
            }
            PushHistory(url) => {
                self.history.push(url);
            }
            UpdateUrlBar => {
                let url = &self.history.last().unwrap();
                let mut hasher = DefaultHasher::new();
                url.host().hash(&mut hasher);
                let hash = hasher.finish();

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

                let css_string = format!(
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
                let provider = gtk::CssProvider::new();
                provider.load_from_data(css_string.as_bytes()).unwrap();

                // TODO: Adding a provider and keeping it in memory forever
                // is a memory leak. Fortunately, it's small
                gtk::StyleContext::add_provider_for_screen(
                    &gdk::Screen::get_default().unwrap(),
                    &provider,
                    1000,
                );

                println!("HASH IS {:x}", hash);
                self.url_bar.set_text(url.as_str());
            }
            Back => {
                self.back();
            }
            BookmarkCurrent => {
                let url = self.history.last().unwrap().to_string();
                let text_view = self.text_view.clone();
                glibctx().spawn_local(async move {
                    Self::favorite(&url)
                        .await
                        .unwrap_or_else(|e| Self::display_error(e, text_view));
                });
            }
        }
        glib::Continue(true)
    }
    fn back(&mut self) {
        if self.history.len() > 1 {
            // remove current url
            self.history.pop();
        }
        if let Some(url) = self.history.pop() {
            self.open_url(url.clone());
        } else {
            println!("Can't go back, history empty");
        }
    }
    fn buffer(text_view: &gtk::TextView) -> gtk::TextBuffer {
        match text_view.get_buffer() {
            Some(b) => b,
            None => Self::clear(&text_view),
        }
    }
    fn display_input(
        url: Url,
        msg: &str,
        text_view: gtk::TextView,
        sender: glib::Sender<AppWindowMsg>,
    ) {
        let text_buffer = Self::buffer(&text_view);

        let mut iter = text_buffer.get_end_iter();
        text_buffer.insert(&mut iter, &msg);
        text_buffer.insert(&mut iter, "\n");

        let anchor = text_buffer
            .create_child_anchor(&mut text_buffer.get_end_iter())
            .unwrap();
        let text_input = gtk::Entry::new();
        text_view.add_child_at_anchor(&text_input, &anchor);
        text_input.show();

        text_input.connect_activate(move |text_input| {
            let query = text_input.get_text().to_string();
            let mut url = url.clone();
            url.set_query(Some(&query));
            sender.send(AppWindowMsg::Open(url.to_string())).unwrap();
        });
    }
    async fn open_file_url(
        url: Url,
        text_view: gtk::TextView,
        sender: glib::Sender<AppWindowMsg>,
    ) -> anyhow::Result<()> {
        let path = url.to_file_path().unwrap();
        let file = fs::File::open(&path).await?;
        let lines = BufReader::new(file);
        let sender_clone = sender.clone();
        match path.extension().map(|x| x.to_str()) {
            Some(Some("gmi")) | Some(Some("gemini")) => {
                Self::display_gemini(lines, text_view.clone(), sender_clone).await
            }
            _ => Self::display_text(lines, text_view.clone()).await,
        }
    }
    fn open_url(&mut self, url: Url) {
        // Drop (and stop) old request asap
        self.current_req = None;

        println!("Good url: {}", url);
        let sender = self.sender.clone();
        let text_view = self.text_view.clone();
        let client = self.client.clone();
        let url_bar = self.url_bar.clone();

        sender.send(AppWindowMsg::PushHistory(url.clone())).unwrap();
        sender.send(AppWindowMsg::UpdateUrlBar).unwrap();

        Self::clear(&text_view);
        let handler = glibctx().spawn_local_with_handle(async move {
            match url.scheme() {
                "about" => {
                    let lines = BufReader::new(ABOUT_PAGE.as_bytes());
                    let sender_clone = sender.clone();
                    Self::display_gemini(lines, text_view.clone(), sender_clone).await
                }
                "file" => {
                    Self::while_loading(
                        &url_bar,
                        Self::open_file_url(url, text_view.clone(), sender),
                    )
                    .await
                }
                "gemini" => {
                    let sender_clone = sender.clone();
                    let text_view_clone = text_view.clone();
                    Self::while_loading(
                        &url_bar,
                        Self::open_gemini_url(url, client, text_view_clone, sender_clone.clone()),
                    )
                    .await
                }
                _ => {
                    println!("Scheme not supported");
                    Self::display_url_confirmation(url, text_view.clone());
                    Ok(())
                }
            }
            .unwrap_or_else(|e| Self::display_error(e, text_view));
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
        url: Url,
        client: gemini_module::Client,
        text_view: gtk::TextView,
        sender: glib::Sender<AppWindowMsg>,
    ) -> anyhow::Result<()> {
        let res: gemini_module::Response = client
            .fetch(url.as_str())
            .await
            .map_err(|e| Error::from(e))?;
        use gemini_module::Status::*;
        let sender_clone = sender.clone();
        let meta = res.meta().to_owned();
        let status = res.status();
        let result = match status {
            Input(_) => {
                Self::display_input(url.clone(), &meta, text_view, sender);
                Ok(())
            }
            Success(_) => {
                let body = res.body().unwrap();
                let buffered = BufReader::new(body);
                if meta.find("text/gemini").is_some() {
                    Self::display_gemini(buffered, text_view.clone(), sender_clone).await
                } else if meta.find("text").is_some() {
                    Self::display_text(buffered, text_view.clone()).await
                } else {
                    Self::display_download(url.clone(), buffered, text_view.clone()).await
                }
                .unwrap_or_else(|e| Self::display_error(e, text_view));
                Ok(())
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
    async fn display_download<T: Read + Unpin>(
        url: Url,
        mut stream: T,
        text_view: gtk::TextView,
    ) -> anyhow::Result<()> {
        let text_buffer = Self::buffer(&text_view);

        let d_path = Self::get_download_path(&url)?;

        let mut buffer = Vec::with_capacity(8192);
        buffer.extend_from_slice(&[0; 8192]);

        let mut read = 0;
        let mut text_iter = text_buffer.get_end_iter();
        text_buffer.insert(
            &mut text_iter,
            &format!("Writing to {:?}\n", d_path.as_os_str()),
        );
        text_buffer.insert(
            &mut text_iter,
            "To interrupt the download, leave this page\n",
        );
        text_buffer.insert(&mut text_iter, "Downloaded\t Kb\n");

        let mut file = fs::File::create(&d_path).await?;
        loop {
            match stream.read(&mut buffer).await {
                Ok(0) => break,
                Ok(n) => {
                    file.write_all(&buffer[..n]).await?;
                    read += n;
                    println!("Lines {}", text_buffer.get_line_count());
                    let mut old_line_iter =
                        text_buffer.get_iter_at_line(text_buffer.get_line_count() - 2);
                    text_buffer.delete(&mut old_line_iter, &mut text_buffer.get_end_iter());
                    text_buffer.insert(
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
        let mut text_iter = text_buffer.get_end_iter();
        text_buffer.insert(&mut text_iter, "Download finished!\n");
        let anchor = text_buffer.create_child_anchor(&mut text_iter).unwrap();
        let downloaded_file_url = format!("file://{}", d_path.as_os_str().to_str().unwrap());
        let btn =
            gtk::LinkButton::with_label(&downloaded_file_url, Some("Open with default program"));
        text_view.add_child_at_anchor(&btn, &anchor);
        btn.show();

        Ok(())
    }
    fn display_url_confirmation(url: Url, text_view: gtk::TextView) {
        let text_buffer = Self::buffer(&text_view);
        let mut text_iter = text_buffer.get_end_iter();
        text_buffer.insert(
            &mut text_iter,
            "Geopard doesn't support this url scheme. 
If you want to open the following link in an external application,
click on the link below\n",
        );

        let btn = gtk::LinkButton::new(url.as_ref());
        let anchor = text_buffer.create_child_anchor(&mut text_iter).unwrap();
        text_view.add_child_at_anchor(&btn, &anchor);
        btn.show();
    }
    fn insert_head(text_buffer: &gtk::TextBuffer, mut text_iter: &mut gtk::TextIter, line: &str) {
        let n = line.chars().filter(|c| *c == '#').count();
        let line = line.trim_start_matches('#').trim_start();
        let line = glib::markup_escape_text(&line);
        let size = match n {
            1 => "xx-large",
            2 => "x-large",
            3 => "large",
            _ => "medium",
        };
        text_buffer.insert_markup(
            &mut text_iter,
            &format!(r#"<span weight="800" size="{}">{}</span>"#, size, &line),
        );
    }

    fn insert_citation(
        text_buffer: &gtk::TextBuffer,
        mut text_iter: &mut gtk::TextIter,
        line: &str,
    ) {
        let line = glib::markup_escape_text(&line);
        text_buffer.insert_markup(&mut text_iter, &format!(r#"<i>{}</i>"#, &line));
    }

    fn insert_pre(text_buffer: &gtk::TextBuffer, mut text_iter: &mut gtk::TextIter, line: &str) {
        let line = glib::markup_escape_text(&line);
        text_buffer.insert_markup(
            &mut text_iter,
            &format!(r#"<span font_family="monospace">{}</span>"#, line),
        );
    }

    async fn display_gemini<T: BufRead + Unpin>(
        mut reader: T,
        text_view: gtk::TextView,
        sender: glib::Sender<AppWindowMsg>,
    ) -> anyhow::Result<()> {
        println!("Displaying gemini text");
        let text_buffer = Self::buffer(&text_view);
        let sender_clone = sender.clone();
        let mut line = String::with_capacity(1024);
        let mut text_iter = text_buffer.get_end_iter();

        let mut inside_pre = false;
        loop {
            line.clear();
            let n = reader.read_line_lossy(&mut line).await?;
            if n == 0 {
                break Ok(());
            }

            if line.starts_with("```") {
                inside_pre = !inside_pre;
            } else if inside_pre {
                Self::insert_pre(&text_buffer, &mut text_iter, &line);
            } else if line.starts_with("#") {
                Self::insert_head(&text_buffer, &mut text_iter, &line);
            } else if line.starts_with(">") {
                Self::insert_citation(&text_buffer, &mut text_iter, &line);
            } else if let Some(captures) = R_GEMINI_LINK.captures(&line) {
                // Insert LinkButton
                let btn = match (captures.name("href"), captures.name("label")) {
                    (Some(m_href), Some(m_label)) if !m_label.as_str().is_empty() => {
                        gtk::LinkButton::with_label(m_href.as_str(), Some(m_label.as_str()))
                    }
                    (Some(m_href), _) => {
                        gtk::LinkButton::with_label(m_href.as_str(), Some(m_href.as_str()))
                    }
                    _ => gtk::LinkButton::with_label("", Some(&line)),
                };

                let sender_clone = sender_clone.clone();
                btn.connect_activate_link(move |btn| {
                    let btn_url = btn.get_uri();
                    if let Some(url) = btn_url {
                        sender_clone
                            .send(AppWindowMsg::LinkClicked(url.to_string()))
                            .unwrap();
                    }
                    gtk::Inhibit(true)
                });
                let anchor = text_buffer.create_child_anchor(&mut text_iter).unwrap();
                text_view.add_child_at_anchor(&btn, &anchor);
                btn.show();
                text_buffer.insert(&mut text_iter, "\n");
            } else {
                text_buffer.insert(&mut text_iter, &line);
            }
        }
    }
    fn display_error(error: anyhow::Error, text_view: gtk::TextView) {
        println!("{:?}", error);
        glibctx().spawn_local(async move {
            let error_text = format!("Geopard experienced an error:\n {}", error);
            let buffered = BufReader::new(error_text.as_bytes());
            Self::display_text(buffered, text_view.clone())
                .await
                .expect("Error while showing error in the text_view. This can't happen");
        })
    }
    fn clear(text_view: &gtk::TextView) -> gtk::TextBuffer {
        let text_buffer = gtk::TextBuffer::new::<gtk::TextTagTable>(None);
        text_view.set_buffer(Some(&text_buffer));
        text_buffer
    }
    async fn display_text(
        mut stream: impl BufRead + Unpin,
        text_view: gtk::TextView,
    ) -> anyhow::Result<()> {
        let text_buffer = Self::buffer(&text_view);
        let mut line = String::with_capacity(1024);
        loop {
            line.clear();
            let n = stream.read_line_lossy(&mut line).await?;
            if n == 0 {
                break Ok(());
            }
            let mut text_iter = text_buffer.get_end_iter();
            text_buffer.insert(&mut text_iter, &line);
            text_buffer.insert(&mut text_buffer.get_end_iter(), "\n");
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
