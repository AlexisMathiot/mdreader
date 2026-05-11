mod pager;
mod render;

use std::env;
use std::io::{self, IsTerminal};
use std::path::PathBuf;

use anyhow::{Result, bail};
use crossterm::ExecutableCommand;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use ratatui::prelude::*;

use pager::Pager;

fn main() -> Result<()> {
    let mut pager = match env::args().nth(1) {
        Some(arg) => Pager::from_path(PathBuf::from(arg))?,
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
