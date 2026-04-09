mod app;
mod screens;
mod ui;

use std::io;
use std::path::PathBuf;
use std::time::Duration;

use app::App;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, prelude::CrosstermBackend};

#[derive(Parser)]
#[command(name = "sickgnal-tui", about = "Sickgnal TUI client")]
struct Args {
    /// Directory for account storage
    #[arg(long, default_value = "./storage")]
    data_dir: PathBuf,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let result = run_app(&mut terminal, args.data_dir);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = result {
        eprintln!("Error: {err}");
    }

    Ok(())
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    data_dir: PathBuf,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new(data_dir);

    loop {
        terminal.draw(|f| ui::draw(f, &mut app))?;

        // Poll for events with a short timeout so we can also check SDK events
        if crossterm::event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                // Global quit: Ctrl+C or Ctrl+Q
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    && (key.code == KeyCode::Char('c') || key.code == KeyCode::Char('q'))
                {
                    return Ok(());
                }

                app.handle_key(key);
            }
        }

        // Process any pending SDK events
        app.poll_sdk_events();

        if app.should_quit {
            return Ok(());
        }
    }
}
