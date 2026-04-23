//! TUI rendering — 7 tabs driven by HeapLens HeapAnalysis trait data.

use crate::app::{shorten, App, Tab};
use ratatui::{
    layout::{Alignment, Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{
        Block, BorderType, Borders, Cell, Gauge, List, ListItem, Paragraph, Row, Table, Tabs, Wrap,
    },
    Frame,
};

// ── Colour palette ────────────────────────────────────────────────────────────
const BG: Color = Color::Rgb(13, 16, 23);
const PANEL: Color = Color::Rgb(20, 24, 35);
const BORDER: Color = Color::Rgb(48, 58, 90);
const ACCENT: Color = Color::Rgb(90, 155, 255);
const TEAL: Color = Color::Rgb(65, 210, 175);
const WARN: Color = Color::Rgb(255, 195, 70);
const DANGER: Color = Color::Rgb(255, 80, 80);
const OK: Color = Color::Rgb(90, 210, 120);
const DIM: Color = Color::Rgb(85, 95, 125);
const TEXT: Color = Color::Rgb(205, 212, 232);
const SEL_BG: Color = Color::Rgb(32, 52, 88);

fn sty() -> Style {
    Style::default().fg(TEXT).bg(BG)
}
fn acc() -> Style {
    Style::default().fg(ACCENT)
}
fn dim() -> Style {
    Style::default().fg(DIM)
}
fn sel() -> Style {
    Style::default()
        .fg(Color::White)
        .bg(SEL_BG)
        .add_modifier(Modifier::BOLD)
}
fn hdr() -> Style {
    Style::default()
        .fg(Color::Rgb(140, 155, 200))
        .add_modifier(Modifier::BOLD)
}
fn warn() -> Style {
    Style::default().fg(WARN)
}

fn panel(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(PANEL))
        .title(Span::styled(format!(" {} ", title), acc()))
        .title_alignment(Alignment::Left)
}

fn header_row<const N: usize>(cols: [&str; N]) -> Row<'static> {
    Row::new(
        cols.iter()
            .map(|h| Cell::from(h.to_string()).style(hdr()))
            .collect::<Vec<_>>(),
    )
    .height(1)
    .style(Style::default().bg(PANEL))
}

// ── Entry point ───────────────────────────────────────────────────────────────
pub fn draw(f: &mut Frame, app: &App) {
    let area = f.area();
    f.render_widget(Block::default().style(sty()), area);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .split(area);

    draw_titlebar(f, app, chunks[0]);
    draw_tabbar(f, app, chunks[1]);
    match app.active_tab {
        Tab::Overview => tab_overview(f, app, chunks[2]),
        Tab::Histogram => tab_histogram(f, app, chunks[2]),
        Tab::Retained => tab_retained(f, app, chunks[2]),
        Tab::LeakSuspects => tab_leaks(f, app, chunks[2]),
        Tab::Waste => tab_waste(f, app, chunks[2]),
        Tab::DomTree => tab_domtree(f, app, chunks[2]),
        Tab::Help => tab_help(f, chunks[2]),
    }
    draw_statusbar(f, app, chunks[3]);
}

// ── Title bar ─────────────────────────────────────────────────────────────────
fn draw_titlebar(f: &mut Frame, app: &App, area: Rect) {
    let s = app.state.get_summary();
    let fname = std::path::Path::new(&app.path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown.hprof");
    let dom_badge = if app.has_dominators {
        Span::styled(" ✓ dominators ", Style::default().fg(OK))
    } else {
        Span::styled(" ⚡ phase-1 only ", Style::default().fg(WARN))
    };
    let line = Line::from(vec![
        Span::styled(
            "  ⬡ hprof-tui  ",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("│ ", dim()),
        Span::styled(
            fname,
            Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
        ),
        Span::styled(
            format!(
                "  {}",
                std::fs::metadata(&app.path)
                    .map(|m| fmt_bytes(m.len()))
                    .unwrap_or_default()
            ),
            dim(),
        ),
        Span::styled("  │  heap ", dim()),
        Span::styled(fmt_bytes(s.total_heap_size), acc()),
        Span::styled("  reachable ", dim()),
        Span::styled(fmt_bytes(s.reachable_heap_size), acc()),
        Span::styled("  │  ", dim()),
        dom_badge,
        Span::styled("  │  ", dim()),
        Span::styled(s.hprof_version.trim_end_matches('\0'), dim()),
    ]);
    f.render_widget(
        Paragraph::new(line).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER))
                .style(Style::default().bg(PANEL)),
        ),
        area,
    );
}

