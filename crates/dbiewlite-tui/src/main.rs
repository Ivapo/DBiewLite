mod app;
mod input;
mod ui;

use std::io;
use std::time::Duration;

use crossterm::event::{self, DisableMouseCapture, EnableMouseCapture, Event};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

fn print_help() {
    println!(
        "dbiew {version}
A friendly viewer for SQLite, DuckDB, and Parquet files.

USAGE:
    dbiew <FILE>

ARGS:
    <FILE>    Database to open (.sqlite, .db, .sqlite3, .duckdb, .parquet, .pq)

OPTIONS:
    -h, -H, --help       Print this help
    -V, -v, --version    Print version

EXAMPLE:
    dbiew mydata.db

Press ? inside the app for keyboard shortcuts.",
        version = env!("CARGO_PKG_VERSION"),
    );
}

/// Resolves the file to open, handling `--help`/`--version` first. Every branch
/// that isn't a path exits the process, so this must run before raw mode — a
/// `println!` on the alternate screen would be wiped on exit.
fn parse_args() -> String {
    let Some(arg) = std::env::args().nth(1) else {
        eprintln!("dbiew: missing database file");
        eprintln!("Try 'dbiew --help' for usage.");
        std::process::exit(1);
    };

    match arg.as_str() {
        "-h" | "-H" | "--help" => {
            print_help();
            std::process::exit(0);
        }
        "-V" | "-v" | "--version" => {
            println!("dbiew {}", env!("CARGO_PKG_VERSION"));
            std::process::exit(0);
        }
        other if other.starts_with('-') => {
            eprintln!("dbiew: unrecognized argument '{other}'");
            eprintln!("Try 'dbiew --help' for usage.");
            std::process::exit(2);
        }
        other => other.to_string(),
    }
}

/// Put the terminal back the way we found it. Safe to call more than once.
fn restore_terminal() {
    let _ = disable_raw_mode();
    let _ = execute!(io::stdout(), DisableMouseCapture, LeaveAlternateScreen);
    let _ = execute!(io::stdout(), crossterm::cursor::Show);
}

/// Without this, a panic unwinds straight past the cleanup at the end of
/// `main`, leaving the user on the alternate screen in raw mode — the shell
/// looks dead and the panic message is never seen.
fn install_panic_hook() {
    let default_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        restore_terminal();
        default_hook(info);
    }));
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let db_path = parse_args();
    install_panic_hook();

    // Setup terminal. Mouse capture is what makes wheel and horizontal
    // trackpad scrolling reach us instead of the terminal emulator.
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let mut app = match app::App::new(&db_path) {
        Ok(a) => a,
        Err(e) => {
            restore_terminal();
            eprintln!("Failed to open database: {}", e);
            std::process::exit(1);
        }
    };

    // Event loop — render on demand only. Idle costs no CPU.
    let status_ttl = Duration::from_secs(3);
    let idle_timeout = Duration::from_secs(60);
    terminal.draw(|f| ui::draw(f, &mut app))?;

    loop {
        // Wake either for the next event or when the status message expires.
        let timeout = match app.status_message_at {
            Some(at) => status_ttl.saturating_sub(at.elapsed()),
            None => idle_timeout,
        };

        let mut dirty = false;

        if event::poll(timeout)? {
            // Drain every pending event before redrawing so bursts (trackpad
            // scrolling, key repeat) coalesce into a single frame.
            loop {
                match event::read()? {
                    Event::Key(key) => {
                        input::handle_key(&mut app, key);
                        dirty = true;
                    }
                    Event::Mouse(mouse) => {
                        if input::handle_mouse(&mut app, mouse) {
                            dirty = true;
                        }
                    }
                    Event::Resize(_, _) => dirty = true,
                    _ => {}
                }
                if app.should_quit || !event::poll(Duration::ZERO)? {
                    break;
                }
            }
        }

        // Deferred until the burst is drained, so holding `j` in the sidebar
        // queries the database once instead of once per key repeat.
        if app.flush_pending_load() {
            dirty = true;
        }

        // Auto-clear status message after 3 seconds
        if let Some(at) = app.status_message_at
            && at.elapsed() >= status_ttl
        {
            app.status_message = None;
            app.status_message_at = None;
            dirty = true;
        }

        if app.should_quit {
            break;
        }

        if dirty {
            terminal.draw(|f| ui::draw(f, &mut app))?;
        }
    }

    restore_terminal();
    Ok(())
}
