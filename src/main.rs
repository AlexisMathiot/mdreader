mod config;
mod pager;
mod remote;
mod render;
mod stash;
mod theme;

use std::fs;
use std::io::{self, IsTerminal, Write};
use std::path::Path;
use std::process::{Command, Stdio};
use std::sync::mpsc;
use std::time::Duration;

use anyhow::{Result, bail};
use clap::Parser;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use notify::{EventKind, RecursiveMode, Watcher};
use ratatui::prelude::*;

use pager::Pager;
use stash::Stash;

const POLL_TIMEOUT: Duration = Duration::from_millis(200);

#[derive(Parser)]
#[command(name = "mdreader", about = "terminal markdown reader")]
struct Cli {
    /// local path, https URL, or owner/repo (reads stdin if omitted and stdin is piped)
    target: Option<String>,

    /// theme preset (dark, dracula, tokyo-night, light)
    #[arg(long)]
    theme: Option<String>,

    /// max render width in columns
    #[arg(long, short = 'w')]
    width: Option<u16>,

    /// print rendered markdown to $PAGER (default `less -R`) instead of the TUI
    #[arg(long, short = 'p')]
    pager: bool,
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

    if cli.pager {
        return run_pager_mode(cli.target, max_width);
    }

    enum Start {
        Pager(Pager),
        Stash(Stash),
    }

    let start = match cli.target.as_deref().map(remote::parse) {
        Some(remote::Input::Local(path)) => Start::Pager(Pager::from_path(path, max_width)?),
        Some(input) => {
            let fetched = remote::fetch(&input)?;
            Start::Pager(Pager::from_remote(fetched, max_width))
        }
        None => {
            if io::stdin().is_terminal() {
                Start::Stash(Stash::scan(".")?)
            } else {
                let content = io::read_to_string(io::stdin())?;
                Start::Pager(Pager::from_stdin(content, max_width))
            }
        }
    };

    let mut stdout = io::stdout();
    enable_raw_mode()?;
    stdout.execute(EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let res = match start {
        Start::Pager(mut pager) => run_pager_loop(&mut terminal, &mut pager).map(|_| ()),
        Start::Stash(stash) => run_app(&mut terminal, stash, max_width),
    };

    disable_raw_mode()?;
    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    res
}

enum PagerExit {
    Quit,
    Back,
}

enum StashExit {
    Quit,
    Open(std::path::PathBuf),
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mut stash: Stash,
    max_width: Option<u16>,
) -> Result<()> {
    loop {
        let path = match run_stash_loop(terminal, &mut stash)? {
            StashExit::Quit => return Ok(()),
            StashExit::Open(p) => p,
        };
        let mut pager = Pager::from_path(path, max_width)?;
        pager.set_allow_back(true);
        match run_pager_loop(terminal, &mut pager)? {
            PagerExit::Quit => return Ok(()),
            PagerExit::Back => continue,
        }
    }
}

fn run_stash_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    stash: &mut Stash,
) -> Result<StashExit> {
    let mut dirty = true;
    loop {
        if stash.should_quit {
            return Ok(StashExit::Quit);
        }
        if let Some(path) = stash.take_open_request() {
            return Ok(StashExit::Open(path));
        }
        if dirty {
            terminal.draw(|frame| stash.draw(frame))?;
            dirty = false;
        }
        if event::poll(POLL_TIMEOUT)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            stash.on_key(key.code, key.modifiers);
            dirty = true;
        }
    }
}

fn run_pager_loop(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    pager: &mut Pager,
) -> Result<PagerExit> {
    let (tx, rx) = mpsc::channel::<()>();

    let _watcher = pager
        .watch_path()
        .map(|path| {
            let watch_tx = tx.clone();
            let mut watcher =
                notify::recommended_watcher(move |res: notify::Result<notify::Event>| {
                    if let Ok(event) = res
                        && !matches!(event.kind, EventKind::Access(_))
                    {
                        let _ = watch_tx.send(());
                    }
                })?;
            watcher.watch(path, RecursiveMode::NonRecursive)?;
            Ok::<_, notify::Error>(watcher)
        })
        .transpose()?;
    drop(tx);

    let mut dirty = true;
    loop {
        if pager.should_quit {
            return Ok(PagerExit::Quit);
        }
        if pager.take_back_request() {
            return Ok(PagerExit::Back);
        }

        if dirty {
            terminal.draw(|frame| pager.draw(frame))?;
            dirty = false;
        }

        if event::poll(POLL_TIMEOUT)?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            pager.on_key(key.code, key.modifiers);
            dirty = true;
        }

        let mut reload_needed = false;
        while rx.try_recv().is_ok() {
            reload_needed = true;
        }
        if reload_needed {
            pager.reload()?;
            dirty = true;
        }

        if let Some(path) = pager.take_edit_request() {
            match run_editor(terminal, &path) {
                Ok(()) => pager.reload()?,
                Err(e) => pager.set_status(format!("editor failed: {e}")),
            }
            dirty = true;
        }
    }
}

fn run_editor(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    path: &Path,
) -> io::Result<()> {
    let editor = std::env::var("VISUAL")
        .or_else(|_| std::env::var("EDITOR"))
        .map_err(|_| {
            io::Error::new(
                io::ErrorKind::NotFound,
                "set $EDITOR (e.g. 'export EDITOR=nvim')",
            )
        })?;

    terminal.backend_mut().execute(LeaveAlternateScreen)?;
    disable_raw_mode()?;

    let status = Command::new(&editor).arg(path).status();

    enable_raw_mode()?;
    terminal.backend_mut().execute(EnterAlternateScreen)?;
    terminal.clear()?;

    status?;
    Ok(())
}

fn run_pager_mode(target: Option<String>, max_width: Option<u16>) -> Result<()> {
    let content = match target.as_deref().map(remote::parse) {
        Some(remote::Input::Local(path)) => fs::read_to_string(&path)
            .map_err(|e| anyhow::anyhow!("lecture de {}: {e}", path.display()))?,
        Some(input) => remote::fetch(&input)?.content,
        None => {
            if io::stdin().is_terminal() {
                bail!("usage: mdreader -p <fichier.md|url|owner/repo>  (or pipe markdown on stdin)");
            }
            io::read_to_string(io::stdin())?
        }
    };

    let width = max_width
        .map(|w| w as usize)
        .unwrap_or_else(|| {
            crossterm::terminal::size()
                .map(|(w, _)| w as usize)
                .unwrap_or(100)
        });

    let lines = render::render(&content, width);
    let ansi = render::lines_to_ansi(&lines);

    let (cmd, args) = pager_command();
    let mut child = Command::new(&cmd)
        .args(&args)
        .stdin(Stdio::piped())
        .spawn()
        .map_err(|e| anyhow::anyhow!("spawn {cmd}: {e}"))?;
    child
        .stdin
        .take()
        .expect("stdin piped")
        .write_all(ansi.as_bytes())?;
    child.wait()?;
    Ok(())
}

fn pager_command() -> (String, Vec<String>) {
    if let Ok(p) = std::env::var("PAGER")
        && !p.trim().is_empty()
    {
        let mut parts = p.split_whitespace().map(String::from);
        if let Some(cmd) = parts.next() {
            return (cmd, parts.collect());
        }
    }
    ("less".into(), vec!["-R".into()])
}
