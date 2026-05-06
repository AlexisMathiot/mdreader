use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::prelude::*;
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Padding, Paragraph, Wrap};

use crate::render;

pub struct Pager {
    content: String,
    text: Text<'static>,
    last_width: u16,
    path: PathBuf,
    scroll: u16,
    viewport_height: u16,
    pub should_quit: bool,
}

impl Pager {
    pub fn new(path: PathBuf) -> Result<Self> {
        let mut pager = Self {
            content: String::new(),
            text: Text::default(),
            last_width: 0,
            path,
            scroll: 0,
            viewport_height: 0,
            should_quit: false,
        };
        pager.reload()?;
        Ok(pager)
    }

    fn ensure_rendered(&mut self, width: u16) {
        if width != self.last_width {
            let lines = render::render(&self.content, width as usize);
            self.text = Text::from(lines);
            self.last_width = width;
        }
    }

    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll = self.scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            KeyCode::Char('g') => self.scroll = 0,
            KeyCode::Char('G') => self.scroll = u16::MAX,
            KeyCode::PageDown | KeyCode::Char(' ') | KeyCode::Char('f') => {
                self.scroll = self.scroll.saturating_add(self.viewport_height);
            }
            KeyCode::PageUp | KeyCode::Char('b') => {
                self.scroll = self.scroll.saturating_sub(self.viewport_height);
            }
            KeyCode::Char('d') => {
                self.scroll = self.scroll.saturating_add(self.viewport_height / 2);
            }
            KeyCode::Char('u') => {
                self.scroll = self.scroll.saturating_sub(self.viewport_height / 2);
            }
            KeyCode::Char('r') => {
                let _ = self.reload();
            }
            _ => {}
        }
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let area = frame.area();
        let h_pad = (area.width / 20).max(2);
        let title = format!(" {} ", self.path.display());
        let block = Block::default()
            .borders(Borders::ALL)
            .padding(Padding::horizontal(h_pad))
            .title(title);
        let inner = block.inner(area);
        self.viewport_height = inner.height;

        self.ensure_rendered(inner.width);

        let paragraph = Paragraph::new(self.text.clone()).wrap(Wrap { trim: false });
        let total_lines = paragraph.line_count(inner.width) as u16;
        let max_scroll = total_lines.saturating_sub(inner.height);
        self.scroll = self.scroll.min(max_scroll);

        frame.render_widget(paragraph.block(block).scroll((self.scroll, 0)), area);
    }

    fn reload(&mut self) -> Result<()> {
        self.last_width = 0;
        self.content = fs::read_to_string(&self.path)
            .with_context(|| format!("lecture de {}", self.path.display()))?;
        Ok(())
    }
}
