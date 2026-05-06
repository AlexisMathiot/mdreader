use std::sync::OnceLock;

use pulldown_cmark::{CodeBlockKind, Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use syntect::easy::HighlightLines;
use syntect::highlighting::{Theme, ThemeSet};
use syntect::parsing::{SyntaxReference, SyntaxSet};
use syntect::util::LinesWithEndings;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

const MAX_COL_WIDTH: usize = 40;
const MIN_COL_WIDTH: usize = 6;
const TEXT_COLOR: Color = Color::Rgb(230, 215, 184);
const CODE_BG: Color = Color::Rgb(40, 40, 40);

fn syntax_set() -> &'static SyntaxSet {
    static SS: OnceLock<SyntaxSet> = OnceLock::new();
    SS.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn theme() -> &'static Theme {
    static TH: OnceLock<Theme> = OnceLock::new();
    TH.get_or_init(|| {
        let ts = ThemeSet::load_defaults();
        ts.themes["base16-ocean.dark"].clone()
    })
}

fn find_syntax<'a>(ss: &'a SyntaxSet, lang: &str) -> &'a SyntaxReference {
    ss.find_syntax_by_token(lang)
        .or_else(|| ss.find_syntax_by_name(lang))
        .unwrap_or_else(|| ss.find_syntax_plain_text())
}

pub fn render(markdown: &str, max_width: usize) -> Vec<Line<'static>> {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, opts);
    let mut r = Renderer::new(max_width);
    for event in parser {
        r.handle(event);
    }
    r.flush_pending();

    while matches!(r.lines.last(), Some(line) if line.spans.is_empty()) {
        r.lines.pop();
    }

    r.lines
}

struct Renderer {
    lines: Vec<Line<'static>>,
    current: Vec<Span<'static>>,
    style: Style,
    style_stack: Vec<Style>,
    list_stack: Vec<Option<u64>>,
    bq_depth: usize,
    in_code_block: bool,
    code_buffer: String,
    code_lang: Option<String>,
    link_url: Option<String>,
    table: Option<TableBuilder>,
    saved_spans: Option<Vec<Span<'static>>>,
    max_width: usize,
}

struct TableBuilder {
    headers: Vec<String>,
    rows: Vec<Vec<String>>,
    current_cells: Vec<String>,
    in_header: bool,
}

impl Renderer {
    fn new(max_width: usize) -> Self {
        Self {
            lines: Vec::new(),
            current: Vec::new(),
            style: Style::default().fg(TEXT_COLOR),
            style_stack: Vec::new(),
            list_stack: Vec::new(),
            bq_depth: 0,
            in_code_block: false,
            code_buffer: String::new(),
            code_lang: None,
            link_url: None,
            table: None,
            saved_spans: None,
            max_width: max_width.max(20),
        }
    }

    fn push_style(&mut self, patch: Style) {
        self.style_stack.push(self.style);
        self.style = self.style.patch(patch);
    }

    fn pop_style(&mut self) {
        if let Some(s) = self.style_stack.pop() {
            self.style = s;
        }
    }

    fn text(&mut self, s: &str) {
        let mut first = true;
        for piece in s.split('\n') {
            if !first {
                self.finish_line();
            }
            first = false;
            if !piece.is_empty() {
                self.current
                    .push(Span::styled(piece.to_string(), self.style));
            }
        }
    }

