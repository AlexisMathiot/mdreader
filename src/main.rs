mod pager;
mod render;
mod theme;

use std::io::{self, IsTerminal};
use std::path::PathBuf;

use anyhow::{Result, bail};
use clap::Parser;
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::prelude::*;

use pager::Pager;

#[derive(Parser)]
#[command(name = "mdreader", about = "terminal markdown reader")]
struct Cli {
    /// markdown file to display (reads stdin if omitted and stdin is piped)
    file: Option<PathBuf>,

    /// theme preset (dark, dracula, tokyo-night, light)
    #[arg(long, default_value = theme::DEFAULT)]
    theme: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    theme::set(&cli.theme)?;

    let mut pager = match cli.file {
        Some(path) => Pager::from_path(path)?,
        None => {
            if io::stdin().is_terminal() {
                bail!("usage: mdreader <fichier.md>  (or pipe markdown on stdin)");
            }
            let content = io::read_to_string(io::stdin())?;
            Pager::from_stdin(content)
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
    while !pager.should_quit {
        terminal.draw(|frame| pager.draw(frame))?;

        if let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            pager.on_key(key.code, key.modifiers);
        }
    }
    Ok(())
}
