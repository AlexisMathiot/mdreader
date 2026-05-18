use std::path::{Path, PathBuf};
use std::time::SystemTime;

use anyhow::{Context, Result};
use crossterm::event::{KeyCode, KeyModifiers};
use nucleo_matcher::pattern::{CaseMatching, Normalization, Pattern};
use nucleo_matcher::{Config, Matcher, Utf32Str};
use ratatui::prelude::*;
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, Borders, Paragraph};
use walkdir::WalkDir;

use crate::theme;

struct Entry {
    path: PathBuf,
    display: String,
    mtime: SystemTime,
}

pub struct Stash {
    root: PathBuf,
    entries: Vec<Entry>,
    filter: String,
    filtered: Vec<usize>,
    selected: usize,
    scroll: usize,
    filtering: bool,
    viewport_height: u16,
    matcher: Matcher,
    open_request: Option<PathBuf>,
    pub should_quit: bool,
}

impl Stash {
    pub fn scan(root: impl AsRef<Path>) -> Result<Self> {
        let root = root.as_ref().to_path_buf();
        let mut entries = Vec::new();
        let walker = WalkDir::new(&root).follow_links(false).into_iter();
        for entry in walker
            .filter_entry(|e| {
                if e.depth() == 0 {
                    return true;
                }
                let name = e.file_name().to_string_lossy();
                if name.starts_with('.') {
                    return false;
                }
                if e.file_type().is_dir() && (name == "target" || name == "node_modules") {
                    return false;
                }
                true
            })
            .filter_map(Result::ok)
        {
            if !entry.file_type().is_file() {
                continue;
            }
            let path = entry.path();
            let is_md = path
                .extension()
                .is_some_and(|e| e == "md" || e == "markdown");
            if !is_md {
                continue;
            }
            let mtime = entry
                .metadata()
                .ok()
                .and_then(|m| m.modified().ok())
                .unwrap_or(SystemTime::UNIX_EPOCH);
            let display = path
                .strip_prefix(&root)
                .unwrap_or(path)
                .to_string_lossy()
                .into_owned();
            entries.push(Entry {
                path: path.to_path_buf(),
                display,
                mtime,
            });
        }
        entries.sort_by(|a, b| b.mtime.cmp(&a.mtime));
        let filtered = (0..entries.len()).collect();
        Ok(Self {
            root: root
                .canonicalize()
                .with_context(|| format!("canonicalize {}", root.display()))
                .unwrap_or(root),
            entries,
            filter: String::new(),
            filtered,
            selected: 0,
            scroll: 0,
            filtering: false,
            viewport_height: 0,
            matcher: Matcher::new(Config::DEFAULT),
            open_request: None,
            should_quit: false,
        })
    }

    pub fn take_open_request(&mut self) -> Option<PathBuf> {
        self.open_request.take()
    }

    pub fn on_key(&mut self, code: KeyCode, mods: KeyModifiers) {
        if mods.contains(KeyModifiers::CONTROL) && code == KeyCode::Char('c') {
            self.should_quit = true;
            return;
        }
        if self.filtering {
            match code {
                KeyCode::Esc => {
                    self.filtering = false;
                    self.filter.clear();
                    self.recompute_filter();
                }
                KeyCode::Enter => {
                    self.filtering = false;
                    self.open_selected();
                }
                KeyCode::Backspace => {
                    self.filter.pop();
                    self.recompute_filter();
                }
                KeyCode::Up => self.move_selection(-1),
                KeyCode::Down => self.move_selection(1),
                KeyCode::Char(c) => {
                    self.filter.push(c);
                    self.recompute_filter();
                }
                _ => {}
            }
            return;
        }
        match code {
            KeyCode::Char('q') | KeyCode::Esc => self.should_quit = true,
            KeyCode::Char('j') | KeyCode::Down => self.move_selection(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_selection(-1),
            KeyCode::Char('g') => self.selected = 0,
            KeyCode::Char('G') => self.selected = self.filtered.len().saturating_sub(1),
            KeyCode::PageDown => {
                self.move_selection(self.viewport_height.saturating_sub(2) as i32)
            }
            KeyCode::PageUp => {
                self.move_selection(-(self.viewport_height.saturating_sub(2) as i32))
            }
            KeyCode::Char('/') => self.filtering = true,
            KeyCode::Enter => self.open_selected(),
            _ => {}
        }
    }

    fn open_selected(&mut self) {
        if let Some(&i) = self.filtered.get(self.selected) {
            self.open_request = Some(self.entries[i].path.clone());
        }
    }

    fn move_selection(&mut self, delta: i32) {
        if self.filtered.is_empty() {
            return;
        }
        let max = self.filtered.len() as i32 - 1;
        self.selected = (self.selected as i32 + delta).clamp(0, max) as usize;
    }

    fn recompute_filter(&mut self) {
        if self.filter.is_empty() {
            self.filtered = (0..self.entries.len()).collect();
            self.selected = 0;
            return;
        }
        let pat = Pattern::parse(&self.filter, CaseMatching::Smart, Normalization::Smart);
        let mut buf = Vec::new();
        let mut scored: Vec<(usize, u32)> = Vec::new();
        for (i, e) in self.entries.iter().enumerate() {
            let h = Utf32Str::new(&e.display, &mut buf);
            if let Some(s) = pat.score(h, &mut self.matcher) {
                scored.push((i, s));
            }
        }
        scored.sort_by(|a, b| b.1.cmp(&a.1));
        self.filtered = scored.into_iter().map(|(i, _)| i).collect();
        self.selected = 0;
    }

    pub fn draw(&mut self, frame: &mut Frame) {
        let [header, list_area, status_area] = Layout::vertical([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(frame.area());

        let bg_style = Style::default().bg(theme::current().bg);
        let title = format!(" {} ", self.root.display());
        frame.render_widget(
            Paragraph::new(title).style(Style::default().add_modifier(Modifier::BOLD)),
            header,
        );

        let block = Block::default().borders(Borders::ALL).style(bg_style);
        let inner = block.inner(list_area);
        self.viewport_height = inner.height;
        let h = inner.height as usize;

        if h > 0 {
            if self.selected < self.scroll {
                self.scroll = self.selected;
            } else if self.selected >= self.scroll + h {
                self.scroll = self.selected + 1 - h;
            }
        }

        let lines: Vec<Line> = self
            .filtered
            .iter()
            .enumerate()
            .skip(self.scroll)
            .take(h)
            .map(|(idx, &i)| {
                let e = &self.entries[i];
                let style = if idx == self.selected {
                    Style::default().add_modifier(Modifier::REVERSED)
                } else {
                    Style::default()
                };
                Line::from(Span::styled(format!(" {} ", e.display), style))
            })
            .collect();

        frame.render_widget(
            Paragraph::new(lines).block(block).style(bg_style),
            list_area,
        );

        let pos = if self.filtered.is_empty() {
            0
        } else {
            self.selected + 1
        };
        let status = if self.filtering {
            format!(" /{}_   enter: open   esc: clear ", self.filter)
        } else if !self.filter.is_empty() {
            format!(
                " {pos}/{}   filter: {}   /: edit   enter: open   q: quit ",
                self.filtered.len(),
                self.filter
            )
        } else {
            format!(
                " {pos}/{}   /: filter   enter: open   q: quit ",
                self.filtered.len()
            )
        };
        frame.render_widget(
            Paragraph::new(status).style(Style::default().add_modifier(Modifier::REVERSED)),
            status_area,
        );
    }
}
