use serde::Deserialize;

pub static EXAMPLE: &str = r#"colors = true
# This is equal to the font css property of a webpage
fonts.normal = "1em \"sans-serif\""
"#;

#[derive(Deserialize, Debug)]
pub struct Fonts {
    pub normal: String,
    // maybe in the future people will be able to set
    // different normal, italic and bold fonts
}

#[derive(Deserialize, Debug)]
pub struct Config {
    pub fonts: Fonts,
    pub colors: bool
}