// ── Tab bar ───────────────────────────────────────────────────────────────────
fn draw_tabbar(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::ALL
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let n = if i < 6 {
                format!("[{}] ", i + 1)
            } else {
                String::new()
            };
            Line::from(vec![Span::styled(n, dim()), Span::raw(t.title())])
        })
        .collect();
    let tabs = Tabs::new(titles)
        .select(app.active_tab.index())
        .block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER))
                .style(Style::default().bg(PANEL)),
        )
        .highlight_style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))
        .divider(Span::styled(" │ ", dim()))
        .style(dim());
    f.render_widget(tabs, area);
}

// ── Status bar ────────────────────────────────────────────────────────────────
fn draw_statusbar(f: &mut Frame, app: &App, area: Rect) {
    let line = if let Some(ref msg) = app.status {
        Line::from(Span::styled(format!(" ✦ {}", msg), warn()))
    } else {
        Line::from(vec![
            Span::styled(" q", acc()),
            Span::styled(" quit  ", dim()),
            Span::styled("Tab/←→", acc()),
            Span::styled(" tab  ", dim()),
            Span::styled("1-6", acc()),
            Span::styled(" jump  ", dim()),
            Span::styled("↑↓jk", acc()),
            Span::styled(" scroll  ", dim()),
            Span::styled("PgDn/u", acc()),
            Span::styled(" page  ", dim()),
            Span::styled("s", acc()),
            Span::styled(" sort  ", dim()),
            Span::styled("Enter/i", acc()),
            Span::styled(" drill-in  ", dim()),
            Span::styled("Esc/o", acc()),
            Span::styled(" drill-out  ", dim()),
            Span::styled("?", acc()),
            Span::styled(" help", dim()),
        ])
    };
    f.render_widget(Paragraph::new(line).style(Style::default().bg(PANEL)), area);
}

// ── TAB 1: Overview ───────────────────────────────────────────────────────────
fn tab_overview(f: &mut Frame, app: &App, area: Rect) {
    let s = app.state.get_summary();
    let hist = app.state.get_class_histogram();
    let leaks = app.state.get_leak_suspects();
    let waste = app.state.get_waste_analysis();

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(36), Constraint::Percentage(64)])
        .split(area);
    let left = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(13), Constraint::Min(0)])
        .split(cols[0]);

    // Stats panel
    let file_size = std::fs::metadata(&app.path)
        .map(|m| fmt_bytes(m.len()))
        .unwrap_or_default();
    let stats: Vec<ListItem> = vec![
        lkv("File", shorten(&app.path, 36)),
        lkv(
            "Version",
            s.hprof_version.trim_end_matches('\0').to_string(),
        ),
        lkv("File size", file_size),
        lkv("Heap (total)", fmt_bytes(s.total_heap_size)),
        lkv("Reachable", fmt_bytes(s.reachable_heap_size)),
        lkv("Instances", fmt_n(s.total_instances)),
        lkv("Classes", fmt_n(s.total_classes)),
        lkv("Arrays", fmt_n(s.total_arrays)),
        lkv("GC roots", fmt_n(s.total_gc_roots)),
        lkv("Leak suspects", leaks.len().to_string()),
        lkv("Waste", fmt_bytes(waste.total_wasted_bytes)),
    ];
    f.render_widget(List::new(stats).block(panel("Heap Summary")), left[0]);

    // Issue badges
    let high = leaks
        .iter()
        .filter(|l| l.retained_percentage >= 30.0)
        .count();
    let med = leaks
        .iter()
        .filter(|l| l.retained_percentage >= 15.0 && l.retained_percentage < 30.0)
        .count();
    let low = leaks
        .iter()
        .filter(|l| l.retained_percentage < 15.0)
        .count();
    let badges: Vec<ListItem> = vec![
        badge("● HIGH", DANGER, high, ">30% heap"),
        badge("● MED ", WARN, med, "15–30% heap"),
        badge("● LOW ", OK, low, "5–15% heap"),
        ListItem::new(Line::from(vec![
            Span::styled("  Waste total  ", dim()),
            Span::styled(
                format!(
                    "{:.1}%  ({})",
                    waste.waste_percentage,
                    fmt_bytes(waste.total_wasted_bytes)
                ),
                warn(),
            ),
        ])),
    ];
    f.render_widget(List::new(badges).block(panel("Issues")), left[1]);

    // Right: bar chart by retained size
    let max_ret = hist.first().map(|e| e.retained_size).unwrap_or(1).max(1);
    let inner_h = cols[1].height.saturating_sub(2) as usize;
    let max_bars = (inner_h / 2).min(15).min(hist.len());
    let bar_w = (cols[1].width.saturating_sub(14) as usize).min(50);
    let mut lines: Vec<Line> = Vec::new();
    for (i, e) in hist.iter().take(max_bars).enumerate() {
        let pct = (e.retained_size as f64 / max_ret as f64 * 100.0) as u16;
        let col = if i == 0 {
            DANGER
        } else if i < 3 {
            WARN
        } else {
            ACCENT
        };
        let filled = (pct as usize * bar_w / 100).min(bar_w);
        lines.push(Line::from(vec![
            Span::styled(format!(" {:>2}. ", i + 1), Style::default().fg(col)),
            Span::styled(format!("{:<38}", shorten(&e.class_name, 38)), sty()),
            Span::styled(format!(" {:>10}", fmt_bytes(e.retained_size)), acc()),
        ]));
        lines.push(Line::from(vec![
            Span::raw("      "),
            Span::styled(
                "█".repeat(filled) + &"░".repeat(bar_w - filled),
                Style::default().fg(col),
            ),
            Span::styled(format!(" {:>3}%", pct), dim()),
        ]));
    }
    f.render_widget(
        Paragraph::new(lines)
            .block(panel("Top Classes by Retained Size"))
            .wrap(Wrap { trim: false }),
        cols[1],
    );
}

