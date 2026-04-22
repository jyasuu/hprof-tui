use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table, Tabs, Wrap,
    },
    Frame,
};

use crate::app::{App, Tab};
use crate::parser::fmt_bytes;

// ── Colour palette ──────────────────────────────────────────────────────────
const C_BG: Color = Color::Rgb(15, 17, 26);
const C_PANEL: Color = Color::Rgb(22, 25, 37);
const C_BORDER: Color = Color::Rgb(55, 65, 100);
const C_ACCENT: Color = Color::Rgb(99, 155, 255);
const C_ACCENT2: Color = Color::Rgb(75, 210, 180);
const C_WARN: Color = Color::Rgb(255, 195, 80);
const C_DANGER: Color = Color::Rgb(255, 90, 90);
const C_LOW: Color = Color::Rgb(100, 220, 130);
const C_DIM: Color = Color::Rgb(90, 100, 130);
const C_TEXT: Color = Color::Rgb(210, 215, 235);
const C_SEL_BG: Color = Color::Rgb(35, 55, 90);
const C_HEADER: Color = Color::Rgb(140, 155, 200);

fn style() -> Style {
    Style::default().fg(C_TEXT).bg(C_BG)
}
fn accent() -> Style {
    Style::default().fg(C_ACCENT)
}
fn dim() -> Style {
    Style::default().fg(C_DIM)
}
fn header_style() -> Style {
    Style::default().fg(C_HEADER).add_modifier(Modifier::BOLD)
}
fn sel_style() -> Style {
    Style::default()
        .fg(Color::White)
        .bg(C_SEL_BG)
        .add_modifier(Modifier::BOLD)
}

// ── Entry point ──────────────────────────────────────────────────────────────
pub fn draw(f: &mut Frame, app: &App) {
    let area = f.size();

    // Root background
    f.render_widget(Block::default().style(style()), area);

    // Layout: title bar | tab bar | content | status bar
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // title
            Constraint::Length(3), // tabs
            Constraint::Min(0),    // content
            Constraint::Length(1), // status
        ])
        .split(area);

    draw_title(f, app, chunks[0]);
    draw_tabs(f, app, chunks[1]);
    draw_content(f, app, chunks[2]);
    draw_status(f, app, chunks[3]);
}

// ── Title bar ────────────────────────────────────────────────────────────────
fn draw_title(f: &mut Frame, app: &App, area: Rect) {
    use std::path::Path;
    let filename = Path::new(&app.analysis.summary.file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.hprof");

    let size_str = fmt_bytes(app.analysis.summary.file_size_bytes);
    let heap_str = fmt_bytes(app.analysis.summary.total_heap_size);
    let ver = &app.analysis.summary.hprof_version;

    let title_line = Line::from(vec![
        Span::styled(
            "  ⬡ hprof-tui ",
            Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", dim()),
        Span::styled(
            filename,
            Style::default().fg(C_ACCENT2).add_modifier(Modifier::BOLD),
        ),
        Span::styled(format!("  {}  on-disk", size_str), dim()),
        Span::styled("  │  ", dim()),
        Span::styled("heap ", dim()),
        Span::styled(heap_str, accent()),
        Span::styled("  │  ", dim()),
        Span::styled(ver, dim()),
    ]);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(C_BORDER))
        .style(Style::default().bg(C_PANEL));

    let para = Paragraph::new(title_line)
        .block(block)
        .alignment(Alignment::Left);

    f.render_widget(para, area);
}

// ── Tab bar ───────────────────────────────────────────────────────────────────
fn draw_tabs(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let shortcut = format!("{}", i + 1);
            if i < 5 {
                Line::from(vec![
                    Span::styled(format!("[{}]", shortcut), dim()),
                    Span::raw(" "),
                    Span::raw(t.title()),
                ])
            } else {
                Line::from(vec![Span::raw(t.title())])
            }
        })
        .collect();

    let tabs = Tabs::new(titles)
        .select(app.active_tab.index())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(C_BORDER))
                .style(Style::default().bg(C_PANEL)),
        )
        .highlight_style(Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD))
        .divider(Span::styled(" │ ", dim()))
        .style(dim());

    f.render_widget(tabs, area);
}

