use serde::{Serialize, Deserialize};
use once_cell::sync::Lazy;

pub const DEFAULT_CONFIG: Lazy<Config> = Lazy::new(|| Config {
    colors: true,
    fonts: Fonts {
        paragraph: Some(Fonts::default_paragraph()),
        preformatted: Some(Fonts::default_preformatted()),
        heading: Some(Fonts::default_heading()),
        quote: Some(Fonts::default_quote())
    }
});

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Font {
    pub family: String,
    pub size: i32,
    pub weight: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Fonts {
    pub paragraph: Option<Font>,
    pub preformatted: Option<Font>,
    pub heading: Option<Font>,
    pub quote: Option<Font>,
}

impl Fonts {
    pub fn default_heading() -> Font {
        Font {
            family: String::from("sans-serif"),
            size: 18,
            weight: 800,
        }
    }

    pub fn default_preformatted() -> Font {
        Font {
            family: String::from("monospace"),
            size: 13,
            weight: 500,
        }
    }

    pub fn default_quote() -> Font {
        Font {
            family: String::from("sans-serif"),
            size: 13,
            weight: 500,
        }
    }

    pub fn default_paragraph() -> Font {
        Font {
            family: String::from("sans-serif"),
            size: 13,
            weight: 500,
        }
    }
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct Config {
    pub colors: bool,
    pub fonts: Fonts,
}