// ── TAB 2: Histogram ─────────────────────────────────────────────────────────
fn tab_histogram(f: &mut Frame, app: &App, area: Rect) {
    let entries = app.sorted_histogram();
    let total = app.state.get_summary().total_heap_size.max(1);
    let visible = area.height.saturating_sub(5) as usize;
    let sort_lbl = if app.hist_sort_retained {
        "retained"
    } else {
        "shallow"
    };
    let title = format!(
        "Class Histogram — {} classes — sorted by {}  [s] toggle",
        entries.len(),
        sort_lbl
    );

    let rows: Vec<Row> = entries
        .iter()
        .enumerate()
        .skip(app.hist_scroll)
        .take(visible)
        .map(|(i, e)| {
            let is_sel = i == app.hist_sel;
            let pct = e.retained_size as f64 / total as f64 * 100.0;
            let rs = if is_sel { sel() } else { sty() };
            let ps = if is_sel { sel() } else { pct_style(pct) };
            Row::new(vec![
                Cell::from(format!("{}", i + 1)).style(dim()),
                Cell::from(shorten(&e.class_name, 46)).style(rs),
                Cell::from(fmt_n(e.instance_count)).style(rs),
                Cell::from(fmt_bytes(e.shallow_size)).style(rs),
                Cell::from(fmt_bytes(e.retained_size)).style(ps),
                Cell::from(format!("{:.2}%", pct)).style(ps),
            ])
            .height(1)
            .style(rs)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(6),
            Constraint::Min(30),
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(8),
        ],
    )
    .header(header_row([
        "#",
        "Class",
        "Instances",
        "Shallow",
        "Retained",
        "% Heap",
    ]))
    .block(panel(&title))
    .column_spacing(1);

    f.render_widget(table, area);
    scroll_info(f, area, app.hist_sel + 1, entries.len());
}