// ── Content dispatcher ───────────────────────────────────────────────────────
fn draw_content(f: &mut Frame, app: &App, area: Rect) {
    match app.active_tab {
        Tab::Overview => draw_overview(f, app, area),
        Tab::Histogram => draw_histogram(f, app, area),
        Tab::LeakSuspects => draw_leak_suspects(f, app, area),
        Tab::GcRoots => draw_gc_roots(f, app, area),
        Tab::DuplicateStrings => draw_dup_strings(f, app, area),
        Tab::Help => draw_help(f, area),
    }
}

// ── Status bar ───────────────────────────────────────────────────────────────
fn draw_status(f: &mut Frame, app: &App, area: Rect) {
    let msg = if let Some(ref s) = app.status_message {
        Line::from(vec![Span::styled(
            format!(" ✦ {}", s),
            Style::default().fg(C_WARN),
        )])
    } else {
        Line::from(vec![
            Span::styled(" q", Style::default().fg(C_ACCENT)),
            Span::styled("/Ctrl-C quit  ", dim()),
            Span::styled("Tab", Style::default().fg(C_ACCENT)),
            Span::styled("/←→ switch tab  ", dim()),
            Span::styled("↑↓/jk", Style::default().fg(C_ACCENT)),
            Span::styled(" scroll  ", dim()),
            Span::styled("s", Style::default().fg(C_ACCENT)),
            Span::styled(" toggle sort  ", dim()),
            Span::styled("g/G", Style::default().fg(C_ACCENT)),
            Span::styled(" top/bottom  ", dim()),
            Span::styled("?", Style::default().fg(C_ACCENT)),
            Span::styled(" help", dim()),
        ])
    };
    let para = Paragraph::new(msg).style(Style::default().bg(C_PANEL));
    f.render_widget(para, area);
}