    fn line_prefix(&self) -> Vec<Span<'static>> {
        (0..self.bq_depth)
            .map(|_| Span::styled("│ ".to_string(), Style::default().fg(TEXT_COLOR)))
            .collect()
    }

    fn finish_line(&mut self) {
        let mut spans = self.line_prefix();
        spans.append(&mut self.current);
        self.lines.push(Line::from(spans));
    }

    fn blank_line(&mut self) {
        if !self.current.is_empty() {
            self.finish_line();
        }
        if !matches!(self.lines.last(), Some(line) if line.spans.is_empty()) {
            self.lines.push(Line::from(""));
        }
    }

    fn flush_pending(&mut self) {
        if !self.current.is_empty() {
            self.finish_line();
        }
    }

    fn emit_code_block(&mut self, code: &str, lang: Option<&str>) {
        let ss = syntax_set();
        let theme = theme();
        let syntax = lang
            .map(|l| find_syntax(ss, l))
            .unwrap_or_else(|| ss.find_syntax_plain_text());
        let mut hl = HighlightLines::new(syntax, theme);

        for raw_line in LinesWithEndings::from(code) {
            let ranges = hl.highlight_line(raw_line, ss).unwrap_or_default();
            let mut spans = self.line_prefix();
            for (style, text) in ranges {
                let piece = text.trim_end_matches('\n');
                if piece.is_empty() {
                    continue;
                }
                let fg = Color::Rgb(style.foreground.r, style.foreground.g, style.foreground.b);
                spans.push(Span::styled(
                    piece.to_string(),
                    Style::default().fg(fg).bg(CODE_BG),
                ));
            }
            self.lines.push(Line::from(spans));
        }
    }

    fn handle(&mut self, ev: Event<'_>) {
        match ev {
            Event::Start(tag) => self.start(tag),
            Event::End(tag) => self.end(tag),
            Event::Text(t) => {
                if self.in_code_block {
                    self.code_buffer.push_str(&t);
                } else {
                    self.text(&t);
                }
            }
            Event::Code(t) => {
                self.push_style(Style::default().fg(Color::Cyan).bg(Color::Rgb(40, 40, 40)));
                self.text(&t);
                self.pop_style();
            }
            Event::SoftBreak => self.text(" "),
            Event::HardBreak => {
                if self.table.is_some() {
                    self.text(" ");
                } else {
                    self.finish_line();
                }
            }
            Event::Rule => {
                if !self.current.is_empty() {
                    self.finish_line();
                }
                self.lines.push(Line::from(Span::styled(
                    "─ ─ ─ ─ ─ ─ ─ ─ ─ ─".to_string(),
                    Style::default().fg(Color::DarkGray),
                )));
                self.blank_line();
            }
            Event::TaskListMarker(done) => {
                self.text(if done { "[x] " } else { "[ ] " });
            }
            _ => {}
        }
    }

    fn start(&mut self, tag: Tag<'_>) {
        match tag {
            Tag::Paragraph => {}
            Tag::Heading { level, .. } => {
                let (color, prefix) = heading_style(level);
                self.push_style(Style::default().fg(color).add_modifier(Modifier::BOLD));
                self.text(prefix);
            }
            Tag::BlockQuote(_) => {
                self.bq_depth += 1;
            }
            Tag::CodeBlock(kind) => {
                self.in_code_block = true;
                self.code_buffer.clear();
                self.code_lang = match kind {
                    CodeBlockKind::Fenced(lang) if !lang.is_empty() => Some(lang.to_string()),
                    _ => None,
                };
            }
            Tag::List(start) => {
                self.list_stack.push(start);
            }
            Tag::Item => {
                let indent = "  ".repeat(self.list_stack.len().saturating_sub(1));
                let marker = match self.list_stack.last_mut() {
                    Some(Some(n)) => {
                        let m = format!("{n}. ");
                        *self.list_stack.last_mut().unwrap() = Some(*n + 1);
                        m
                    }
                    _ => "• ".to_string(),
                };
                self.text(&indent);
                let saved = self.style;
                self.style = self.style.patch(Style::default().fg(Color::DarkGray));
                self.text(&marker);
                self.style = saved;
            }
            Tag::Emphasis => {
                self.push_style(Style::default().add_modifier(Modifier::ITALIC));
            }
            Tag::Strong => {
                self.push_style(Style::default().add_modifier(Modifier::BOLD));
            }
            Tag::Strikethrough => {
                self.push_style(Style::default().add_modifier(Modifier::CROSSED_OUT));
            }
            Tag::Link { dest_url, .. } => {
                self.link_url = Some(dest_url.to_string());
                self.push_style(
                    Style::default()
                        .fg(Color::Blue)
                        .add_modifier(Modifier::UNDERLINED),
                );
            }
            Tag::Image { .. } => {
                self.push_style(Style::default().fg(Color::Magenta));
                self.text("[image: ");
            }
            Tag::Table(_) => {
                if !self.current.is_empty() {
                    self.finish_line();
                }
                self.table = Some(TableBuilder {
                    headers: Vec::new(),
                    rows: Vec::new(),
                    current_cells: Vec::new(),
                    in_header: false,
                });
            }
            Tag::TableHead => {
                if let Some(tb) = &mut self.table {
                    tb.in_header = true;
                    tb.current_cells.clear();
                }
            }
            Tag::TableRow => {
                if let Some(tb) = &mut self.table {
                    tb.current_cells.clear();
                }
            }
            Tag::TableCell => {
                self.saved_spans = Some(std::mem::take(&mut self.current));
            }
            _ => {}
        }
    }

    fn end(&mut self, tag: TagEnd) {
        match tag {
            TagEnd::Paragraph => {
                self.finish_line();
                self.blank_line();
            }
            TagEnd::Heading(_) => {
                self.pop_style();
                self.finish_line();
                self.blank_line();
            }
            TagEnd::BlockQuote(_) => {
                self.bq_depth = self.bq_depth.saturating_sub(1);
            }
            TagEnd::CodeBlock => {
                self.in_code_block = false;
                if !self.current.is_empty() {
                    self.finish_line();
                }
                let code = std::mem::take(&mut self.code_buffer);
                let lang = self.code_lang.take();
                self.emit_code_block(&code, lang.as_deref());
                self.blank_line();
            }
            TagEnd::List(_) => {
                self.list_stack.pop();
                if self.list_stack.is_empty() {
                    self.blank_line();
                }
            }
            TagEnd::Item => {
                self.finish_line();
            }
            TagEnd::Emphasis | TagEnd::Strong | TagEnd::Strikethrough => {
                self.pop_style();
            }
            TagEnd::Link => {
                self.pop_style();
                if let Some(url) = self.link_url.take() {
                    let saved = self.style;
                    self.style = self.style.patch(Style::default().fg(Color::DarkGray));
                    self.text(&format!(" ({url})"));
                    self.style = saved;
                }
            }
            TagEnd::Image => {
                self.text("]");
                self.pop_style();
            }
            TagEnd::Table => {
                if let Some(tb) = self.table.take() {
                    for line in layout_table(&tb, self.max_width) {
                        self.lines.push(line);
                    }
                    self.blank_line();
                }
            }
            TagEnd::TableHead => {
                if let Some(tb) = &mut self.table {
                    let cells = std::mem::take(&mut tb.current_cells);
                    tb.headers = cells;
                    tb.in_header = false;
                }
            }
            TagEnd::TableRow => {
                if let Some(tb) = &mut self.table {
                    let cells = std::mem::take(&mut tb.current_cells);
                    tb.rows.push(cells);
                }
            }
            TagEnd::TableCell => {
                let cell_spans = std::mem::take(&mut self.current);
                if let Some(saved) = self.saved_spans.take() {
                    self.current = saved;
                }
                let text: String = cell_spans.iter().map(|s| s.content.as_ref()).collect();
                if let Some(tb) = &mut self.table {
                    tb.current_cells.push(text);
                }
            }
            _ => {}
        }
    }
}

