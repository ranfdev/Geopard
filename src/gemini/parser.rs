use once_cell::sync::Lazy;
use regex::Regex;
static R_GEMINI_LINK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^=>\s*(?P<href>\S*)\s*(?P<label>.*)").unwrap());

#[derive(Debug)]
pub enum PageElement {
    Heading(String),
    Quote(String),
    Preformatted(String),
    Text(String),
    Link(String, Option<String>),
    Empty,
}

pub struct Parser {
    inside_pre: bool,
}

impl Parser {
    pub fn new() -> Self {
        Self { inside_pre: false }
    }
    pub fn parse_line(&mut self, line: &str) -> PageElement {
        if line.starts_with("```") {
            self.inside_pre = !self.inside_pre;
            PageElement::Empty
        } else if self.inside_pre {
            PageElement::Preformatted(line.to_string())
        } else if line.starts_with('#') {
            PageElement::Heading(line.to_string())
        } else if line.starts_with('>') {
            PageElement::Quote(line.to_string())
        } else if let Some(captures) = R_GEMINI_LINK.captures(line) {
            match (captures.name("href"), captures.name("label")) {
                (Some(m_href), Some(m_label)) if !m_label.as_str().is_empty() => PageElement::Link(
                    m_href.as_str().to_string(),
                    Some(m_label.as_str().to_string()),
                ),
                (Some(m_href), _) => PageElement::Link(m_href.as_str().to_string(), None),
                _ => PageElement::Empty,
            }
        } else {
            PageElement::Text(line.to_string())
        }
    }
}