// ────────────────────────────────────────────────────────────────────────────
// TAB 1: Overview
// ────────────────────────────────────────────────────────────────────────────
fn draw_overview(f: &mut Frame, app: &App, area: Rect) {
    let s = &app.analysis.summary;

    // Split into left (stats) and right (top classes chart)
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(42), Constraint::Percentage(58)])
        .split(area);

    // Left: split into stats + warnings
    let left_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(14), Constraint::Min(0)])
        .split(cols[0]);

    // ── Stats panel ──
    let stats_items: Vec<ListItem> = vec![
        listitem_kv("File", &s.file_path),
        listitem_kv("Version", &s.hprof_version),
        listitem_kv("File size", &fmt_bytes(s.file_size_bytes)),
        listitem_kv("Heap size", &fmt_bytes(s.total_heap_size)),
        listitem_kv("Instances", &fmt_u64(s.total_instances)),
        listitem_kv("Classes", &fmt_u64(s.total_classes)),
        listitem_kv("Arrays", &fmt_u64(s.total_arrays)),
        listitem_kv("GC roots", &fmt_u64(s.total_gc_roots)),
        listitem_kv(
            "Leak suspects",
            &app.analysis.leak_suspects.len().to_string(),
        ),
        listitem_kv(
            "Dup strings",
            &app.analysis.duplicate_strings.len().to_string(),
        ),
    ];

    let stats_list = List::new(stats_items).block(panel_block("Heap Summary"));
    f.render_widget(stats_list, left_rows[0]);

    // ── Leak severity summary ──
    let high = app
        .analysis
        .leak_suspects
        .iter()
        .filter(|l| l.severity == crate::parser::SuspectSeverity::High)
        .count();
    let med = app
        .analysis
        .leak_suspects
        .iter()
        .filter(|l| l.severity == crate::parser::SuspectSeverity::Medium)
        .count();
    let low = app
        .analysis
        .leak_suspects
        .iter()
        .filter(|l| l.severity == crate::parser::SuspectSeverity::Low)
        .count();

    let sev_items: Vec<ListItem> = vec![
        ListItem::new(Line::from(vec![
            Span::styled(
                "  ● HIGH  ",
                Style::default().fg(C_DANGER).add_modifier(Modifier::BOLD),
            ),
            Span::styled(high.to_string(), Style::default().fg(C_TEXT)),
            Span::styled(" class(es) retaining >30% of heap", dim()),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                "  ● MED   ",
                Style::default().fg(C_WARN).add_modifier(Modifier::BOLD),
            ),
            Span::styled(med.to_string(), Style::default().fg(C_TEXT)),
            Span::styled(" class(es) retaining 15-30% of heap", dim()),
        ])),
        ListItem::new(Line::from(vec![
            Span::styled(
                "  ● LOW   ",
                Style::default().fg(C_LOW).add_modifier(Modifier::BOLD),
            ),
            Span::styled(low.to_string(), Style::default().fg(C_TEXT)),
            Span::styled(" class(es) retaining 5-15% of heap", dim()),
        ])),
    ];

    let sev_list = List::new(sev_items).block(panel_block("Leak Suspects at a Glance"));
    f.render_widget(sev_list, left_rows[1]);

    // ── Right: Top 15 classes bar chart ──
    let right_rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0)])
        .split(cols[1]);

    let hist = &app.analysis.class_histogram;
    let max_size = hist.first().map(|e| e.shallow_size).unwrap_or(1).max(1);

    // Each bar occupies 2 lines: label row + gauge row
    let bar_area = right_rows[0];
    let inner_h = bar_area.height.saturating_sub(2) as usize; // subtract block border
    let max_entries = (inner_h / 2).min(15).min(hist.len());

    // Build rows as text lines
    let mut lines: Vec<Line> = Vec::new();
    for (i, entry) in hist.iter().take(max_entries).enumerate() {
        let pct = (entry.shallow_size as f64 / max_size as f64 * 100.0) as u16;
        let short_name = shorten_class(&entry.class_name, 36);
        let rank_color = if i == 0 {
            C_DANGER
        } else if i < 3 {
            C_WARN
        } else {
            C_ACCENT
        };

        // Label line
        lines.push(Line::from(vec![
            Span::styled(format!(" {:>3}. ", i + 1), Style::default().fg(rank_color)),
            Span::styled(format!("{:<36}", short_name), Style::default().fg(C_TEXT)),
            Span::styled(format!(" {:>8}", fmt_bytes(entry.shallow_size)), accent()),
            Span::styled(format!("  ×{}", fmt_u64(entry.instance_count)), dim()),
        ]));

        // Gauge line — draw a Unicode block bar
        let bar_width = (bar_area.width.saturating_sub(6) as usize).min(60);
        let filled = (pct as usize * bar_width / 100).min(bar_width);
        let bar: String = "█".repeat(filled) + &"░".repeat(bar_width - filled);
        lines.push(Line::from(vec![
            Span::raw("      "),
            Span::styled(bar, Style::default().fg(rank_color)),
            Span::styled(format!(" {:>3}%", pct), dim()),
        ]));
    }

    let para = Paragraph::new(lines)
        .block(panel_block("Top Classes by Shallow Size"))
        .wrap(Wrap { trim: false });
    f.render_widget(para, bar_area);
}

// ────────────────────────────────────────────────────────────────────────────
// TAB 2: Class Histogram
// ────────────────────────────────────────────────────────────────────────────
fn draw_histogram(f: &mut Frame, app: &App, area: Rect) {
    let hist = &app.analysis.class_histogram;
    let total_heap = app.analysis.summary.total_heap_size.max(1);

    let header_cells = ["#", "Class Name", "Instances", "Shallow Size", "% Heap"]
        .iter()
        .map(|h| Cell::from(*h).style(header_style()));
    let header = Row::new(header_cells)
        .height(1)
        .bottom_margin(0)
        .style(Style::default().bg(C_PANEL));

    let visible = area.height.saturating_sub(5) as usize;
    let scroll = app.histogram_scroll;

    let rows: Vec<Row> = hist
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(i, entry)| {
            let pct = entry.shallow_size as f64 / total_heap as f64 * 100.0;
            let pct_str = format!("{:.2}%", pct);
            let is_sel = i == app.histogram_selected;

            let row_style = if is_sel { sel_style() } else { style() };

            let rank_str = format!("{}", i + 1);
            let name_style = if pct > 15.0 {
                Style::default().fg(C_DANGER)
            } else if pct > 5.0 {
                Style::default().fg(C_WARN)
            } else {
                Style::default().fg(C_TEXT)
            };

            Row::new(vec![
                Cell::from(rank_str).style(dim()),
                Cell::from(entry.class_name.clone()).style(if is_sel {
                    sel_style()
                } else {
                    name_style
                }),
                Cell::from(fmt_u64(entry.instance_count)).style(row_style),
                Cell::from(fmt_bytes(entry.shallow_size)).style(row_style),
                Cell::from(pct_str).style(if is_sel {
                    sel_style()
                } else {
                    pct_color_style(pct)
                }),
            ])
            .height(1)
            .style(row_style)
        })
        .collect();

    let sort_label = if app.sort_by_count { "count" } else { "size" };
    let title = format!(
        "Class Histogram  ({} classes)  sorted by {}  [s] toggle",
        hist.len(),
        sort_label
    );

    let scroll_info = format!(" {}/{} ", app.histogram_selected + 1, hist.len());

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Min(35),
            Constraint::Length(12),
            Constraint::Length(13),
            Constraint::Length(8),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(C_BORDER))
            .style(Style::default().bg(C_PANEL))
            .title(Span::styled(format!(" {} ", title), accent()))
            .title_alignment(Alignment::Left)
            .title_bottom(Span::styled(scroll_info, dim())),
    )
    .column_spacing(1);

    f.render_widget(table, area);
}