fn heading_style(level: HeadingLevel) -> (Color, &'static str) {
    match level {
        HeadingLevel::H1 => (Color::Red, "═══ "),
        HeadingLevel::H2 => (Color::Yellow, "── "),
        HeadingLevel::H3 => (Color::Cyan, "▸ "),
        HeadingLevel::H4 => (Color::Green, "▹ "),
        HeadingLevel::H5 => (Color::Magenta, "• "),
        HeadingLevel::H6 => (Color::Gray, "· "),
    }
}

fn layout_table(tb: &TableBuilder, max_width: usize) -> Vec<Line<'static>> {
    let n_cols = tb
        .headers
        .len()
        .max(tb.rows.iter().map(|r| r.len()).max().unwrap_or(0));
    if n_cols == 0 {
        return Vec::new();
    }

    let widths = fit_widths(compute_widths(tb, n_cols), max_width);
    let border_style = Style::default().add_modifier(Modifier::BOLD);
    let mut out: Vec<Line<'static>> = Vec::new();

    let make_border = |left: char, mid: char, right: char| -> Line<'static> {
        let mut s = String::new();
        s.push(left);
        for (i, &w) in widths.iter().enumerate() {
            if i > 0 {
                s.push(mid);
            }
            for _ in 0..(w + 2) {
                s.push('─');
            }
        }
        s.push(right);
        Line::from(Span::styled(s, border_style))
    };

    out.push(make_border('┌', '┬', '┐'));

    if !tb.headers.is_empty() {
        emit_row(&mut out, &tb.headers, &widths, true);
        out.push(make_border('├', '┼', '┤'));
    }

    for row in &tb.rows {
        emit_row(&mut out, row, &widths, false);
    }

    out.push(make_border('└', '┴', '┘'));

    out
}

fn compute_widths(tb: &TableBuilder, n_cols: usize) -> Vec<usize> {
    let mut max_widths = vec![0usize; n_cols];

    let all_rows = std::iter::once(&tb.headers).chain(tb.rows.iter());
    for row in all_rows {
        for (c, cell) in row.iter().enumerate() {
            if c < n_cols {
                let w = UnicodeWidthStr::width(cell.as_str()).min(MAX_COL_WIDTH);
                max_widths[c] = max_widths[c].max(w);
            }
        }
    }

    max_widths.iter_mut().for_each(|w| *w = (*w).max(1));
    max_widths
}