// ── TAB 3: Retained ──────────────────────────────────────────────────────────
fn tab_retained(f: &mut Frame, app: &App, area: Rect) {
    if !app.has_dominators {
        f.render_widget(no_dom_msg("Retained Sizes"), area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(11)])
        .split(area);

    let hist = app.state.get_class_histogram(); // retained-sorted by engine
    let total = app.state.get_summary().total_heap_size.max(1);
    let vis = chunks[0].height.saturating_sub(5) as usize;

    let rows: Vec<Row> = hist
        .iter()
        .enumerate()
        .skip(app.ret_scroll)
        .take(vis)
        .map(|(i, e)| {
            let is_sel = i == app.ret_sel;
            let pct = e.retained_size as f64 / total as f64 * 100.0;
            let ovh = if e.shallow_size > 0 {
                e.retained_size as f64 / e.shallow_size as f64
            } else {
                1.0
            };
            let rs = if is_sel { sel() } else { sty() };
            let ret_s = if is_sel {
                sel()
            } else if pct > 10.0 {
                Style::default().fg(DANGER)
            } else if pct > 3.0 {
                Style::default().fg(WARN)
            } else {
                acc()
            };
            let ovh_s = if is_sel {
                sel()
            } else if ovh > 5.0 {
                Style::default().fg(WARN)
            } else {
                dim()
            };
            Row::new(vec![
                Cell::from(format!("{}", i + 1)).style(dim()),
                Cell::from(shorten(&e.class_name, 42)).style(rs),
                Cell::from(fmt_n(e.instance_count)).style(rs),
                Cell::from(fmt_bytes(e.shallow_size)).style(rs),
                Cell::from(fmt_bytes(e.retained_size)).style(ret_s),
                Cell::from(format!("{:.1}×", ovh)).style(ovh_s),
                Cell::from(format!("{:.2}%", pct)).style(ret_s),
            ])
            .height(1)
            .style(rs)
        })
        .collect();

    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Min(28),
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(11),
            Constraint::Length(8),
            Constraint::Length(8),
        ],
    )
    .header(header_row([
        "#",
        "Class",
        "Instances",
        "Shallow",
        "Retained",
        "×Overhead",
        "% Heap",
    ]))
    .block(panel(
        "Retained Sizes — sorted by retained size (Lengauer-Tarjan dominator tree)",
    ))
    .column_spacing(1);
    f.render_widget(table, chunks[0]);
    scroll_info(f, chunks[0], app.ret_sel + 1, hist.len());

    // Ingredients detail pane for selected class
    let detail = panel("Heap Ingredients — overhead breakdown for selected class");
    if let Some(e) = hist.get(app.ret_sel) {
        let pct = e.retained_size as f64 / total as f64 * 100.0;
        let ovh = if e.shallow_size > 0 {
            e.retained_size as f64 / e.shallow_size as f64
        } else {
            1.0
        };
        let self_pct = if e.retained_size > 0 {
            e.shallow_size as f64 / e.retained_size as f64 * 100.0
        } else {
            100.0
        };
        let b = |p: u64, tot: u64, w: usize| {
            let f = if tot > 0 {
                (p as f64 / tot as f64 * w as f64) as usize
            } else {
                0
            }
            .min(w);
            format!("{}{}", "█".repeat(f), "░".repeat(w - f))
        };
        let lines = vec![
            Line::from(vec![
                Span::styled(
                    format!("  {} ", shorten(&e.class_name, 50)),
                    Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  ×{} instances", fmt_n(e.instance_count)), dim()),
            ]),
            Line::from(vec![
                Span::styled("  Shallow  ", dim()),
                Span::styled(fmt_bytes(e.shallow_size), acc()),
                Span::styled("  │  Retained  ", dim()),
                Span::styled(
                    fmt_bytes(e.retained_size),
                    Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
                ),
                Span::styled(format!("  ({:.2}% of heap)", pct), dim()),
                Span::styled("  │  Overhead  ", dim()),
                Span::styled(
                    format!("{:.1}×", ovh),
                    if ovh > 5.0 {
                        warn()
                    } else {
                        Style::default().fg(OK)
                    },
                ),
                Span::styled(
                    format!(
                        "  │  Avg/instance  {}",
                        fmt_bytes(e.retained_size / e.instance_count.max(1))
                    ),
                    dim(),
                ),
            ]),
            Line::from(Span::styled("  ─".repeat(55), dim())),
            Line::from(vec![
                Span::styled(format!("  {:<40}", "self (own fields)"), acc()),
                Span::styled(format!(" {:>11}", fmt_bytes(e.shallow_size)), acc()),
                Span::styled(
                    format!("  {}", b(e.shallow_size, e.retained_size, 28)),
                    acc(),
                ),
                Span::styled(format!(" {:.1}%", self_pct), dim()),
            ]),
            Line::from(vec![
                Span::styled(format!("  {:<40}", "exclusively-owned children"), dim()),
                Span::styled(
                    format!(
                        " {:>11}",
                        fmt_bytes(e.retained_size.saturating_sub(e.shallow_size))
                    ),
                    Style::default().fg(TEAL),
                ),
                Span::styled(
                    format!(
                        "  {}",
                        b(
                            e.retained_size.saturating_sub(e.shallow_size),
                            e.retained_size,
                            28
                        )
                    ),
                    Style::default().fg(TEAL),
                ),
                Span::styled(format!(" {:.1}%", 100.0 - self_pct), dim()),
            ]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Tip: ", dim()),
                Span::styled(retention_tip(&e.class_name), sty()),
            ]),
        ];
        f.render_widget(
            Paragraph::new(lines)
                .block(detail)
                .wrap(Wrap { trim: false }),
            chunks[1],
        );
    } else {
        f.render_widget(
            Paragraph::new("  No class selected.")
                .style(dim())
                .block(detail),
            chunks[1],
        );
    }
}

