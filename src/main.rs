mod app;
mod events;
mod parser;
mod retained;
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
        let heap_nz = s.total_heap_size.max(1);
        println!("\nTop 30 classes by shallow size:");
        println!(
            "  {:>3}  {:<55} {:>10}  {:>10}  {:>7}",
            "#", "Class", "ShallowSz", "Instances", "% Heap"
        );
        println!(
            "  {}  {}  {}  {}  {}",
            "-".repeat(3),
            "-".repeat(55),
            "-".repeat(10),
            "-".repeat(10),
            "-".repeat(7)
        );
        for (i, c) in analysis.class_histogram.iter().take(30).enumerate() {
            let pct = c.shallow_size as f64 / heap_nz as f64 * 100.0;
            println!(
                "  {:>3}  {:<55} {:>10}  {:>10}  {:>6.2}%",
                i + 1,
                if c.class_name.len() > 55 {
                    format!("…{}", &c.class_name[c.class_name.len() - 54..])
                } else {
                    c.class_name.clone()
                },
                parser::fmt_bytes(c.shallow_size),
                c.instance_count,
                pct
            );
        }
        // Lower threshold: show all classes >1% of heap
        let suspects: Vec<_> = analysis
            .class_histogram
            .iter()
            .filter(|c| (c.shallow_size as f64 / heap_nz as f64 * 100.0) >= 1.0)
            .collect();
        println!("\nClasses using >1% of heap ({} found):", suspects.len());
        for c in &suspects {
            let pct = c.shallow_size as f64 / heap_nz as f64 * 100.0;
            let sev = if pct >= 30.0 {
                "HIGH"
            } else if pct >= 15.0 {
                "MED "
            } else if pct >= 5.0 {
                "LOW "
            } else {
                "    "
            };
            println!(
                "  [{}] {:>6.2}%  {:>10}  x{:<8}  {}",
                sev,
                pct,
                parser::fmt_bytes(c.shallow_size),
                c.instance_count,
                c.class_name
            );
        }
        // Oracle/vendor specific classes
        let vendor: Vec<_> = analysis
            .class_histogram
            .iter()
            .filter(|c| {
                c.class_name.contains("oracle")
                    || c.class_name.contains("jdbc")
                    || c.class_name.contains("hibernate")
                    || c.class_name.contains("spring")
                    || c.class_name.contains("tomcat")
                    || c.class_name.contains("netty")
                    || c.class_name.contains("apache")
            })
            .take(15)
            .collect();
        if !vendor.is_empty() {
            println!("\nTop vendor/framework classes:");
            for c in &vendor {
                let pct = c.shallow_size as f64 / heap_nz as f64 * 100.0;
                println!(
                    "  {:>6.2}%  {:>10}  x{:<8}  {}",
                    pct,
                    parser::fmt_bytes(c.shallow_size),
                    c.instance_count,
                    c.class_name
                );
            }
        }
        // Also run retained analysis in dump mode
        if std::env::var("HPROF_RETAINED").is_ok() {
            eprintln!("Computing retained sizes (this may take a while)...");
            match crate::retained::compute_retained(&cli.hprof_file) {
                Ok(ra) => {
                    println!(
                        "\n=== Retained Size Analysis ({} classes{}) ===",
                        ra.entries.len(),
                        if ra.truncated { ", TRUNCATED" } else { "" }
                    );
                    println!(
                        "{:>3}  {:<52} {:>11}  {:>11}  {:>7}  {:>8}",
                        "#", "Class", "Retained", "Shallow", "×Over", "%Heap"
                    );
                    println!(
                        "{}  {}  {}  {}  {}  {}",
                        "-".repeat(3),
                        "-".repeat(52),
                        "-".repeat(11),
                        "-".repeat(11),
                        "-".repeat(5),
                        "-".repeat(8)
                    );
                    let heap = ra
                        .entries
                        .iter()
                        .map(|e| e.shallow_size)
                        .sum::<u64>()
                        .max(1);
                    for (i, e) in ra.entries.iter().take(25).enumerate() {
                        let pct = e.retained_size as f64 / heap as f64 * 100.0;
                        let name = if e.class_name.len() > 52 {
                            format!("…{}", &e.class_name[e.class_name.len() - 51..])
                        } else {
                            e.class_name.clone()
                        };
                        println!(
                            "{:>3}  {:<52} {:>11}  {:>11}  {:>5.1}×  {:>7.2}%",
                            i + 1,
                            name,
                            parser::fmt_bytes(e.retained_size),
                            parser::fmt_bytes(e.shallow_size),
                            e.overhead_ratio,
                            pct
                        );
                        if !e.top_contributors.is_empty() {
                            for c in e.top_contributors.iter().take(3) {
                                let c_pct = c.total_bytes as f64 / e.retained_size as f64 * 100.0;
                                let cname = if c.class_name.len() > 48 {
                                    format!("…{}", &c.class_name[c.class_name.len() - 47..])
                                } else {
                                    c.class_name.clone()
                                };
                                println!(
                                    "         ↳  {:<48} {:>11}  {:.1}%  ×{}",
                                    cname,
                                    parser::fmt_bytes(c.total_bytes),
                                    c_pct,
                                    c.object_count
                                );
                            }
                        }
                    }
                }
                Err(e) => eprintln!("Retained analysis error: {}", e),
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