fn fit_widths(mut widths: Vec<usize>, max_width: usize) -> Vec<usize> {
    let n = widths.len();
    if n == 0 {
        return widths;
    }
    let overhead = 3 * n + 1;
    let available = max_width.saturating_sub(overhead);
    let current_sum: usize = widths.iter().sum();

    if current_sum > available {
        while widths.iter().sum::<usize>() > available {
            let max_w = *widths.iter().max().unwrap_or(&0);
            if max_w <= MIN_COL_WIDTH {
                break;
            }
            if let Some(idx) = widths.iter().position(|&w| w == max_w) {
                widths[idx] -= 1;
            } else {
                break;
            }
        }
    } else if current_sum < available && current_sum > 0 {
        let extra = available - current_sum;
        let additions: Vec<usize> = widths.iter().map(|&w| (w * extra) / current_sum).collect();
        for (w, add) in widths.iter_mut().zip(additions.iter()) {
            *w += add;
        }

        let mut sorted_idx: Vec<usize> = (0..widths.len()).collect();
        sorted_idx.sort_by_key(|&i| std::cmp::Reverse(widths[i]));
        let mut remainder = available.saturating_sub(widths.iter().sum::<usize>());
        for &idx in sorted_idx.iter().cycle() {
            if remainder == 0 {
                break;
            }
            widths[idx] += 1;
            remainder -= 1;
        }
    }

    widths
}

fn emit_row(out: &mut Vec<Line<'static>>, cells: &[String], widths: &[usize], is_header: bool) {
    let wrapped: Vec<Vec<String>> = widths
        .iter()
        .enumerate()
        .map(|(c, &w)| {
            let text = cells.get(c).map(String::as_str).unwrap_or("");
            wrap_cjk(text, w)
        })
        .collect();

    let height = wrapped.iter().map(Vec::len).max().unwrap_or(1).max(1);

    let cell_style = if is_header {
        Style::default()
            .fg(Color::White)
            .add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(TEXT_COLOR)
    };
    let sep_style = Style::default().add_modifier(Modifier::BOLD);

    for row_line in 0..height {
        let mut spans: Vec<Span<'static>> = Vec::new();
        for (c, &w) in widths.iter().enumerate() {
            let content = wrapped[c].get(row_line).cloned().unwrap_or_default();
            let padded = pad_to_width(&content, w);
            if c == 0 {
                spans.push(Span::styled("│ ".to_string(), sep_style));
            } else {
                spans.push(Span::styled(" │ ".to_string(), sep_style));
            }
            spans.push(Span::styled(padded, cell_style));
        }
        spans.push(Span::styled(" │".to_string(), sep_style));
        out.push(Line::from(spans));
    }
}

fn pad_to_width(s: &str, w: usize) -> String {
    let cur = UnicodeWidthStr::width(s);
    if cur >= w {
        s.to_string()
    } else {
        let mut out = String::with_capacity(s.len() + (w - cur));
        out.push_str(s);
        for _ in 0..(w - cur) {
            out.push(' ');
        }
        out
    }
}

fn wrap_cjk(text: &str, width: usize) -> Vec<String> {
    if width == 0 {
        return vec![text.to_string()];
    }
    if text.is_empty() {
        return vec![String::new()];
    }

    let mut lines: Vec<String> = Vec::new();
    let mut current = String::new();
    let mut current_w = 0usize;

    let mut chars = text.chars().peekable();
    while let Some(&ch) = chars.peek() {
        let cw = UnicodeWidthChar::width(ch).unwrap_or(0);

        if ch == ' ' {
            chars.next();
            if current_w > 0 && current_w < width {
                current.push(' ');
                current_w += 1;
            }
        } else if cw > 1 {
            chars.next();
            if current_w + cw > width && !current.is_empty() {
                trim_trailing_space(&mut current, &mut current_w);
                lines.push(std::mem::take(&mut current));
                current_w = 0;
            }
            current.push(ch);
            current_w += cw;
        } else {
            let mut word = String::new();
            let mut word_w = 0usize;
            while let Some(&ch2) = chars.peek() {
                if ch2 == ' ' {
                    break;
                }
                let cw2 = UnicodeWidthChar::width(ch2).unwrap_or(1);
                if cw2 > 1 {
                    break;
                }
                chars.next();
                word.push(ch2);
                word_w += cw2;
            }

            if word_w == 0 {
                continue;
            }

            if current_w + word_w <= width {
                current.push_str(&word);
                current_w += word_w;
            } else if word_w <= width {
                trim_trailing_space(&mut current, &mut current_w);
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                }
                current.push_str(&word);
                current_w = word_w;
            } else {
                trim_trailing_space(&mut current, &mut current_w);
                if !current.is_empty() {
                    lines.push(std::mem::take(&mut current));
                    current_w = 0;
                }
                for ch2 in word.chars() {
                    if current_w >= width {
                        lines.push(std::mem::take(&mut current));
                        current_w = 0;
                    }
                    current.push(ch2);
                    current_w += 1;
                }
            }
        }
    }

    trim_trailing_space(&mut current, &mut current_w);
    if !current.is_empty() || lines.is_empty() {
        lines.push(current);
    }
    lines
}

fn trim_trailing_space(s: &mut String, w: &mut usize) {
    while s.ends_with(' ') {
        s.pop();
        *w = w.saturating_sub(1);
    }
}