// ── TAB 4: Leak Suspects ─────────────────────────────────────────────────────
fn tab_leaks(f: &mut Frame, app: &App, area: Rect) {
    let leaks = app.state.get_leak_suspects();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(7)])
        .split(area);

    let vis = chunks[0].height.saturating_sub(5) as usize;
    let rows: Vec<Row> = leaks
        .iter()
        .enumerate()
        .skip(app.leak_scroll)
        .take(vis)
        .map(|(i, s)| {
            let is_sel = i == app.leak_sel;
            let (sev_lbl, sev_col) = severity_label(s.retained_percentage);
            let rs = if is_sel { sel() } else { sty() };
            Row::new(vec![
                Cell::from(sev_lbl)
                    .style(Style::default().fg(sev_col).add_modifier(Modifier::BOLD)),
                Cell::from(shorten(&s.class_name, 40)).style(if is_sel {
                    sel()
                } else {
                    Style::default().fg(sev_col)
                }),
                Cell::from(if s.object_id > 0 {
                    format!("0x{:x}", s.object_id)
                } else {
                    "—".into()
                })
                .style(dim()),
                Cell::from(fmt_bytes(s.retained_size)).style(rs),
                Cell::from(format!("{:.1}%", s.retained_percentage)).style(rs),
                Cell::from(shorten(&s.description, 40)).style(dim()),
            ])
            .height(1)
            .style(rs)
        })
        .collect();

    let title = if leaks.is_empty() {
        "✔ No Leak Suspects detected".into()
    } else {
        format!(
            "⚠  {} Leak Suspect(s) — objects/classes retaining >10% of heap",
            leaks.len()
        )
    };
    let table = Table::new(
        rows,
        [
            Constraint::Length(5),
            Constraint::Min(26),
            Constraint::Length(14),
            Constraint::Length(11),
            Constraint::Length(7),
            Constraint::Min(20),
        ],
    )
    .header(header_row([
        "Sev",
        "Class",
        "Object ID",
        "Retained",
        "% Heap",
        "Description",
    ]))
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(PANEL))
            .title(Span::styled(
                format!(" {} ", title),
                Style::default().fg(WARN).add_modifier(Modifier::BOLD),
            )),
    )
    .column_spacing(1);
    f.render_widget(table, chunks[0]);
    scroll_info(f, chunks[0], app.leak_sel + 1, leaks.len());

    // Detail pane
    let det = panel("Suspect Detail");
    if let Some(s) = leaks.get(app.leak_sel) {
        let (_, col) = severity_label(s.retained_percentage);
        let lines = vec![
            Line::from(vec![
                Span::styled("  Class:   ", dim()),
                Span::styled(
                    &s.class_name,
                    Style::default().fg(col).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                Span::styled("  Retained:", dim()),
                Span::styled(
                    format!(
                        " {}  ({:.1}% of heap)",
                        fmt_bytes(s.retained_size),
                        s.retained_percentage
                    ),
                    acc(),
                ),
            ]),
            if s.object_id > 0 {
                Line::from(vec![
                    Span::styled("  Object:  ", dim()),
                    Span::styled(
                        format!(
                            " 0x{:x}  →  press 6, then navigate to this object in Dominator Tree",
                            s.object_id
                        ),
                        acc(),
                    ),
                ])
            } else {
                Line::from("")
            },
            Line::from(vec![
                Span::styled("  Note:    ", dim()),
                Span::styled(&s.description, sty()),
            ]),
            Line::from(vec![
                Span::styled("  Fix:     ", dim()),
                Span::styled(retention_tip(&s.class_name), sty()),
            ]),
        ];
        f.render_widget(
            Paragraph::new(lines).block(det).wrap(Wrap { trim: false }),
            chunks[1],
        );
    } else {
        f.render_widget(
            Paragraph::new("  No suspects — great!")
                .style(dim())
                .block(det),
            chunks[1],
        );
    }
}

