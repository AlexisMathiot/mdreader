use std::sync::OnceLock;

use anyhow::{Result, bail};
use ratatui::style::Color;

pub struct MdTheme {
    pub name: &'static str,
    pub bg: Color,
    pub text: Color,
    pub h1: Color,
    pub h2: Color,
    pub h3: Color,
    pub h4: Color,
    pub h5: Color,
    pub h6: Color,
    pub link: Color,
    pub link_url: Color,
    pub code_inline_fg: Color,
    pub code_inline_bg: Color,
    pub code_block_bg: Color,
    pub blockquote: Color,
    pub list_marker: Color,
    pub rule: Color,
    pub image: Color,
    pub table_border: Color,
    pub table_header: Color,
    pub syntect: &'static str,
}

pub const DARK: MdTheme = MdTheme {
    name: "dark",
    bg: Color::Reset,
    text: Color::Rgb(230, 215, 184),
    h1: Color::Red,
    h2: Color::Yellow,
    h3: Color::Cyan,
    h4: Color::Green,
    h5: Color::Magenta,
    h6: Color::Gray,
    link: Color::Blue,
    link_url: Color::DarkGray,
    code_inline_fg: Color::Cyan,
    code_inline_bg: Color::Rgb(40, 40, 40),
    code_block_bg: Color::Rgb(40, 40, 40),
    blockquote: Color::Rgb(230, 215, 184),
    list_marker: Color::DarkGray,
    rule: Color::DarkGray,
    image: Color::Magenta,
    table_border: Color::White,
    table_header: Color::White,
    syntect: "base16-ocean.dark",
};

pub const DRACULA: MdTheme = MdTheme {
    name: "dracula",
    bg: Color::Reset,
    text: Color::Rgb(248, 248, 242),
    h1: Color::Rgb(255, 85, 85),
    h2: Color::Rgb(255, 184, 108),
    h3: Color::Rgb(241, 250, 140),
    h4: Color::Rgb(80, 250, 123),
    h5: Color::Rgb(139, 233, 253),
    h6: Color::Rgb(189, 147, 249),
    link: Color::Rgb(139, 233, 253),
    link_url: Color::Rgb(98, 114, 164),
    code_inline_fg: Color::Rgb(80, 250, 123),
    code_inline_bg: Color::Rgb(68, 71, 90),
    code_block_bg: Color::Rgb(40, 42, 54),
    blockquote: Color::Rgb(241, 250, 140),
    list_marker: Color::Rgb(98, 114, 164),
    rule: Color::Rgb(98, 114, 164),
    image: Color::Rgb(255, 121, 198),
    table_border: Color::Rgb(189, 147, 249),
    table_header: Color::Rgb(255, 121, 198),
    syntect: "base16-mocha.dark",
};

pub const TOKYO_NIGHT: MdTheme = MdTheme {
    name: "tokyo-night",
    bg: Color::Reset,
    text: Color::Rgb(192, 202, 245),
    h1: Color::Rgb(247, 118, 142),
    h2: Color::Rgb(255, 158, 100),
    h3: Color::Rgb(224, 175, 104),
    h4: Color::Rgb(158, 206, 106),
    h5: Color::Rgb(125, 207, 255),
    h6: Color::Rgb(187, 154, 247),
    link: Color::Rgb(122, 162, 247),
    link_url: Color::Rgb(86, 95, 137),
    code_inline_fg: Color::Rgb(125, 207, 255),
    code_inline_bg: Color::Rgb(36, 40, 59),
    code_block_bg: Color::Rgb(26, 27, 38),
    blockquote: Color::Rgb(187, 154, 247),
    list_marker: Color::Rgb(86, 95, 137),
    rule: Color::Rgb(86, 95, 137),
    image: Color::Rgb(187, 154, 247),
    table_border: Color::Rgb(122, 162, 247),
    table_header: Color::Rgb(247, 118, 142),
    syntect: "base16-eighties.dark",
};

pub const LIGHT: MdTheme = MdTheme {
    name: "light",
    bg: Color::Rgb(250, 250, 250),
    text: Color::Rgb(40, 40, 40),
    h1: Color::Rgb(175, 0, 0),
    h2: Color::Rgb(175, 95, 0),
    h3: Color::Rgb(0, 95, 135),
    h4: Color::Rgb(0, 135, 0),
    h5: Color::Rgb(135, 0, 175),
    h6: Color::Rgb(88, 88, 88),
    link: Color::Rgb(0, 95, 175),
    link_url: Color::Rgb(118, 118, 118),
    code_inline_fg: Color::Rgb(175, 0, 0),
    code_inline_bg: Color::Rgb(238, 238, 238),
    code_block_bg: Color::Rgb(238, 238, 238),
    blockquote: Color::Rgb(118, 118, 118),
    list_marker: Color::Rgb(118, 118, 118),
    rule: Color::Rgb(208, 208, 208),
    image: Color::Rgb(175, 0, 175),
    table_border: Color::Rgb(88, 88, 88),
    table_header: Color::Rgb(0, 0, 0),
    syntect: "InspiredGitHub",
};

pub const PRESETS: &[&MdTheme] = &[&DARK, &DRACULA, &TOKYO_NIGHT, &LIGHT];
pub const DEFAULT: &str = "dark";

static CURRENT: OnceLock<&'static MdTheme> = OnceLock::new();

pub fn set(name: &str) -> Result<()> {
    let theme = lookup(name)?;
    let _ = CURRENT.set(theme);
    Ok(())
}

pub fn current() -> &'static MdTheme {
    CURRENT.get().copied().unwrap_or(&DARK)
}

fn lookup(name: &str) -> Result<&'static MdTheme> {
    for t in PRESETS {
        if t.name == name {
            return Ok(*t);
        }
    }
    let names: Vec<&str> = PRESETS.iter().map(|t| t.name).collect();
    bail!("unknown theme '{name}'. available: {}", names.join(", "));
}