// ────────────────────────────────────────────────────────────────────────────
// TAB 3: Leak Suspects
// ────────────────────────────────────────────────────────────────────────────
fn draw_leak_suspects(f: &mut Frame, app: &App, area: Rect) {
    let suspects = &app.analysis.leak_suspects;

    // Split: list on top, detail pane on bottom
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(7)])
        .split(area);

    let header_cells = [
        "Sev",
        "Class Name",
        "Instances",
        "Shallow Size",
        "% of Heap",
    ]
    .iter()
    .map(|h| Cell::from(*h).style(header_style()));
    let header = Row::new(header_cells)
        .height(1)
        .style(Style::default().bg(C_PANEL));

    let visible = chunks[0].height.saturating_sub(5) as usize;
    let scroll = app.leak_scroll;

    let rows: Vec<Row> = suspects
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(i, s)| {
            let is_sel = i == app.leak_selected;
            let sev_color = severity_color(&s.severity);
            let row_style = if is_sel { sel_style() } else { style() };

            Row::new(vec![
                Cell::from(s.severity.label())
                    .style(Style::default().fg(sev_color).add_modifier(Modifier::BOLD)),
                Cell::from(s.class_name.clone()).style(if is_sel {
                    sel_style()
                } else {
                    Style::default().fg(sev_color)
                }),
                Cell::from(fmt_u64(s.instance_count)).style(row_style),
                Cell::from(fmt_bytes(s.total_shallow_size)).style(row_style),
                Cell::from(format!("{:.1}%", s.heap_percentage)).style(row_style),
            ])
            .height(1)
            .style(row_style)
        })
        .collect();

    let no_suspects = suspects.is_empty();
    let table_title = if no_suspects {
        " ✔ No Leak Suspects Detected ".to_string()
    } else {
        format!(
            " ⚠ {} Leak Suspect(s) — classes retaining >5% of heap ",
            suspects.len()
        )
    };

    let scroll_info = if !suspects.is_empty() {
        format!(" {}/{} ", app.leak_selected + 1, suspects.len())
    } else {
        String::new()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Min(35),
            Constraint::Length(12),
            Constraint::Length(13),
            Constraint::Length(10),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(C_BORDER))
            .style(Style::default().bg(C_PANEL))
            .title(Span::styled(
                table_title,
                Style::default().fg(C_WARN).add_modifier(Modifier::BOLD),
            ))
            .title_alignment(Alignment::Left)
            .title_bottom(Span::styled(scroll_info, dim())),
    )
    .column_spacing(1);

    f.render_widget(table, chunks[0]);

    // ── Detail pane for selected suspect ──
    let detail_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(C_BORDER))
        .style(Style::default().bg(C_PANEL))
        .title(Span::styled(" Details ", accent()));

    if let Some(s) = suspects.get(app.leak_selected) {
        let sev_color = severity_color(&s.severity);
        let lines = vec![
            Line::from(vec![
                Span::styled("  Class:    ", dim()),
                Span::styled(
                    &s.class_name,
                    Style::default().fg(sev_color).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Severity: ", dim()),
                Span::styled(
                    s.severity.label(),
                    Style::default().fg(sev_color).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  ({:.1}% of heap)", s.heap_percentage), dim()),
            ]),
            Line::from(vec![
                Span::styled("  Retained: ", dim()),
                Span::styled(fmt_bytes(s.total_shallow_size), accent()),
                Span::styled(
                    format!("  across {} instance(s)", fmt_u64(s.instance_count)),
                    dim(),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Tip: ", dim()),
                Span::styled(leak_tip(&s.class_name), Style::default().fg(C_TEXT)),
            ]),
        ];
        let para = Paragraph::new(lines)
            .block(detail_block)
            .wrap(Wrap { trim: false });
        f.render_widget(para, chunks[1]);
    } else {
        let para = Paragraph::new("  No suspect selected.")
            .style(dim())
            .block(detail_block);
        f.render_widget(para, chunks[1]);
    }
}

// ────────────────────────────────────────────────────────────────────────────
// TAB 4: GC Roots
// ────────────────────────────────────────────────────────────────────────────
fn draw_gc_roots(f: &mut Frame, app: &App, area: Rect) {
    // Count by root type
    let roots = &app.analysis.gc_roots;
    let total = roots.len();

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(38), Constraint::Percentage(62)])
        .split(area);

    // ── Left: summary by type ──
    let mut type_counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for r in roots {
        *type_counts.entry(r.root_type.as_str()).or_insert(0) += 1;
    }
    let mut type_vec: Vec<_> = type_counts.iter().collect();
    type_vec.sort_by(|a, b| b.1.cmp(a.1));

    let summary_items: Vec<ListItem> = type_vec
        .iter()
        .map(|(name, count)| {
            let pct = **count as f64 / total.max(1) as f64 * 100.0;
            ListItem::new(Line::from(vec![
                Span::styled(format!("  {:<18}", name), Style::default().fg(C_TEXT)),
                Span::styled(format!("{:>6}", count), accent()),
                Span::styled(format!("  ({:.1}%)", pct), dim()),
            ]))
        })
        .collect();

    let summary_block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(C_BORDER))
        .style(Style::default().bg(C_PANEL))
        .title(Span::styled(
            format!(" GC Root Types ({} total) ", total),
            accent(),
        ));

    f.render_widget(List::new(summary_items).block(summary_block), cols[0]);

    // ── Right: scrollable list of roots ──
    let header_cells = ["#", "Object ID (hex)", "Root Type"]
        .iter()
        .map(|h| Cell::from(*h).style(header_style()));
    let header = Row::new(header_cells)
        .height(1)
        .style(Style::default().bg(C_PANEL));

    let visible = area.height.saturating_sub(5) as usize;
    let scroll = app.gc_scroll;

    let rows: Vec<Row> = roots
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(i, r)| {
            let is_sel = i == app.gc_selected;
            let row_style = if is_sel { sel_style() } else { style() };
            Row::new(vec![
                Cell::from(format!("{}", i + 1)).style(dim()),
                Cell::from(format!("0x{:016x}", r.object_id)).style(row_style),
                Cell::from(r.root_type.clone()).style(if is_sel {
                    sel_style()
                } else {
                    Style::default().fg(C_ACCENT2)
                }),
            ])
            .height(1)
            .style(row_style)
        })
        .collect();

    let scroll_info = if total > 0 {
        format!(" {}/{} ", app.gc_selected + 1, total)
    } else {
        String::new()
    };

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Length(20),
            Constraint::Min(20),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(C_BORDER))
            .style(Style::default().bg(C_PANEL))
            .title(Span::styled(" GC Root Objects ", accent()))
            .title_bottom(Span::styled(scroll_info, dim())),
    )
    .column_spacing(2);

    f.render_widget(table, cols[1]);
}