// ── TAB 5: Waste ─────────────────────────────────────────────────────────────
fn tab_waste(f: &mut Frame, app: &App, area: Rect) {
    let w = app.state.get_waste_analysis();
    let heap = app.state.get_summary().total_heap_size.max(1);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5),
            Constraint::Length(11),
            Constraint::Min(0),
        ])
        .split(area);

    // Gauge
    let pct = (w.total_wasted_bytes as f64 / heap as f64 * 100.0).min(100.0) as u16;
    f.render_widget(
        Gauge::default()
            .block(panel("Waste Overview"))
            .gauge_style(Style::default().fg(WARN).bg(PANEL))
            .percent(pct)
            .label(format!(
                " Total waste: {}  ({:.1}% of heap) ",
                fmt_bytes(w.total_wasted_bytes),
                w.waste_percentage
            )),
        chunks[0],
    );

    // Breakdown list
    let breakdown: Vec<ListItem> = vec![
        waste_li(
            "Duplicate strings",
            w.duplicate_string_wasted_bytes,
            w.duplicate_strings.len(),
        ),
        waste_li(
            "Empty collections",
            w.empty_collection_wasted_bytes,
            w.empty_collections.len(),
        ),
        waste_li(
            "Over-allocated collections",
            w.over_allocated_wasted_bytes,
            w.over_allocated_collections.len(),
        ),
        waste_li("Boxed primitives", w.boxed_primitive_wasted_bytes, 0),
    ];
    f.render_widget(
        List::new(breakdown).block(panel("Waste Breakdown")),
        chunks[1],
    );

    // Dup strings table
    if !w.duplicate_strings.is_empty() {
        let vis = chunks[2].height.saturating_sub(5) as usize;
        let rows: Vec<Row> = w
            .duplicate_strings
            .iter()
            .enumerate()
            .skip(app.waste_scroll)
            .take(vis)
            .map(|(i, d)| {
                let is_sel = i == app.waste_sel;
                let rs = if is_sel { sel() } else { sty() };
                Row::new(vec![
                    Cell::from(format!("{}", i + 1)).style(dim()),
                    Cell::from(shorten(&d.preview, 50)).style(rs),
                    Cell::from(fmt_n(d.count)).style(rs),
                    Cell::from(fmt_bytes(d.wasted_bytes)).style(if is_sel {
                        sel()
                    } else {
                        warn()
                    }),
                    Cell::from(fmt_bytes(d.total_bytes)).style(rs),
                ])
                .height(1)
                .style(rs)
            })
            .collect();

        let title = format!(
            "Top Duplicate Strings ({} groups)",
            w.duplicate_strings.len()
        );

        let table = Table::new(
            rows,
            [
                Constraint::Length(5),
                Constraint::Min(35),
                Constraint::Length(9),
                Constraint::Length(11),
                Constraint::Length(11),
            ],
        )
        .header(header_row([
            "#",
            "String Preview",
            "Count",
            "Wasted",
            "Total",
        ]))
        .block(panel(&title))
        .column_spacing(1);
        f.render_widget(table, chunks[2]);
        scroll_info(f, chunks[2], app.waste_sel + 1, w.duplicate_strings.len());
    } else {
        f.render_widget(
            Paragraph::new("  No duplicate string groups found.")
                .style(dim())
                .block(panel("Duplicate Strings")),
            chunks[2],
        );
    }
}

