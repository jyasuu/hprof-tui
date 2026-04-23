mod app;
mod events;
mod ui;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::{io, time::Duration};

use app::App;

#[derive(Parser)]
#[command(
    name = "hprof-tui",
    about = "Terminal UI for Java/Android HPROF heap dump analysis\nPowered by the HeapLens Rust engine (two-phase CSR + Lengauer-Tarjan dominators)"
)]
struct Cli {
    /// Path to the .hprof file to analyse
    hprof_file: String,

    /// Skip Phase 2 (edges + dominator tree) — fast startup, no retained sizes
    #[arg(long, default_value_t = false)]
    phase1_only: bool,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Build analysis (may take 10-120s depending on heap size)
    eprintln!("Loading {}…", cli.hprof_file);
    let app = App::new(&cli.hprof_file, cli.phase1_only)?;
    eprintln!("Analysis complete. Opening TUI…");

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let result = run(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    result
}

fn run<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, &app))?;
        if event::poll(Duration::from_millis(50))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q')
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    return Ok(());
                }
                events::handle_key(&mut app, key);
            }
        }
    }
}
