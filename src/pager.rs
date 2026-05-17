use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyModifiers};
use ratatui::prelude::*;
use ratatui::text::Text;
use ratatui::widgets::{Block, Borders, Clear, Padding, Paragraph, Wrap};

use crate::render;
use crate::theme;

pub enum Source {
    File(PathBuf),
    Stdin,
}

impl Source {
    fn display_name(&self) -> String {
        match self {
            Source::File(path) => path.display().to_string(),
            Source::Stdin => "stdin".into(),
        }
    }
}

pub struct Pager {
    content: String,
    text: Text<'static>,
    last_width: u16,
    source: Source,
    max_width: Option<u16>,
    scroll: u16,
    viewport_height: u16,
    show_help: bool,
    pub should_quit: bool,
}

impl Pager {
    pub fn from_path(path: PathBuf, max_width: Option<u16>) -> Result<Self> {
        let content = fs::read_to_string(&path)
            .with_context(|| format!("lecture de {}", path.display()))?;
        Ok(Self::new(Source::File(path), content, max_width))
    }

    pub fn from_stdin(content: String, max_width: Option<u16>) -> Self {
        Self::new(Source::Stdin, content, max_width)
    }

    fn new(source: Source, content: String, max_width: Option<u16>) -> Self {
        Self {
            content,
            text: Text::default(),
            last_width: 0,
            source,
            max_width,
            scroll: 0,
            viewport_height: 0,
            show_help: false,
            should_quit: false,
        }
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
        let [full_content, status_area] =
            Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(frame.area());

        let bg_style = Style::default().bg(theme::current().bg);

        let h_pad = if self.max_width.is_some() {
            2
        } else {
            (full_content.width / 20).max(2)
        };

        let content_area = match self.max_width {
            Some(max) => {
                let desired = max.saturating_add(2 + 2 * h_pad);
                let w = desired.min(full_content.width);
                Rect {
                    x: full_content.x + (full_content.width - w) / 2,
                    y: full_content.y,
                    width: w,
                    height: full_content.height,
                }
            }
            None => full_content,
        };

        let name = self.source.display_name();
        let title = format!(" {name} ");
        let block = Block::default()
            .borders(Borders::ALL)
            .padding(Padding::horizontal(h_pad))
            .title(title)
            .style(bg_style);
        let inner = block.inner(content_area);
        self.viewport_height = inner.height;

        self.ensure_rendered(inner.width);

        let paragraph = Paragraph::new(self.text.clone())
            .style(bg_style)
            .wrap(Wrap { trim: false });
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
        let status = format!(" {name}  {pct}%   ?: help   q: quit ");
        frame.render_widget(
            Paragraph::new(status).style(Style::default().add_modifier(Modifier::REVERSED)),
            status_area,
        );

        if self.show_help {
            self.draw_help(frame);
        }
    }

    fn draw_help(&self, frame: &mut Frame) {
        let mut lines = vec![
            Line::from(" j / ↓        line down"),
            Line::from(" k / ↑        line up"),
            Line::from(" d            half page down"),
            Line::from(" u            half page up"),
            Line::from(" f / space    page down"),
            Line::from(" b            page up"),
            Line::from(" g            top"),
            Line::from(" G            bottom"),
        ];
        if matches!(self.source, Source::File(_)) {
            lines.push(Line::from(" r            reload"));
        }
        lines.push(Line::from(" ?            toggle help"));
        lines.push(Line::from(" q / Ctrl+C   quit"));
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
        let bg_style = Style::default().bg(theme::current().bg);
        let block = Block::default()
            .borders(Borders::ALL)
            .title(" help ")
            .style(bg_style);
        frame.render_widget(Clear, popup);
        frame.render_widget(Paragraph::new(lines).style(bg_style).block(block), popup);
    }

    pub fn watch_path(&self) -> Option<&Path> {
        match &self.source {
            Source::File(path) => Some(path),
            Source::Stdin => None,
        }
    }

    pub fn reload(&mut self) -> Result<()> {
        let Source::File(path) = &self.source else {
            return Ok(());
        };
        self.content = fs::read_to_string(path)
            .with_context(|| format!("lecture de {}", path.display()))?;
        self.last_width = 0;
        Ok(())
    }
}