// ── TAB 6: Dominator Tree ────────────────────────────────────────────────────
fn tab_domtree(f: &mut Frame, app: &App, area: Rect) {
    if !app.has_dominators {
        f.render_widget(no_dom_msg("Dominator Tree"), area);
        return;
    }

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3),
            Constraint::Min(0),
            Constraint::Length(5),
        ])
        .split(area);

    // Breadcrumb bar
    let crumb = format!(
        "  {}  ({} children at this level)",
        app.dom_breadcrumb(),
        app.dom_children.len()
    );
    f.render_widget(
        Paragraph::new(Span::styled(crumb, Style::default().fg(TEAL))).block(
            Block::default()
                .borders(Borders::ALL)
                .border_type(BorderType::Rounded)
                .border_style(Style::default().fg(BORDER))
                .style(Style::default().bg(PANEL)),
        ),
        chunks[0],
    );

    // Children table
    let total = app.state.get_summary().total_heap_size.max(1);
    let vis = chunks[1].height.saturating_sub(5) as usize;

    if app.dom_children.is_empty() {
        f.render_widget(
            Paragraph::new("\n  No children at this level — this is a leaf node.\n  Press Esc or 'o' to go back up.")
                .style(dim()).block(panel("Dominator Tree")),
            chunks[1],
        );
    } else {
        let rows: Vec<Row> = app
            .dom_children
            .iter()
            .enumerate()
            .skip(app.dom_scroll)
            .take(vis)
            .map(|(i, obj)| {
                let is_sel = i == app.dom_sel;
                let pct = obj.retained_size as f64 / total as f64 * 100.0;
                let rs = if is_sel { sel() } else { sty() };
                let name = if obj.class_name.is_empty() {
                    &obj.node_type
                } else {
                    &obj.class_name
                };
                let has_ch = app
                    .state
                    .get_children(obj.object_id)
                    .map(|c| !c.is_empty())
                    .unwrap_or(false);
                let arrow = if has_ch { "▶" } else { " " };
                Row::new(vec![
                    Cell::from(format!("{}", i + 1)).style(dim()),
                    Cell::from(arrow).style(if is_sel { sel() } else { acc() }),
                    Cell::from(shorten(name, 38)).style(rs),
                    Cell::from(shorten(&obj.node_type, 10)).style(dim()),
                    Cell::from(if obj.object_id > 0 {
                        format!("0x{:x}", obj.object_id)
                    } else {
                        "—".into()
                    })
                    .style(dim()),
                    Cell::from(fmt_bytes(obj.shallow_size)).style(rs),
                    Cell::from(fmt_bytes(obj.retained_size)).style(if is_sel {
                        sel()
                    } else {
                        pct_style(pct)
                    }),
                    Cell::from(format!("{:.2}%", pct)).style(if is_sel {
                        sel()
                    } else {
                        pct_style(pct)
                    }),
                ])
                .height(1)
                .style(rs)
            })
            .collect();

        let table = Table::new(
            rows,
            [
                Constraint::Length(4),
                Constraint::Length(2),
                Constraint::Min(24),
                Constraint::Length(10),
                Constraint::Length(14),
                Constraint::Length(11),
                Constraint::Length(11),
                Constraint::Length(8),
            ],
        )
        .header(header_row([
            "#",
            "",
            "Class",
            "Type",
            "Object ID",
            "Shallow",
            "Retained",
            "% Heap",
        ]))
        .block(panel(
            "Dominator Tree  [▶ = has children]  Enter/i = drill-in   Esc/o = up",
        ))
        .column_spacing(1);
        f.render_widget(table, chunks[1]);
        scroll_info(f, chunks[1], app.dom_sel + 1, app.dom_children.len());
    }

    // Mini help for this tab
    let help_lines = vec![
        Line::from(vec![
            Span::styled("  Enter / i", acc()),
            Span::styled(
                "  Drill into selected object (follow dominator edge)",
                dim(),
            ),
        ]),
        Line::from(vec![
            Span::styled("  Esc / o  ", acc()),
            Span::styled("  Go back up to parent level", dim()),
        ]),
        Line::from(vec![
            Span::styled("  ▶        ", acc()),
            Span::styled("  Object has dominator children (can drill in)", dim()),
        ]),
        Line::from(vec![
            Span::styled("  Depth: ", dim()),
            Span::styled(
                format!("{}", app.dom_stack.len()),
                Style::default().fg(TEAL),
            ),
            Span::styled(
                "  — retained size = shallow + sum of all exclusively-dominated objects",
                dim(),
            ),
        ]),
    ];
    f.render_widget(
        Paragraph::new(help_lines)
            .block(panel("Navigation Help"))
            .wrap(Wrap { trim: false }),
        chunks[2],
    );
}

// ── TAB 7: Help ──────────────────────────────────────────────────────────────
fn tab_help(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled(
            "  hprof-tui  v0.2  —  HeapLens engine edition",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Navigation",
            Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
        )),
        hr("Tab / → / l", "Next tab"),
        hr("Shift+Tab / ← / h", "Previous tab"),
        hr("1 – 6", "Jump to tab directly"),
        hr("↑↓ / j k", "Scroll one row"),
        hr("PgUp/PgDn / u d", "Scroll 10 rows"),
        hr("g / Home", "Jump to top"),
        hr("q / Ctrl-C", "Quit"),
        Line::from(""),
        Line::from(Span::styled(
            "  Actions",
            Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
        )),
        hr("s", "Toggle histogram sort: retained ↔ shallow"),
        hr("Enter / i", "Dominator Tree: drill into selected node"),
        hr("Esc / o", "Dominator Tree: go back to parent level"),
        hr("?", "This help screen"),
        Line::from(""),
        Line::from(Span::styled(
            "  Tabs",
            Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
        )),
        hr("1  Overview", "Heap stats + top-15 retained bar chart"),
        hr("2  Histogram", "All classes: shallow + retained, sortable"),
        hr("3  Retained", "Classes ranked by retained + overhead ratio"),
        hr("4  Leak Suspects", "Classes/objects retaining >10% heap"),
        hr("5  Waste", "Dup strings, empty/over-alloc collections"),
        hr(
            "6  Dominator Tree",
            "Interactive tree — drill into object graph",
        ),
        Line::from(""),
        Line::from(Span::styled(
            "  Engine",
            Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(
            "  Two-phase CSR parser + Lengauer-Tarjan dominators from HeapLens (Apache 2.0)",
            dim(),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Startup flags",
            Style::default().fg(TEAL).add_modifier(Modifier::BOLD),
        )),
        hr(
            "(default)",
            "Full analysis: Phase 1 + Phase 2 (dominators, retained sizes)",
        ),
        hr(
            "--phase1-only",
            "Fast startup — no dominators, no retained sizes",
        ),
    ];
    f.render_widget(
        Paragraph::new(lines)
            .block(panel("Help"))
            .wrap(Wrap { trim: false }),
        area,
    );
}