// ────────────────────────────────────────────────────────────────────────────
// TAB 5: Duplicate Strings
// ────────────────────────────────────────────────────────────────────────────
fn draw_dup_strings(f: &mut Frame, app: &App, area: Rect) {
    let dups = &app.analysis.duplicate_strings;

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    // ── Summary gauge ──
    let total_wasted: u64 = dups.iter().map(|d| d.wasted_bytes).sum();
    let heap_size = app.analysis.summary.total_heap_size.max(1);
    let waste_pct = (total_wasted as f64 / heap_size as f64 * 100.0).min(100.0) as u16;

    let gauge_label = format!(
        " Total wasted by duplicate strings: {}  ({:.1}% of heap) ",
        fmt_bytes(total_wasted),
        waste_pct
    );

    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(C_BORDER))
                .style(Style::default().bg(C_PANEL))
                .title(Span::styled(" Waste Summary ", accent())),
        )
        .gauge_style(Style::default().fg(C_ACCENT).bg(C_PANEL))
        .percent(waste_pct)
        .label(gauge_label);

    f.render_widget(gauge, chunks[0]);

    // ── Table of duplicate strings ──
    let header_cells = ["#", "Value Preview", "Count", "Wasted"]
        .iter()
        .map(|h| Cell::from(*h).style(header_style()));
    let header = Row::new(header_cells)
        .height(1)
        .style(Style::default().bg(C_PANEL));

    let visible = chunks[1].height.saturating_sub(5) as usize;
    let scroll = app.dup_scroll;

    let rows: Vec<Row> = dups
        .iter()
        .enumerate()
        .skip(scroll)
        .take(visible)
        .map(|(i, d)| {
            let is_sel = i == app.dup_selected;
            let row_style = if is_sel { sel_style() } else { style() };
            Row::new(vec![
                Cell::from(format!("{}", i + 1)).style(dim()),
                Cell::from(d.value_preview.clone()).style(row_style),
                Cell::from(fmt_u64(d.count)).style(row_style),
                Cell::from(fmt_bytes(d.wasted_bytes)).style(if is_sel {
                    sel_style()
                } else {
                    Style::default().fg(C_WARN)
                }),
            ])
            .height(1)
            .style(row_style)
        })
        .collect();

    let scroll_info = if !dups.is_empty() {
        format!(" {}/{} ", app.dup_selected + 1, dups.len())
    } else {
        String::new()
    };

    let empty_msg = "  No duplicate string groups detected.";
    let content_note = if dups.is_empty() { empty_msg } else { "" };

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Min(40),
            Constraint::Length(10),
            Constraint::Length(12),
        ],
    )
    .header(header)
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(C_BORDER))
            .style(Style::default().bg(C_PANEL))
            .title(Span::styled(
                format!(" Duplicate Strings ({} groups) ", dups.len()),
                accent(),
            ))
            .title_bottom(Span::styled(scroll_info, dim())),
    )
    .column_spacing(1);

    if dups.is_empty() {
        let para = Paragraph::new(content_note).style(dim()).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(C_BORDER))
                .style(Style::default().bg(C_PANEL))
                .title(Span::styled(" Duplicate Strings ", accent())),
        );
        f.render_widget(para, chunks[1]);
    } else {
        f.render_widget(table, chunks[1]);
    }
}

