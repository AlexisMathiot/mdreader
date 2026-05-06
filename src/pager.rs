use std::fs;
use std::path::PathBuf;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::prelude::*;
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap};

use crate::render;

pub struct Pager {
    content: String,
    text: Text<'static>,
    last_width: u16,
    path: PathBuf,
    scroll: u16,
    viewport_height: u16,
    show_help: bool,
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
            show_help: false,
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
        if self.show_help {
            match code {
                KeyCode::Char('?') | KeyCode::Esc => self.show_help = false,
                KeyCode::Char('q') => self.should_quit = true,
                _ => {}
            }
            return;
        }
        match code {
            KeyCode::Char('q') => self.should_quit = true,
            KeyCode::Char('?') => self.show_help = true,
            KeyCode::Char('j') | KeyCode::Down => {
                self.scroll = self.scroll.saturating_add(1);
            }
            KeyCode::Char('k') | KeyCode::Up => {
                self.scroll = self.scroll.saturating_sub(1);
            }
            KeyCode::Char('g') => self.scroll = 0,
            KeyCode::Char('G') => self.scroll = u16::MAX,
            KeyCode::PageDown | KeyCode::Char(' ') | KeyCode::Char('f') => {
                self.scroll = self
                    .scroll
                    .saturating_add(self.viewport_height.saturating_sub(2));
            }
            KeyCode::PageUp | KeyCode::Char('b') => {
                self.scroll = self
                    .scroll
                    .saturating_sub(self.viewport_height.saturating_sub(2));
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
        let [content_area, status_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());

        let h_pad = (content_area.width / 20).max(2);
        let title = format!(" {} ", self.path.display());
        let block = Block::default()
            .borders(Borders::ALL)
            .padding(Padding::horizontal(h_pad))
            .title(title);
        let inner = block.inner(content_area);
        self.viewport_height = inner.height;

        self.ensure_rendered(inner.width);

        let paragraph = Paragraph::new(self.text.clone()).wrap(Wrap { trim: false });
        let total_lines = paragraph.line_count(inner.width) as u16;
        let max_scroll = total_lines.saturating_sub(inner.height);
        self.scroll = self.scroll.min(max_scroll);

        frame.render_widget(
            paragraph.block(block).scroll((self.scroll, 0)),
            content_area,
        );

        let pct = if max_scroll == 0 {
            100
        } else {
            (self.scroll as u32 * 100 / max_scroll as u32) as u16
        };
        let status = format!(
            " {}  {pct}%   ?: help   q: quit ",
            self.path.display()
        );
        frame.render_widget(
            Paragraph::new(status).style(Style::default().add_modifier(Modifier::REVERSED)),
            status_area,
        );

        if self.show_help {
            self.draw_help(frame);
        }
    }

    fn draw_help(&self, frame: &mut Frame) {
        let lines = vec![
            Line::from(" j / ↓        line down"),
            Line::from(" k / ↑        line up"),
            Line::from(" d            half page down"),
            Line::from(" u            half page up"),
            Line::from(" f / space    page down"),
            Line::from(" b            page up"),
            Line::from(" g            top"),
            Line::from(" G            bottom"),
            Line::from(" r            reload"),
            Line::from(" ?            toggle help"),
            Line::from(" q / Ctrl+C   quit"),
        ];
        let popup_w = 36u16;
        let popup_h = lines.len() as u16 + 2;
        let area = frame.area();
        let w = popup_w.min(area.width);
        let h = popup_h.min(area.height);
        let popup = Rect {
            x: area.x + (area.width - w) / 2,
            y: area.y + (area.height - h) / 2,
            width: w,
            height: h,
        };
        let block = Block::default().borders(Borders::ALL).title(" help ");
        frame.render_widget(Clear, popup);
        frame.render_widget(Paragraph::new(lines).block(block), popup);
    }

    fn reload(&mut self) -> Result<()> {
        self.last_width = 0;
        self.content = fs::read_to_string(&self.path)
            .with_context(|| format!("lecture de {}", self.path.display()))?;
        Ok(())
    }
}