// ── Shared widgets ────────────────────────────────────────────────────────────

fn no_dom_msg(title: &str) -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Dominator tree not available.",
            Style::default().fg(WARN),
        )),
        Line::from(Span::styled(
            "  Re-run without --phase1-only to enable retained sizes and dominator tree.",
            dim(),
        )),
    ])
    .block(
        Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(BORDER))
            .style(Style::default().bg(PANEL))
            .title(Span::styled(format!(" {} ", title), acc())),
    )
}

fn scroll_info(f: &mut Frame, area: Rect, cur: usize, total: usize) {
    if total == 0 {
        return;
    }
    let txt = format!(" {}/{} ", cur, total);
    let x = area.x + 2;
    let y = area.y + area.height.saturating_sub(1);
    if y < area.y + area.height {
        f.render_widget(
            Paragraph::new(Span::styled(txt, dim())),
            Rect {
                x,
                y,
                width: 20,
                height: 1,
            },
        );
    }
}

// ── Format helpers ────────────────────────────────────────────────────────────

pub fn fmt_bytes(b: u64) -> String {
    if b == 0 {
        return "0 B".into();
    }
    const U: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let i = ((b as f64).log(1024.0).floor() as usize).min(U.len() - 1);
    let v = b as f64 / 1024f64.powi(i as i32);
    if i > 1 {
        format!("{:.2} {}", v, U[i])
    } else {
        format!("{:.0} {}", v, U[i])
    }
}

fn fmt_n(n: u64) -> String {
    let s = n.to_string();
    let mut r = String::new();
    for (j, c) in s.chars().rev().enumerate() {
        if j > 0 && j % 3 == 0 {
            r.push(',');
        }
        r.push(c);
    }
    r.chars().rev().collect()
}

fn pct_style(pct: f64) -> Style {
    if pct > 10.0 {
        Style::default().fg(DANGER)
    } else if pct > 3.0 {
        Style::default().fg(WARN)
    } else if pct > 0.5 {
        acc()
    } else {
        dim()
    }
}

fn severity_label(pct: f64) -> (&'static str, Color) {
    if pct >= 30.0 {
        ("HIGH", DANGER)
    } else if pct >= 15.0 {
        ("MED ", WARN)
    } else {
        ("LOW ", OK)
    }
}

fn lkv(k: &str, v: String) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::styled(format!("  {:<18}", k), dim()),
        Span::styled(v, sty()),
    ]))
}

fn badge(label: &str, col: Color, count: usize, desc: &str) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::styled(
            format!("  {} ", label),
            Style::default().fg(col).add_modifier(Modifier::BOLD),
        ),
        Span::styled(count.to_string(), sty()),
        Span::styled(format!("  {}", desc), dim()),
    ]))
}

fn waste_li(label: &str, bytes: u64, groups: usize) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::styled(format!("  {:<32}", label), dim()),
        Span::styled(format!("{:>11}", fmt_bytes(bytes)), warn()),
        if groups > 0 {
            Span::styled(format!("  ({} groups)", groups), dim())
        } else {
            Span::raw("")
        },
    ]))
}

fn hr(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {:<24}", key), acc()),
        Span::styled(desc.to_string(), sty()),
    ])
}

fn retention_tip(class: &str) -> String {
    let c = class.to_lowercase();
    if c.contains("string") {
        "Use String.intern() or byte[]; audit caches and ThreadLocals.".into()
    } else if c.contains("statement") || c.contains("jdbc") {
        "Close PreparedStatement in try-with-resources; limit statement cache.".into()
    } else if c.contains("hashmap") || c.contains("map") {
        "Bound map sizes; use WeakHashMap or Caffeine with eviction.".into()
    } else if c.contains("list") || c.contains("array") {
        "Enable pagination/virtualization; trim after bulk ops.".into()
    } else if c.contains("thread") {
        "Check ThreadLocals and pool sizes; call ThreadLocal.remove().".into()
    } else if c.contains("byte[]") || c.contains("char[]") {
        "Large byte/char arrays may be I/O buffers — flush and close streams.".into()
    } else if c.contains("session") {
        "Shorten session TTL; call session.invalidate() on logout.".into()
    } else if c.contains("cache") {
        "Set max size and TTL; use SoftReference or Caffeine.".into()
    } else {
        format!(
            "Review static fields and long-lived caches holding {}.",
            class
        )
    }
}
