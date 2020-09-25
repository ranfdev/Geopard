use serde::Deserialize;

pub static EXAMPLE: &str = r#"colors = true
custom_css = """
textview {
    font-family: Cantarell;
    font-size: 1.1em;
    font-weight: 400;
}
"""
"#;

#[derive(Deserialize, Debug)]
pub struct Config {
    pub custom_css: String,
    pub colors: bool
}
