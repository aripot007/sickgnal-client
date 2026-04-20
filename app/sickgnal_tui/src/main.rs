mod app;
mod screens;
mod ui;

use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use app::App;
use clap::{Parser, ValueEnum};
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::{Terminal, prelude::CrosstermBackend};
use sickgnal_sdk::TlsConfig;
use tracing_subscriber::EnvFilter;

/// TLS implementation to use for the server connection.
#[derive(Debug, Clone, Copy, ValueEnum)]
enum TlsMode {
    /// Production TLS via rustls (recommended)
    Rustls,
    /// Custom (experimental) TLS implementation
    Insecure,
    /// No TLS — plain TCP (development only)
    None,
}

#[derive(Parser)]
#[command(name = "sickgnal-tui", about = "Sickgnal TUI client")]
struct Args {
    /// Directory for account storage
    #[arg(long, default_value = "./storage")]
    data_dir: PathBuf,

    /// Enable tracing and log to the specified file
    #[arg(long)]
    log: Option<PathBuf>,

    /// Server address (host:port)
    #[arg(long, default_value = "sickgnal.bapttf.com:443")]
    server: String,

    /// TLS implementation to use
    #[arg(long, value_enum, default_value_t = TlsMode::Rustls)]
    tls: TlsMode,

    /// Path to a PEM-encoded CA certificate to trust (for self-signed servers)
    #[arg(long)]
    tls_ca: Option<PathBuf>,
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    // Build TlsConfig from CLI args
    let tls_config = match args.tls {
        TlsMode::Rustls => TlsConfig::Rustls {
            custom_ca: args.tls_ca,
        },
        TlsMode::Insecure => TlsConfig::Insecure {
            custom_ca: args.tls_ca,
        },
        TlsMode::None => TlsConfig::None,
    };

    // Setup tracing if --log is provided
    let _guard = if let Some(log_path) = &args.log {
        let dir = log_path.parent().unwrap_or(Path::new("."));
        let filename = log_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .to_string();
        let file_appender = tracing_appender::rolling::never(dir, filename);
        let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
        tracing_subscriber::fmt()
            .with_env_filter(EnvFilter::from_default_env())
            .with_writer(non_blocking)
            .with_ansi(false)
            .init();
        Some(guard)
    } else {
        None
    };
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let result = run_app(&mut terminal, args.data_dir, args.server, tls_config);

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
    server_addr: String,
    tls_config: TlsConfig,
) -> Result<(), Box<dyn std::error::Error>> {
    let mut app = App::new(data_dir, server_addr, tls_config);

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

        // Check if async auth connection completed
        app.poll_auth_completion();

        if app.should_quit {
            return Ok(());
        }
    }
}