// ────────────────────────────────────────────────────────────────────────────
// TAB 6: Help
// ────────────────────────────────────────────────────────────────────────────
fn draw_help(f: &mut Frame, area: Rect) {
    let text = vec![
        Line::from(vec![Span::styled(
            "  hprof-tui — Terminal HPROF Heap Dump Analyzer",
            Style::default().fg(C_ACCENT).add_modifier(Modifier::BOLD),
        )]),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Navigation",
            Style::default().fg(C_ACCENT2).add_modifier(Modifier::BOLD),
        )]),
        help_row("Tab / →", "Next tab"),
        help_row("Shift+Tab / ←", "Previous tab"),
        help_row("1-5", "Jump to tab directly"),
        help_row("↑ / k", "Scroll up one row"),
        help_row("↓ / j", "Scroll down one row"),
        help_row("Page Up / u", "Scroll up 10 rows"),
        help_row("Page Down / d", "Scroll down 10 rows"),
        help_row("g / Home", "Jump to top"),
        help_row("G / End", "Jump to bottom"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Actions",
            Style::default().fg(C_ACCENT2).add_modifier(Modifier::BOLD),
        )]),
        help_row("s", "Toggle histogram sort: size ↔ count"),
        help_row("q / Ctrl-C", "Quit"),
        help_row("?", "Show this help screen"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Tabs",
            Style::default().fg(C_ACCENT2).add_modifier(Modifier::BOLD),
        )]),
        help_row("1 Overview", "Heap summary stats + top classes bar chart"),
        help_row("2 Histogram", "All classes sorted by size or count"),
        help_row(
            "3 Leak Suspects",
            "Classes retaining >5% of heap, with severity",
        ),
        help_row("4 GC Roots", "GC root object summary by type"),
        help_row(
            "5 Dup Strings",
            "Duplicate java.lang.String instances wasting heap",
        ),
        Line::from(""),
        Line::from(vec![Span::styled(
            "  Colour Key",
            Style::default().fg(C_ACCENT2).add_modifier(Modifier::BOLD),
        )]),
        Line::from(vec![
            Span::raw("    "),
            Span::styled("■ RED", Style::default().fg(C_DANGER)),
            Span::styled(" = HIGH severity / >30% heap    ", dim()),
            Span::styled("■ YELLOW", Style::default().fg(C_WARN)),
            Span::styled(" = MED / 15-30%    ", dim()),
            Span::styled("■ GREEN", Style::default().fg(C_LOW)),
            Span::styled(" = LOW / 5-15%", dim()),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Tip: ", dim()),
            Span::styled("Open a heap dump with: ", dim()),
            Span::styled(
                "hprof-tui <path/to/heap.hprof>",
                Style::default().fg(C_ACCENT2),
            ),
        ]),
    ];

    let para = Paragraph::new(text)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(C_BORDER))
                .style(Style::default().bg(C_PANEL))
                .title(Span::styled(" Help ", accent())),
        )
        .wrap(Wrap { trim: false });

    f.render_widget(para, area);
}

