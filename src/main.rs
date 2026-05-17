mod config;
mod pager;
mod render;
mod theme;

use std::io::{self, IsTerminal};
use std::path::PathBuf;
use std::sync::mpsc;
use std::thread;

use anyhow::{Result, bail};
use clap::Parser;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyEvent, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use notify::{EventKind, RecursiveMode, Watcher};
use ratatui::prelude::*;

use pager::Pager;

enum AppEvent {
    Key(KeyEvent),
    FileChanged,
}

#[derive(Parser)]
#[command(name = "mdreader", about = "terminal markdown reader")]
struct Cli {
    /// markdown file to display (reads stdin if omitted and stdin is piped)
    file: Option<PathBuf>,

    /// theme preset (dark, dracula, tokyo-night, light)
    #[arg(long)]
    theme: Option<String>,

    /// max render width in columns
    #[arg(long, short = 'w')]
    width: Option<u16>,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let cfg = config::Config::load()?;

    let theme_name = cli
        .theme
        .or(cfg.theme)
        .unwrap_or_else(|| theme::DEFAULT.into());
    theme::set(&theme_name)?;

    let max_width = cli.width.or(cfg.width);

    let mut pager = match cli.file {
        Some(path) => Pager::from_path(path, max_width)?,
        None => {
            if io::stdin().is_terminal() {
                bail!("usage: mdreader <fichier.md>  (or pipe markdown on stdin)");
            }
            let content = io::read_to_string(io::stdin())?;
            Pager::from_stdin(content, max_width)
        }
    };

    let mut stdout = io::stdout();
    enable_raw_mode()?;
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = run(&mut terminal, &mut pager);

    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    pager: &mut Pager,
) -> Result<()> {
    let (tx, rx) = mpsc::channel::<AppEvent>();

    let key_tx = tx.clone();
    thread::spawn(move || {
        loop {
            match event::read() {
                Ok(Event::Key(key)) => {
                    if key_tx.send(AppEvent::Key(key)).is_err() {
                        break;
                    }
                }
                Ok(_) => {}
                Err(_) => break,
            }
        }
    });

    let _watcher = pager.watch_path().map(|path| {
        let watch_tx = tx.clone();
        let mut watcher = notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
            if let Ok(event) = res
                && !matches!(event.kind, EventKind::Access(_))
            {
                let _ = watch_tx.send(AppEvent::FileChanged);
            }
        })?;
        watcher.watch(path, RecursiveMode::NonRecursive)?;
        Ok::<_, notify::Error>(watcher)
    }).transpose()?;

    drop(tx);

    while !pager.should_quit {
        terminal.draw(|frame| pager.draw(frame))?;
        match rx.recv() {
            Ok(AppEvent::Key(key)) if key.kind == KeyEventKind::Press => {
                pager.on_key(key.code, key.modifiers);
            }
            Ok(AppEvent::FileChanged) => {
                pager.reload()?;
            }
            Ok(_) => {}
            Err(_) => break,
        }
    }
    Ok(())
}
