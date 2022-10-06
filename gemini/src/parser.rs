use once_cell::sync::Lazy;
use regex::Regex;
static R_GEMINI_LINK: Lazy<Regex> =
    Lazy::new(|| Regex::new(r"^=>\s*(?P<href>\S+)(\s+(?P<label>.+))?").unwrap());

// See gemini://gemini.circumlunar.space/docs/cheatsheet.gmi

#[derive(Debug, Clone)]
pub enum Tag {
    Paragraph, // Is just a text line
    Heading(u8),
    BlockQuote,
    CodeBlock,
    UnorderedList,
    Item,
    Link(String, Option<String>),
}

#[derive(Debug, Clone)]
pub enum Event<'a> {
    Start(Tag),
    End,
    Text(&'a str),
    BlankLine,
}

#[derive(Debug, Clone, Default)]
pub struct Parser {
    tag_stack: Vec<Tag>,
}

impl Parser {
    pub fn new() -> Self {
        Self { tag_stack: vec![] }
    }

    /// Returns an `Event` when an event it's ready, else, `None`
    // TODO: Make this work on text input of any length, don't impose the "line" chunk requirement
    // some work has already been done, the pushed result is already structured to do so.
    pub fn parse_line<'a>(&mut self, line: &'a str, res: &mut Vec<Event<'a>>) {
        let parent_tag = self.tag_stack.last();

        // Close pending multi-line tags
        if matches!(parent_tag, Some(Tag::BlockQuote)) && !line.starts_with('>')
            || matches!(parent_tag, Some(Tag::UnorderedList))
        {
            res.push(Event::End);
            self.tag_stack.pop();
        }

        let parent_tag = self.tag_stack.last();

        if line.starts_with("```") {
            let inner_res = if let Some(Tag::CodeBlock) = parent_tag {
                self.tag_stack.pop();
                Event::End
            } else {
                self.tag_stack.push(Tag::CodeBlock);
                Event::Start(Tag::CodeBlock)
            };
            res.push(inner_res);
        } else if let Some(Tag::CodeBlock) = parent_tag {
            res.push(Event::Text(line));
        } else if line.trim().is_empty() {
            res.push(Event::BlankLine);
        } else if line.starts_with('#') {
            let line = line.trim_end();
            let lvl = line.chars().filter(|c| *c == '#').count();
            let heading = Tag::Heading(lvl as u8);
            res.push(Event::Start(heading));

            let text = line.trim_start_matches('#').trim_start();
            res.push(Event::Text(text));
            res.push(Event::End);
        } else if line.starts_with('>') {
            if !matches!(parent_tag, Some(Tag::BlockQuote)) {
                res.push(Event::Start(Tag::BlockQuote));
            }
            res.push(Event::Text(line.trim_start_matches('>')));
        } else if let Some(stripped) = line.strip_prefix("* ") {
            if !matches!(parent_tag, Some(Tag::UnorderedList)) {
                res.push(Event::Start(Tag::UnorderedList));
            }
            res.push(Event::Start(Tag::Item));
            res.push(Event::Text(stripped.trim_end()));
            res.push(Event::End);
        } else if let Some(captures) = R_GEMINI_LINK.captures(line.trim_end()) {
            let href = captures.name("href").unwrap();
            let label = captures.name("label").map(|x| x.as_str());
            res.push(Event::Start(Tag::Link(
                href.as_str().to_string(),
                label.map(|x| x.to_string()),
            )));
            res.push(Event::End);
        } else {
            res.push(Event::Start(Tag::Paragraph));
            res.push(Event::Text(line.trim_end()));
            res.push(Event::End);
        }
    }
}
