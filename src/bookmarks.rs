use std::collections::BTreeMap;
use std::path::Path;

use anyhow::{Context, Ok};
use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use toml;

// todo!(bookmarks): replace bookmarks.bookmarks.insert() with bookmarks.insert_bookmark()
pub static DEFAULT_BOOKMARKS: Lazy<Bookmarks> = Lazy::new(|| {
    let mut bookmarks = Bookmarks::default();

    bookmarks.bookmarks.insert(
        1.to_string(),
        BookmarkBuilder::new()
            .title("Gemini Project")
            .url("gemini://geminiprotocol.net")
            .build(),
    );

    bookmarks.bookmarks.insert(
        2.to_string(),
        BookmarkBuilder::new()
            .title("Spacewalk aggregator")
            .url("gemini://rawtext.club:1965/~sloum/spacewalk.gmi")
            .build(),
    );

    bookmarks.bookmarks.insert(
        3.to_string(),
        BookmarkBuilder::new()
            .title("About geopard + help")
            .url("about:help")
            .build(),
    );

    bookmarks
});

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct Bookmark {
    title: String,
    description: Option<String>,
    url: String,
}

#[derive(Clone, Default, Debug)]
pub struct BookmarkBuilder {
    title: String,
    description: Option<String>,
    url: String,
}

#[derive(Clone, Default, Serialize, Deserialize, Debug)]
pub struct Bookmarks {
    #[serde(rename = "bookmark")]
    pub bookmarks: BTreeMap<String, Bookmark>,
}

impl BookmarkBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn title(mut self, title: &str) -> Self {
        self.title = String::from(title);
        self
    }

    pub fn description(mut self, description: Option<&str>) -> Self {
        match description {
            Some(desc) => self.description = Some(String::from(desc)),
            None => self.description = None,
        }
        self
    }

    pub fn url(mut self, url: &str) -> Self {
        self.url = String::from(url);
        self
    }

    pub fn build(self) -> Bookmark {
        Bookmark {
            title: self.title,
            description: self.description,
            url: self.url,
        }
    }
}

impl Bookmark {
    pub fn title(&self) -> String {
        self.title.clone()
    }

    pub fn set_title(&mut self, title: &str) {
        self.title = String::from(title);
    }

    pub fn description(&self) -> Option<String> {
        self.description.as_ref().cloned()
    }

    pub fn set_description(&mut self, description: &str) {
        self.description = Some(String::from(description));
    }

    pub fn url(&self) -> String {
        self.url.clone()
    }

    pub fn set_url(&mut self, url: &str) {
        self.url = String::from(url);
    }
}

//todo!(bookmarks): Add from_gmi() method for migrations
impl Bookmarks {
    pub async fn from_file(&self, path: &Path) -> anyhow::Result<Self> {
        let file_str = async_fs::read_to_string(path)
            .await
            .context("Reading bookmarks file")?;

        let bookmarks = toml::from_str(&file_str)?;

        Ok(bookmarks)
    }

    pub async fn to_file(&self, path: &Path) -> anyhow::Result<()> {
        let toml = toml::to_string(self)?;

        async_fs::write(path, toml)
            .await
            .context("Writting data to bookmarks file")?;

        Ok(())
    }

    //todo!(bookmarks): key must be the biggest current key + 1
    pub fn insert_bookmark(&mut self, bookmark: Bookmark) {
        self.bookmarks.insert(1.to_string(), bookmark);
    }

    pub fn update_bookmark(&mut self, key: u32, new_bookmark: Bookmark) {
        self.bookmarks.insert(key.to_string(), new_bookmark);
    }

    pub fn remove_bookmark(&mut self, key: u32) {
        if self.bookmarks.is_empty() {
            return;
        }

        self.bookmarks.remove(&key.to_string());
    }
}