// ────────────────────────────────────────────────────────────────────────────
// Helpers
// ────────────────────────────────────────────────────────────────────────────

fn panel_block(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(C_BORDER))
        .style(Style::default().bg(C_PANEL))
        .title(Span::styled(format!(" {} ", title), accent()))
        .title_alignment(Alignment::Left)
}

fn listitem_kv(key: &str, val: &str) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::styled(format!("  {:<16}", key), dim()),
        Span::styled(val.to_string(), Style::default().fg(C_TEXT)),
    ]))
}

fn fmt_u64(n: u64) -> String {
    // Add thousands separators
    let s = n.to_string();
    let mut result = String::new();
    for (i, ch) in s.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            result.push(',');
        }
        result.push(ch);
    }
    result.chars().rev().collect()
}

fn shorten_class(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        return name.to_string();
    }
    // Try to keep the simple class name at the end
    if let Some(dot_pos) = name.rfind('.') {
        let simple = &name[dot_pos + 1..];
        if simple.len() + 3 <= max_len {
            let prefix_len = max_len - simple.len() - 3;
            let prefix = &name[..prefix_len];
            return format!("{}…{}", prefix, simple);
        }
        return format!("…{}", &simple[..simple.len().min(max_len - 1)]);
    }
    format!("{}…", &name[..max_len - 1])
}

fn severity_color(sev: &crate::parser::SuspectSeverity) -> Color {
    match sev {
        crate::parser::SuspectSeverity::High => C_DANGER,
        crate::parser::SuspectSeverity::Medium => C_WARN,
        crate::parser::SuspectSeverity::Low => C_LOW,
    }
}

fn pct_color_style(pct: f64) -> Style {
    if pct > 30.0 {
        Style::default().fg(C_DANGER)
    } else if pct > 15.0 {
        Style::default().fg(C_WARN)
    } else if pct > 5.0 {
        Style::default().fg(C_ACCENT)
    } else {
        dim()
    }
}

fn help_row(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<22}", key), Style::default().fg(C_ACCENT)),
        Span::styled(desc.to_string(), Style::default().fg(C_TEXT)),
    ])
}

fn leak_tip(class_name: &str) -> String {
    let cn = class_name.to_lowercase();
    if cn.contains("string") {
        "Consider string interning or using byte arrays instead.".to_string()
    } else if cn.contains("hashmap") || cn.contains("map") {
        "Review map lifecycle; consider weak references or explicit eviction.".to_string()
    } else if cn.contains("list") || cn.contains("array") {
        "Check for unbounded list growth; add capacity limits or LRU eviction.".to_string()
    } else if cn.contains("thread") {
        "Thread objects alive may indicate thread-local leaks or pool misconfiguration.".to_string()
    } else if cn.contains("byte[]") || cn.contains("[b") {
        "Large byte arrays may be undrained I/O buffers or cached blobs.".to_string()
    } else if cn.contains("cache") {
        "Cache has no eviction policy or is not bounded — set a max size.".to_string()
    } else {
        format!(
            "Review references holding {}; look for static fields or long-lived listeners.",
            class_name
        )
    }
}
