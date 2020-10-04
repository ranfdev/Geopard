use once_cell::sync::Lazy;
use regex::Regex;
static R_GEMINI_LINK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^=>\s*(?P<href>\S*)\s*(?P<label>.*)").unwrap());

#[derive(Debug)]
pub enum Token<'a> {
    Heading(&'a str),
    Quote(&'a str),
    Preformatted(&'a str),
    Text(&'a str),
    Link(&'a str, Option<&'a str>),
    Empty,
}

pub struct Parser {
    inside_pre: bool,
}

impl Parser {
    pub fn new() -> Self {
        Self { inside_pre: false }
    }
    pub fn parse_line<'a>(&mut self, line: &'a str) -> Token<'a> {
        if line.starts_with("```") {
            self.inside_pre = !self.inside_pre;
            Token::Empty
        } else if self.inside_pre {
            Token::Preformatted(line)
        } else if line.starts_with("#") {
            Token::Heading(line)
        } else if line.starts_with(">") {
            Token::Quote(line)
        } else if let Some(captures) = R_GEMINI_LINK.captures(&line) {
            match (captures.name("href"), captures.name("label")) {
                (Some(m_href), Some(m_label)) if !m_label.as_str().is_empty() => {
                    Token::Link(m_href.as_str(), Some(m_label.as_str()))
                }
                (Some(m_href), _) => Token::Link(m_href.as_str(), None),
                _ => Token::Empty,
            }
        } else {
            Token::Text(line)
        }
    }
}
