mod app;
mod events;
mod parser;
mod ui;

use anyhow::Result;
use clap::Parser;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyModifiers},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};
use std::io;
use std::time::Duration;

use app::App;

#[derive(Parser)]
#[command(
    name = "hprof-tui",
    about = "Terminal UI for Java/Android HPROF heap dump analysis"
)]
struct Cli {
    /// Path to the .hprof file to analyze
    hprof_file: String,
}

fn main() -> Result<()> {
    let cli = Cli::parse();

    // Headless dump mode for CI / non-TTY environments
    if std::env::var("HPROF_DUMP").is_ok() {
        let analysis = app::App::new(&cli.hprof_file)?.analysis;
        let s = &analysis.summary;
        println!("=== hprof-tui parse results ===");
        println!("Version   : {}", s.hprof_version);
        println!("File size : {}", parser::fmt_bytes(s.file_size_bytes));
        println!("Heap size : {}", parser::fmt_bytes(s.total_heap_size));
        println!("Instances : {}", s.total_instances);
        println!("Classes   : {}", s.total_classes);
        println!("Arrays    : {}", s.total_arrays);
        println!("GC roots  : {}", s.total_gc_roots);
        println!("Histogram : {} entries", analysis.class_histogram.len());
        println!("Suspects  : {}", analysis.leak_suspects.len());
        println!("Dup str   : {}", analysis.duplicate_strings.len());
        println!("\nTop 5 classes:");
        for (i, c) in analysis.class_histogram.iter().take(5).enumerate() {
            println!(
                "  {:2}. {:<50} {:>8}  x{}",
                i + 1,
                c.class_name,
                parser::fmt_bytes(c.shallow_size),
                c.instance_count
            );
        }
        if !analysis.leak_suspects.is_empty() {
            println!("\nLeak suspects:");
            for ls in analysis.leak_suspects.iter().take(5) {
                println!(
                    "  [{}] {:.1}%  {}",
                    ls.severity.label(),
                    ls.heap_percentage,
                    ls.class_name
                );
            }
        }
        return Ok(());
    }

    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Create app and load hprof
    let mut app = App::new(&cli.hprof_file)?;

    // Run the main loop
    let res = run_app(&mut terminal, &mut app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: ratatui::backend::Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.code == KeyCode::Char('q')
                    || (key.code == KeyCode::Char('c')
                        && key.modifiers.contains(KeyModifiers::CONTROL))
                {
                    return Ok(());
                }
                events::handle_key(app, key);
            }
        }
    }
}
