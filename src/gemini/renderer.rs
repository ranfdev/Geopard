use crate::common::Ctx;
use crate::gemini::Token;
use crate::TextRender;
use gtk::prelude::*;

pub struct Renderer {
    ctx: Ctx,
}

impl Renderer {
    pub fn new(ctx: Ctx) -> Self {
        Self {
            ctx
        }
    }
}

impl<'a> TextRender<Token<'a>> for Renderer {
    fn render(&mut self, token: Token<'a>) {
        let mut text_iter = self.ctx.text_buffer.get_end_iter();
        dbg!(&token);

        match token {
            Token::Text(line) => {
                self.ctx.insert_paragraph(&mut text_iter, &line);
            }
            Token::Heading(line) => {
                self.ctx.insert_heading(&mut text_iter, &line);
            }
            Token::Quote(line) => {
                self.ctx.insert_quote(&mut text_iter, &line);
            }
            Token::Preformatted(line) => {
                self.ctx.insert_preformatted(&mut text_iter, &line);
            }
            Token::Empty => {
                self.ctx.insert_paragraph(&mut text_iter, "\n");
            }
            Token::Link(url, label) => {
                self.ctx.insert_internal_link(&mut text_iter, url, label);
            }
        }
    }
}
