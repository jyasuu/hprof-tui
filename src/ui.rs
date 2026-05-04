//! TUI rendering — 9 tabs driven by HeapLens HeapAnalysis trait data.

use crate::app::{shorten, App, InputMode, InspectorFocus, Tab};
use hprof_analyzer::WasteAnalysis;
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
const EDIT_BG: Color = Color::Rgb(25, 40, 65);

fn sty() -> Style { Style::default().fg(TEXT).bg(BG) }
fn acc() -> Style { Style::default().fg(ACCENT) }
fn dim() -> Style { Style::default().fg(DIM) }
fn sel() -> Style { Style::default().fg(Color::White).bg(SEL_BG).add_modifier(Modifier::BOLD) }
fn hdr() -> Style { Style::default().fg(Color::Rgb(140, 155, 200)).add_modifier(Modifier::BOLD) }
fn warn() -> Style { Style::default().fg(WARN) }
fn ok() -> Style { Style::default().fg(OK) }
fn teal() -> Style { Style::default().fg(TEAL) }
fn danger() -> Style { Style::default().fg(DANGER) }

fn panel(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(BORDER))
        .style(Style::default().bg(PANEL))
        .title(Span::styled(format!(" {} ", title), acc()))
        .title_alignment(Alignment::Left)
}

fn panel_focused(title: &str) -> Block<'_> {
    Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Style::default().fg(ACCENT))
        .style(Style::default().bg(PANEL))
        .title(Span::styled(format!(" {} ", title), Style::default().fg(TEAL).add_modifier(Modifier::BOLD)))
        .title_alignment(Alignment::Left)
}

fn header_row<const N: usize>(cols: [&str; N]) -> Row<'static> {
    Row::new(cols.iter().map(|h| Cell::from(h.to_string()).style(hdr())).collect::<Vec<_>>())
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
        Tab::Query => tab_query(f, app, chunks[2]),
        Tab::Inspector => tab_inspector(f, app, chunks[2]),
        Tab::Help => tab_help(f, chunks[2]),
    }
    draw_statusbar(f, app, chunks[3]);
}

// ── Title bar ─────────────────────────────────────────────────────────────────
fn draw_titlebar(f: &mut Frame, app: &App, area: Rect) {
    let s = app.state.get_summary();
    let fname = std::path::Path::new(&app.path)
        .file_name().and_then(|n| n.to_str()).unwrap_or("unknown.hprof");
    let dom_badge = if app.has_dominators {
        Span::styled(" ✓ dominators ", Style::default().fg(OK))
    } else {
        Span::styled(" ⚡ phase-1 only ", Style::default().fg(WARN))
    };
    let line = Line::from(vec![
        Span::styled("  ⬡ hprof-tui  ", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        Span::styled("│ ", dim()),
        Span::styled(fname, Style::default().fg(TEAL).add_modifier(Modifier::BOLD)),
        Span::styled(format!("  {}", std::fs::metadata(&app.path).map(|m| fmt_bytes(m.len())).unwrap_or_default()), dim()),
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
        Paragraph::new(line).block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(BORDER)).style(Style::default().bg(PANEL))),
        area,
    );
}

// ── Tab bar ───────────────────────────────────────────────────────────────────
fn draw_tabbar(f: &mut Frame, app: &App, area: Rect) {
    let titles: Vec<Line> = Tab::ALL.iter().enumerate().map(|(i, t)| {
        let n = if i < 8 { format!("[{}] ", i + 1) } else { "".to_string() };
        Line::from(vec![Span::styled(n, dim()), Span::raw(t.title())])
    }).collect();
    let tabs = Tabs::new(titles)
        .select(app.active_tab.index())
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(BORDER)).style(Style::default().bg(PANEL)))
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
            Span::styled(" q", acc()), Span::styled(" quit  ", dim()),
            Span::styled("Tab/←→", acc()), Span::styled(" tab  ", dim()),
            Span::styled("1-8", acc()), Span::styled(" jump  ", dim()),
            Span::styled("↑↓jk", acc()), Span::styled(" scroll  ", dim()),
            Span::styled("PgDn/u", acc()), Span::styled(" page  ", dim()),
            Span::styled("s", acc()), Span::styled(" sort  ", dim()),
            Span::styled("7", acc()), Span::styled(" HeapQL  ", dim()),
            Span::styled("8", acc()), Span::styled(" Inspector  ", dim()),
            Span::styled("x", acc()), Span::styled(" inspect sel  ", dim()),
            Span::styled("?", acc()), Span::styled(" help", dim()),
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

    let file_size = std::fs::metadata(&app.path).map(|m| fmt_bytes(m.len())).unwrap_or_default();
    let stats: Vec<ListItem> = vec![
        lkv("File", shorten(&app.path, 36)),
        lkv("Version", s.hprof_version.trim_end_matches('\0').to_string()),
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

    let high = leaks.iter().filter(|l| l.retained_percentage >= 30.0).count();
    let med = leaks.iter().filter(|l| l.retained_percentage >= 15.0 && l.retained_percentage < 30.0).count();
    let low = leaks.iter().filter(|l| l.retained_percentage < 15.0).count();
    let badges: Vec<ListItem> = vec![
        badge("● HIGH", DANGER, high, ">30% heap"),
        badge("● MED ", WARN, med, "15–30% heap"),
        badge("● LOW ", OK, low, "5–15% heap"),
        ListItem::new(Line::from(vec![
            Span::styled("  Waste total  ", dim()),
            Span::styled(format!("{:.1}%  ({})", waste.waste_percentage, fmt_bytes(waste.total_wasted_bytes)), warn()),
        ])),
    ];
    f.render_widget(List::new(badges).block(panel("Issues")), left[1]);

    let reachable = s.reachable_heap_size.max(1);
    let normal_max = hist.iter().map(|e| e.retained_size.min(reachable)).max().unwrap_or(1).max(1);
    let inner_h = cols[1].height.saturating_sub(2) as usize;
    let max_bars = (inner_h / 2).min(15).min(hist.len());
    let bar_w = (cols[1].width.saturating_sub(16) as usize).min(48);
    let mut lines: Vec<Line> = Vec::new();
    for (i, e) in hist.iter().take(max_bars).enumerate() {
        let abnormal = e.retained_size > reachable;
        let capped = e.retained_size.min(reachable);
        let bar_pct = (capped as f64 / normal_max as f64 * 100.0) as u16;
        let heap_pct = e.retained_size as f64 / reachable as f64 * 100.0;
        let col = if abnormal || i == 0 { DANGER } else if i < 3 { WARN } else { ACCENT };
        let filled = (bar_pct as usize * bar_w / 100).min(bar_w);
        let pct_label = if abnormal { "⚠>100%".to_string() } else { format!("{:.1}%", heap_pct) };
        lines.push(Line::from(vec![
            Span::styled(format!(" {:>2}. ", i + 1), Style::default().fg(col)),
            Span::styled(format!("{:<36}", shorten(&e.class_name, 36)), sty()),
            Span::styled(format!(" {:>10}", fmt_bytes(e.retained_size)), if abnormal { danger() } else { acc() }),
        ]));
        lines.push(Line::from(vec![
            Span::raw("      "),
            Span::styled("█".repeat(filled) + &"░".repeat(bar_w - filled), Style::default().fg(col)),
            Span::styled(format!(" {}", pct_label), if abnormal { danger() } else { dim() }),
        ]));
    }
    f.render_widget(Paragraph::new(lines).block(panel("Top Classes by Retained Size")).wrap(Wrap { trim: false }), cols[1]);
}

// ── TAB 2: Histogram ─────────────────────────────────────────────────────────
fn tab_histogram(f: &mut Frame, app: &App, area: Rect) {
    let entries = app.sorted_histogram();
    let total = app.state.get_summary().total_heap_size.max(1);
    let visible = area.height.saturating_sub(5) as usize;
    let sort_lbl = if app.hist_sort_retained { "retained" } else { "shallow" };
    let title = format!("Class Histogram — {} classes — sorted by {}  [s] toggle", entries.len(), sort_lbl);

    let rows: Vec<Row> = entries.iter().enumerate().skip(app.hist_scroll).take(visible).map(|(i, e)| {
        let is_sel = i == app.hist_sel;
        let (pct_str, abnormal) = retained_pct(e.retained_size, total);
        let rs = if is_sel { sel() } else { sty() };
        let ps = pct_display_style(&pct_str, abnormal, is_sel);
        Row::new(vec![
            Cell::from(format!("{}", i + 1)).style(dim()),
            Cell::from(shorten(&e.class_name, 46)).style(rs),
            Cell::from(fmt_n(e.instance_count)).style(rs),
            Cell::from(fmt_bytes(e.shallow_size)).style(rs),
            Cell::from(fmt_bytes(e.retained_size)).style(if abnormal && !is_sel { danger() } else { ps }),
            Cell::from(pct_str).style(ps),
        ]).height(1).style(rs)
    }).collect();

    let table = Table::new(rows, [Constraint::Length(5), Constraint::Min(30), Constraint::Length(11), Constraint::Length(11), Constraint::Length(12), Constraint::Length(8)])
        .header(header_row(["#", "Class", "Instances", "Shallow", "Retained", "% Heap"]))
        .block(panel(&title)).column_spacing(1);
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

    let hist = app.state.get_class_histogram();
    let total = app.state.get_summary().total_heap_size.max(1);
    let vis = chunks[0].height.saturating_sub(5) as usize;

    let rows: Vec<Row> = hist.iter().enumerate().skip(app.ret_scroll).take(vis).map(|(i, e)| {
        let is_sel = i == app.ret_sel;
        let (pct_str, abnormal) = retained_pct(e.retained_size, total);
        let ovh = if e.shallow_size > 0 { e.retained_size as f64 / e.shallow_size as f64 } else { 1.0 };
        let rs = if is_sel { sel() } else { sty() };
        let ret_s = if is_sel { sel() } else if abnormal { danger() } else { pct_display_style(&pct_str, false, false) };
        let ovh_s = if is_sel { sel() } else if ovh > 5.0 { warn() } else { dim() };
        Row::new(vec![
            Cell::from(format!("{}", i + 1)).style(dim()),
            Cell::from(shorten(&e.class_name, 42)).style(rs),
            Cell::from(fmt_n(e.instance_count)).style(rs),
            Cell::from(fmt_bytes(e.shallow_size)).style(rs),
            Cell::from(fmt_bytes(e.retained_size)).style(ret_s),
            Cell::from(if ovh > 9999.0 { "⚠huge".to_string() } else { format!("{:.1}×", ovh) }).style(ovh_s),
            Cell::from(pct_str).style(ret_s),
        ]).height(1).style(rs)
    }).collect();

    let table = Table::new(rows, [Constraint::Length(5), Constraint::Min(28), Constraint::Length(10), Constraint::Length(11), Constraint::Length(12), Constraint::Length(8), Constraint::Length(8)])
        .header(header_row(["#", "Class", "Instances", "Shallow", "Retained", "×Over", "% Heap"]))
        .block(panel("Retained Sizes — sorted by retained size (Lengauer-Tarjan dominator tree)"))
        .column_spacing(1);
    f.render_widget(table, chunks[0]);
    scroll_info(f, chunks[0], app.ret_sel + 1, hist.len());

    let detail = panel("Heap Ingredients — overhead breakdown for selected class");
    if let Some(e) = hist.get(app.ret_sel) {
        let pct = e.retained_size as f64 / total as f64 * 100.0;
        let ovh = if e.shallow_size > 0 { e.retained_size as f64 / e.shallow_size as f64 } else { 1.0 };
        let self_pct = if e.retained_size > 0 { e.shallow_size as f64 / e.retained_size as f64 * 100.0 } else { 100.0 };
        let b = |p: u64, tot: u64, w: usize| {
            let f = if tot > 0 { (p as f64 / tot as f64 * w as f64) as usize } else { 0 }.min(w);
            format!("{}{}", "█".repeat(f), "░".repeat(w - f))
        };
        let lines = vec![
            Line::from(vec![
                Span::styled(format!("  {} ", shorten(&e.class_name, 50)), teal().add_modifier(Modifier::BOLD)),
                Span::styled(format!("  ×{} instances", fmt_n(e.instance_count)), dim()),
            ]),
            Line::from(vec![
                Span::styled("  Shallow  ", dim()), Span::styled(fmt_bytes(e.shallow_size), acc()),
                Span::styled("  │  Retained  ", dim()), Span::styled(fmt_bytes(e.retained_size), teal().add_modifier(Modifier::BOLD)),
                Span::styled(format!("  ({:.2}% of heap)", pct), dim()),
                Span::styled("  │  Overhead  ", dim()), Span::styled(format!("{:.1}×", ovh), if ovh > 5.0 { warn() } else { ok() }),
                Span::styled(format!("  │  Avg/instance  {}", fmt_bytes(e.retained_size / e.instance_count.max(1))), dim()),
            ]),
            Line::from(Span::styled("  ─".repeat(55), dim())),
            Line::from(vec![
                Span::styled(format!("  {:<40}", "self (own fields)"), acc()),
                Span::styled(format!(" {:>11}", fmt_bytes(e.shallow_size)), acc()),
                Span::styled(format!("  {}", b(e.shallow_size, e.retained_size, 28)), acc()),
                Span::styled(format!(" {:.1}%", self_pct), dim()),
            ]),
            Line::from(vec![
                Span::styled(format!("  {:<40}", "exclusively-owned children"), dim()),
                Span::styled(format!(" {:>11}", fmt_bytes(e.retained_size.saturating_sub(e.shallow_size))), teal()),
                Span::styled(format!("  {}", b(e.retained_size.saturating_sub(e.shallow_size), e.retained_size, 28)), teal()),
                Span::styled(format!(" {:.1}%", 100.0 - self_pct), dim()),
            ]),
            Line::from(""),
            Line::from(vec![Span::styled("  Tip: ", dim()), Span::styled(retention_tip(&e.class_name), sty())]),
        ];
        f.render_widget(Paragraph::new(lines).block(detail).wrap(Wrap { trim: false }), chunks[1]);
    } else {
        f.render_widget(Paragraph::new("  No class selected.").style(dim()).block(detail), chunks[1]);
    }
}

// ── TAB 4: Leak Suspects ─────────────────────────────────────────────────────
fn tab_leaks(f: &mut Frame, app: &App, area: Rect) {
    let leaks = if app.leak_show_objects {
        app.state.get_object_leak_suspects()
    } else {
        app.state.get_leak_suspects()
    };
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(7)])
        .split(area);

    let (sel_idx, scroll_idx) = if app.leak_show_objects {
        (app.obj_leak_sel, app.obj_leak_scroll)
    } else {
        (app.leak_sel, app.leak_scroll)
    };
    let view_label = if app.leak_show_objects { "Object-level" } else { "Class-level" };
    let vis = chunks[0].height.saturating_sub(5) as usize;

    let rows: Vec<Row> = leaks.iter().enumerate().skip(scroll_idx).take(vis).map(|(i, s)| {
        let is_sel = i == sel_idx;
        let (sev_lbl, sev_col) = severity_label(s.retained_percentage);
        let rs = if is_sel { sel() } else { sty() };
        Row::new(vec![
            Cell::from(sev_lbl).style(Style::default().fg(sev_col).add_modifier(Modifier::BOLD)),
            Cell::from(shorten(&s.class_name, 40)).style(if is_sel { sel() } else { Style::default().fg(sev_col) }),
            Cell::from(if s.object_id > 0 { format!("0x{:x}", s.object_id) } else { "—".into() }).style(dim()),
            Cell::from(fmt_bytes(s.retained_size)).style(rs),
            Cell::from(if s.retained_percentage > 100.0 { "⚠>100%".to_string() } else { format!("{:.1}%", s.retained_percentage) }).style(rs),
            Cell::from(shorten(&s.description, 40)).style(dim()),
        ]).height(1).style(rs)
    }).collect();

    let title = if leaks.is_empty() {
        format!("✔ No {} Leak Suspects detected", view_label)
    } else {
        format!("⚠  {} {} Suspect(s)  [s] toggle view — retaining >10% of heap", leaks.len(), view_label)
    };
    let table = Table::new(rows, [Constraint::Length(5), Constraint::Min(26), Constraint::Length(14), Constraint::Length(12), Constraint::Length(8), Constraint::Min(20)])
        .header(header_row(["Sev", "Class", "Object ID", "Retained", "% Heap", "Description"]))
        .block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(BORDER)).style(Style::default().bg(PANEL)).title(Span::styled(format!(" {} ", title), warn().add_modifier(Modifier::BOLD))))
        .column_spacing(1);
    f.render_widget(table, chunks[0]);
    scroll_info(f, chunks[0], sel_idx + 1, leaks.len());

    let det = panel("Suspect Detail  [s]=toggle class/object view  [8]=open in Inspector");
    if let Some(s) = leaks.get(sel_idx) {
        let (_, col) = severity_label(s.retained_percentage);
        let lines = vec![
            Line::from(vec![Span::styled("  Class:   ", dim()), Span::styled(&s.class_name, Style::default().fg(col).add_modifier(Modifier::BOLD))]),
            Line::from(vec![Span::styled("  Retained:", dim()), Span::styled(format!(" {}  ({:.1}% of heap)", fmt_bytes(s.retained_size), s.retained_percentage), acc())]),
            if s.object_id > 0 {
                Line::from(vec![Span::styled("  Object:  ", dim()), Span::styled(format!(" 0x{:x}  →  press 8 to open in Inspector", s.object_id), acc())])
            } else { Line::from("") },
            Line::from(vec![Span::styled("  Note:    ", dim()), Span::styled(&s.description, sty())]),
            Line::from(vec![Span::styled("  Fix:     ", dim()), Span::styled(retention_tip(&s.class_name), sty())]),
        ];
        f.render_widget(Paragraph::new(lines).block(det).wrap(Wrap { trim: false }), chunks[1]);
    } else {
        f.render_widget(Paragraph::new("  No suspects — great!").style(dim()).block(det), chunks[1]);
    }
}

// ── TAB 5: Waste ─────────────────────────────────────────────────────────────
fn tab_waste(f: &mut Frame, app: &App, area: Rect) {
    let w = app.state.get_waste_analysis();
    let heap = app.state.get_summary().total_heap_size.max(1);

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Length(8), Constraint::Min(0)])
        .split(area);

    // Gauge
    let pct = (w.total_wasted_bytes as f64 / heap as f64 * 100.0).min(100.0) as u16;
    f.render_widget(
        Gauge::default()
            .block(panel("Waste Overview  [s]=cycle sub-view"))
            .gauge_style(Style::default().fg(WARN).bg(PANEL))
            .percent(pct)
            .label(format!(" Total: {}  ({:.1}%)  Strings {} | Empty colls {} | Over-alloc {} | Boxed {} ",
                fmt_bytes(w.total_wasted_bytes), w.waste_percentage,
                fmt_bytes(w.duplicate_string_wasted_bytes),
                fmt_bytes(w.empty_collection_wasted_bytes),
                fmt_bytes(w.over_allocated_wasted_bytes),
                fmt_bytes(w.boxed_primitive_wasted_bytes),
            )),
        chunks[0],
    );

    // Breakdown list — highlight active sub-view
    let sub_labels = ["Duplicate strings", "Empty collections", "Over-allocated collections", "Boxed primitives"];
    let sub_bytes = [w.duplicate_string_wasted_bytes, w.empty_collection_wasted_bytes, w.over_allocated_wasted_bytes, w.boxed_primitive_wasted_bytes];
    let sub_counts = [w.duplicate_strings.len(), w.empty_collections.len(), w.over_allocated_collections.len(), w.boxed_primitives.len()];
    let breakdown: Vec<ListItem> = (0..4).map(|i| {
        let active = i == app.waste_sub;
        let label_style = if active { Style::default().fg(TEAL).add_modifier(Modifier::BOLD) } else { dim() };
        let bytes_style = if active { warn().add_modifier(Modifier::BOLD) } else { warn() };
        let marker = if active { "▶ " } else { "  " };
        ListItem::new(Line::from(vec![
            Span::styled(format!("  {}{:<34}", marker, sub_labels[i]), label_style),
            Span::styled(format!("{:>11}", fmt_bytes(sub_bytes[i])), bytes_style),
            Span::styled(format!("  ({} groups)", sub_counts[i]), dim()),
        ]))
    }).collect();
    f.render_widget(List::new(breakdown).block(panel("Waste Breakdown  [s]=cycle")), chunks[1]);

    // Active sub-view table
    waste_sub_table(f, app, w, chunks[2]);
}

fn waste_sub_table(f: &mut Frame, app: &App, w: &WasteAnalysis, area: Rect) {
    let vis = area.height.saturating_sub(5) as usize;
    match app.waste_sub {
        0 => {
            if w.duplicate_strings.is_empty() {
                f.render_widget(Paragraph::new("  No duplicate strings found — great!").style(dim()).block(panel("Duplicate Strings")), area);
                return;
            }
            let rows: Vec<Row> = w.duplicate_strings.iter().enumerate().skip(app.waste_scroll).take(vis).map(|(i, d)| {
                let is_sel = i == app.waste_sel;
                let rs = if is_sel { sel() } else { sty() };
                Row::new(vec![
                    Cell::from(format!("{}", i + 1)).style(dim()),
                    Cell::from(shorten(&d.preview, 48)).style(rs),
                    Cell::from(fmt_n(d.count)).style(rs),
                    Cell::from(fmt_bytes(d.wasted_bytes)).style(if is_sel { sel() } else { warn() }),
                    Cell::from(fmt_bytes(d.total_bytes)).style(rs),
                ]).height(1).style(rs)
            }).collect();
            let title = format!("Duplicate Strings — {} groups  ({} wasted)", w.duplicate_strings.len(), fmt_bytes(w.duplicate_string_wasted_bytes));
            let table = Table::new(rows, [Constraint::Length(5), Constraint::Min(35), Constraint::Length(9), Constraint::Length(11), Constraint::Length(11)])
                .header(header_row(["#", "String Preview", "Copies", "Wasted", "Total"]))
                .block(panel(&title)).column_spacing(1);
            f.render_widget(table, area);
            scroll_info(f, area, app.waste_sel + 1, w.duplicate_strings.len());
        }
        1 => {
            if w.empty_collections.is_empty() {
                f.render_widget(Paragraph::new("  No empty collections found.").style(dim()).block(panel("Empty Collections")), area);
                return;
            }
            let rows: Vec<Row> = w.empty_collections.iter().enumerate().skip(app.waste_scroll).take(vis).map(|(i, e)| {
                let is_sel = i == app.waste_sel;
                let rs = if is_sel { sel() } else { sty() };
                Row::new(vec![
                    Cell::from(format!("{}", i + 1)).style(dim()),
                    Cell::from(shorten(&e.class_name, 44)).style(rs),
                    Cell::from(fmt_n(e.count)).style(rs),
                    Cell::from(fmt_bytes(e.wasted_bytes)).style(if is_sel { sel() } else { warn() }),
                ]).height(1).style(rs)
            }).collect();
            let title = format!("Empty Collections — {} groups  ({} wasted)", w.empty_collections.len(), fmt_bytes(w.empty_collection_wasted_bytes));
            let table = Table::new(rows, [Constraint::Length(5), Constraint::Min(38), Constraint::Length(11), Constraint::Length(11)])
                .header(header_row(["#", "Class", "Count", "Wasted"]))
                .block(panel(&title)).column_spacing(1);
            f.render_widget(table, area);
            scroll_info(f, area, app.waste_sel + 1, w.empty_collections.len());
        }
        2 => {
            if w.over_allocated_collections.is_empty() {
                f.render_widget(Paragraph::new("  No over-allocated collections found.").style(dim()).block(panel("Over-Allocated Collections")), area);
                return;
            }
            let rows: Vec<Row> = w.over_allocated_collections.iter().enumerate().skip(app.waste_scroll).take(vis).map(|(i, e)| {
                let is_sel = i == app.waste_sel;
                let rs = if is_sel { sel() } else { sty() };
                Row::new(vec![
                    Cell::from(format!("{}", i + 1)).style(dim()),
                    Cell::from(shorten(&e.class_name, 38)).style(rs),
                    Cell::from(fmt_n(e.count)).style(rs),
                    Cell::from(fmt_bytes(e.wasted_bytes)).style(if is_sel { sel() } else { warn() }),
                    Cell::from(format!("{:.0}%", e.avg_fill_ratio)).style(if is_sel { sel() } else { pct_style(100.0 - e.avg_fill_ratio) }),
                ]).height(1).style(rs)
            }).collect();
            let title = format!("Over-Allocated Collections — {} groups  ({} wasted)", w.over_allocated_collections.len(), fmt_bytes(w.over_allocated_wasted_bytes));
            let table = Table::new(rows, [Constraint::Length(5), Constraint::Min(34), Constraint::Length(11), Constraint::Length(11), Constraint::Length(8)])
                .header(header_row(["#", "Class", "Count", "Wasted", "Fill%"]))
                .block(panel(&title)).column_spacing(1);
            f.render_widget(table, area);
            scroll_info(f, area, app.waste_sel + 1, w.over_allocated_collections.len());
        }
        3 => {
            if w.boxed_primitives.is_empty() {
                f.render_widget(Paragraph::new("  No boxed primitives detected.").style(dim()).block(panel("Boxed Primitives")), area);
                return;
            }
            let rows: Vec<Row> = w.boxed_primitives.iter().enumerate().skip(app.waste_scroll).take(vis).map(|(i, e)| {
                let is_sel = i == app.waste_sel;
                let rs = if is_sel { sel() } else { sty() };
                Row::new(vec![
                    Cell::from(format!("{}", i + 1)).style(dim()),
                    Cell::from(shorten(&e.class_name, 30)).style(rs),
                    Cell::from(fmt_n(e.count)).style(rs),
                    Cell::from(fmt_bytes(e.wasted_bytes)).style(if is_sel { sel() } else { warn() }),
                    Cell::from(fmt_bytes(e.unboxed_size)).style(if is_sel { sel() } else { dim() }),
                ]).height(1).style(rs)
            }).collect();
            let title = format!("Boxed Primitives — {} groups  ({} overhead vs primitives)", w.boxed_primitives.len(), fmt_bytes(w.boxed_primitive_wasted_bytes));
            let table = Table::new(rows, [Constraint::Length(5), Constraint::Min(26), Constraint::Length(11), Constraint::Length(11), Constraint::Length(11)])
                .header(header_row(["#", "Class", "Count", "Overhead", "If Unboxed"]))
                .block(panel(&title)).column_spacing(1);
            f.render_widget(table, area);
            scroll_info(f, area, app.waste_sel + 1, w.boxed_primitives.len());
        }
        _ => {}
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
        .constraints([Constraint::Length(3), Constraint::Min(0), Constraint::Length(5)])
        .split(area);

    let crumb = format!("  {}  ({} children at this level)", app.dom_breadcrumb(), app.dom_children.len());
    f.render_widget(
        Paragraph::new(Span::styled(crumb, teal())).block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(BORDER)).style(Style::default().bg(PANEL))),
        chunks[0],
    );

    let total = app.state.get_summary().total_heap_size.max(1);
    let vis = chunks[1].height.saturating_sub(5) as usize;

    if app.dom_children.is_empty() {
        f.render_widget(
            Paragraph::new("\n  No children at this level — leaf node.\n  Press Esc or 'o' to go back up.").style(dim()).block(panel("Dominator Tree")),
            chunks[1],
        );
    } else {
        let rows: Vec<Row> = app.dom_children.iter().enumerate().skip(app.dom_scroll).take(vis).map(|(i, obj)| {
            let is_sel = i == app.dom_sel;
            let (pct_str, abnormal) = retained_pct(obj.retained_size, total);
            let rs = if is_sel { sel() } else { sty() };
            let ps = pct_display_style(&pct_str, abnormal, is_sel);
            let name = if obj.class_name.is_empty() { &obj.node_type } else { &obj.class_name };
            let has_ch = app.state.get_children(obj.object_id).map(|c| !c.is_empty()).unwrap_or(false);
            let arrow = if has_ch { "▶" } else { " " };
            Row::new(vec![
                Cell::from(format!("{}", i + 1)).style(dim()),
                Cell::from(arrow).style(if is_sel { sel() } else { acc() }),
                Cell::from(shorten(name, 38)).style(rs),
                Cell::from(shorten(&obj.node_type, 10)).style(dim()),
                Cell::from(if obj.object_id > 0 { format!("0x{:x}", obj.object_id) } else { "—".into() }).style(dim()),
                Cell::from(fmt_bytes(obj.shallow_size)).style(rs),
                Cell::from(fmt_bytes(obj.retained_size)).style(ps),
                Cell::from(pct_str).style(ps),
            ]).height(1).style(rs)
        }).collect();
        let table = Table::new(rows, [Constraint::Length(4), Constraint::Length(2), Constraint::Min(24), Constraint::Length(10), Constraint::Length(14), Constraint::Length(11), Constraint::Length(11), Constraint::Length(8)])
            .header(header_row(["#", "", "Class", "Type", "Object ID", "Shallow", "Retained", "% Heap"]))
            .block(panel("Dominator Tree  [▶=children]  Enter/i=drill-in  Esc/o=up  x=inspect"))
            .column_spacing(1);
        f.render_widget(table, chunks[1]);
        scroll_info(f, chunks[1], app.dom_sel + 1, app.dom_children.len());
    }

    let help_lines = vec![
        Line::from(vec![Span::styled("  Enter / i", acc()), Span::styled("  Drill into selected object", dim())]),
        Line::from(vec![Span::styled("  Esc / o  ", acc()), Span::styled("  Go back to parent level", dim())]),
        Line::from(vec![Span::styled("  x        ", acc()), Span::styled("  Open selected object in Inspector tab", dim())]),
        Line::from(vec![Span::styled("  Depth: ", dim()), Span::styled(format!("{}", app.dom_stack.len()), teal()), Span::styled("  — retained = shallow + exclusively-dominated children", dim())]),
    ];
    f.render_widget(Paragraph::new(help_lines).block(panel("Navigation Help")).wrap(Wrap { trim: false }), chunks[2]);
}

// ── TAB 7: HeapQL ─────────────────────────────────────────────────────────────
fn tab_query(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0), Constraint::Length(1)])
        .split(area);

    // Input area
    let editing = app.query.mode == InputMode::Editing;
    let input_block = if editing { panel_focused("HeapQL Query  [Enter=run  Esc=cancel  ↑↓=history  Ctrl-U=clear]") }
    else { panel("HeapQL Query  [e or Enter to edit  •  Ctrl-U to clear]") };

    let input_style = if editing { Style::default().fg(TEXT).bg(EDIT_BG) } else { dim() };
    let (before, cursor_char, after) = split_at_cursor(&app.query.input, app.query.cursor);
    let input_line = if editing {
        Line::from(vec![
            Span::raw("  "),
            Span::styled(before, input_style),
            Span::styled(cursor_char, Style::default().fg(BG).bg(ACCENT)),
            Span::styled(after, input_style),
        ])
    } else {
        Line::from(vec![Span::raw("  "), Span::styled(&app.query.input, dim())])
    };

    // Hint lines
    let hints = vec![
        Line::from(vec![
            Span::styled("  Examples: ", dim()),
            Span::styled("SELECT * FROM class_histogram LIMIT 20  │  SELECT * FROM instances WHERE class_name LIKE '%Cache%'  │  :info 0x1234", acc()),
        ]),
        input_line,
    ];
    f.render_widget(Paragraph::new(hints).block(input_block).wrap(Wrap { trim: false }), chunks[0]);

    // Results area
    if let Some(ref err) = app.query.error {
        let err_lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled("  ✗ Error: ", danger().add_modifier(Modifier::BOLD)), Span::styled(err, warn())]),
            Line::from(""),
            Line::from(vec![
                Span::styled("  Available tables: ", dim()),
                Span::styled("instances  class_histogram  dominator_tree  leak_suspects", acc()),
            ]),
            Line::from(vec![
                Span::styled("  Special commands: ", dim()),
                Span::styled(":path <id>  :refs <id>  :children <id>  :info <id>", acc()),
            ]),
        ];
        f.render_widget(Paragraph::new(err_lines).block(panel("Results")).wrap(Wrap { trim: false }), chunks[1]);
    } else if app.query.columns.is_empty() {
        // Initial help
        let help_lines = vec![
            Line::from(""),
            Line::from(vec![Span::styled("  Supported tables", teal().add_modifier(Modifier::BOLD))]),
            Line::from(vec![Span::styled("  instances        ", acc()), Span::styled("object_id, node_type, class_name, shallow_size, retained_size", dim())]),
            Line::from(vec![Span::styled("  class_histogram  ", acc()), Span::styled("class_name, instance_count, shallow_size, retained_size", dim())]),
            Line::from(vec![Span::styled("  dominator_tree   ", acc()), Span::styled("object_id, node_type, class_name, shallow_size, retained_size  [WHERE object_id = X]", dim())]),
            Line::from(vec![Span::styled("  leak_suspects    ", acc()), Span::styled("class_name, object_id, retained_size, retained_percentage, description", dim())]),
            Line::from(""),
            Line::from(vec![Span::styled("  Special commands", teal().add_modifier(Modifier::BOLD))]),
            Line::from(vec![Span::styled("  :path <id>       ", acc()), Span::styled("GC root path to object", dim())]),
            Line::from(vec![Span::styled("  :refs <id>       ", acc()), Span::styled("All objects referencing this object", dim())]),
            Line::from(vec![Span::styled("  :children <id>   ", acc()), Span::styled("Dominator tree children of object", dim())]),
            Line::from(vec![Span::styled("  :info <id>       ", acc()), Span::styled("Object metadata", dim())]),
            Line::from(""),
            Line::from(vec![Span::styled("  Clauses: ", dim()), Span::styled("WHERE  ORDER BY  LIMIT  GROUP BY  JOIN  IN (SELECT ...)  COUNT/SUM/AVG/MIN/MAX", acc())]),
            Line::from(vec![Span::styled("  Sizes:   ", dim()), Span::styled("WHERE shallow_size > 1MB  (supports B/KB/MB/GB suffixes)", acc())]),
        ];
        f.render_widget(Paragraph::new(help_lines).block(panel("Results — press e or Enter to start a query")).wrap(Wrap { trim: false }), chunks[1]);
    } else {
        // Render result table
        let vis = chunks[1].height.saturating_sub(5) as usize;
        let ncols = app.query.columns.len();
        let col_w = if ncols == 0 { 20 } else { ((chunks[1].width.saturating_sub(4)) / ncols as u16).max(10) as usize };

        let rows: Vec<Row> = app.query.rows.iter().enumerate().skip(app.query.scroll).take(vis).map(|(i, row)| {
            let is_sel = i == app.query.sel;
            let rs = if is_sel { sel() } else { sty() };
            let cells: Vec<Cell> = row.iter().map(|v| Cell::from(shorten(v, col_w)).style(rs)).collect();
            Row::new(cells).height(1).style(rs)
        }).collect();

        let widths: Vec<Constraint> = (0..ncols).map(|_| Constraint::Min(col_w as u16)).collect();
        let header_cells: Vec<Cell> = app.query.columns.iter().map(|c| Cell::from(c.as_str()).style(hdr())).collect();
        let header = Row::new(header_cells).height(1).style(Style::default().bg(PANEL));

        let total_rows = app.query.rows.len();
        let title = format!("Results — {}  ({}/{})", app.query.stats, app.query.sel.min(total_rows.saturating_sub(1)) + 1, total_rows);
        let table = Table::new(rows, widths).header(header).block(panel(&title)).column_spacing(1);
        f.render_widget(table, chunks[1]);
        scroll_info(f, chunks[1], app.query.sel + 1, app.query.rows.len());
    }

    // Bottom hint
    let bottom = Line::from(vec![
        Span::styled(" Tables: ", dim()),
        Span::styled("instances  class_histogram  dominator_tree  leak_suspects", acc()),
        Span::styled("  |  Cmds: ", dim()),
        Span::styled(":path :refs :children :info", acc()),
    ]);
    f.render_widget(Paragraph::new(bottom).style(Style::default().bg(PANEL)), chunks[2]);
}

// ── TAB 8: Inspector ─────────────────────────────────────────────────────────
fn tab_inspector(f: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(5), Constraint::Min(0)])
        .split(area);

    // Header: object ID input + summary
    let editing = app.inspector.mode == InputMode::Editing;
    let input_block = if editing {
        panel_focused("Object ID  [Enter=load  Esc=cancel]  (decimal or 0x hex)")
    } else {
        panel("Object Inspector  [e=edit ID  p=cycle panels  x from Dom Tree]")
    };

    let (before, cursor_char, after) = split_at_cursor(&app.inspector.input, app.inspector.cursor);
    let id_span = if editing {
        Line::from(vec![
            Span::styled("  Object ID: ", dim()),
            Span::styled(before, Style::default().fg(TEXT).bg(EDIT_BG)),
            Span::styled(cursor_char, Style::default().fg(BG).bg(ACCENT)),
            Span::styled(after, Style::default().fg(TEXT).bg(EDIT_BG)),
        ])
    } else {
        Line::from(vec![
            Span::styled("  Object ID: ", dim()),
            Span::styled(if app.inspector.input.is_empty() { "<none — press e to enter an ID>" } else { &app.inspector.input }, if app.inspector.input.is_empty() { dim() } else { acc() }),
        ])
    };

    let summary_line = if let Some(ref obj) = app.inspector.current {
        Line::from(vec![
            Span::styled("  ", dim()),
            Span::styled(format!("{} ", obj.node_type), dim()),
            Span::styled(shorten(&obj.class_name, 50), teal().add_modifier(Modifier::BOLD)),
            Span::styled("  shallow ", dim()), Span::styled(fmt_bytes(obj.shallow_size), acc()),
            Span::styled("  retained ", dim()), Span::styled(fmt_bytes(obj.retained_size), acc()),
            Span::styled(format!("  ID 0x{:x}", obj.object_id), dim()),
        ])
    } else if let Some(ref err) = app.inspector.error {
        Line::from(Span::styled(format!("  ✗ {}", err), danger()))
    } else {
        Line::from(Span::styled("  Enter an object ID above to inspect it.", dim()))
    };

    f.render_widget(Paragraph::new(vec![id_span, Line::from(""), summary_line]).block(input_block).wrap(Wrap { trim: false }), chunks[0]);

    if app.inspector.current.is_none() {
        f.render_widget(
            Paragraph::new(vec![
                Line::from(""),
                Line::from(Span::styled("  No object loaded.", dim())),
                Line::from(""),
                Line::from(vec![Span::styled("  You can: ", dim()), Span::styled("• press e to type an object ID  •  press x on the Dominator Tree tab to inspect a node", acc())]),
            ]).block(panel("Fields / Referrers / GC Path")).wrap(Wrap { trim: false }),
            chunks[1],
        );
        return;
    }

    // Three panels: Fields | Referrers | GC Path
    let panels = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(30), Constraint::Percentage(30)])
        .split(chunks[1]);

    draw_fields_panel(f, app, panels[0]);
    draw_referrers_panel(f, app, panels[1]);
    draw_gcpath_panel(f, app, panels[2]);
}

fn draw_fields_panel(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.inspector.focus == InspectorFocus::Fields;
    let block = if focused { panel_focused("Fields  [p=next panel  ↑↓=scroll]") } else { panel("Fields") };
    let fields = &app.inspector.fields;
    if fields.is_empty() {
        let msg = if app.inspector.current.as_ref().map(|o| o.node_type.as_str()) == Some("Instance") {
            "  No field data available (no HPROF bytes loaded)."
        } else {
            "  Field inspection is only available for Instance nodes."
        };
        f.render_widget(Paragraph::new(msg).style(dim()).block(block), area);
        return;
    }

    let vis = area.height.saturating_sub(5) as usize;
    let rows: Vec<Row> = fields.iter().enumerate().skip(app.inspector.field_scroll).take(vis).map(|(i, fi)| {
        let is_sel = focused && i == app.inspector.field_sel;
        let rs = if is_sel { sel() } else { sty() };
        let value = fi.primitive_value.as_deref()
            .unwrap_or_else(|| fi.ref_object_id.map(|_| "<ref>").unwrap_or("null"));
        let ref_info = fi.ref_object_id.map(|id| {
            fi.ref_summary.as_ref().map(|s| format!("0x{:x} {}", id, shorten(&s.class_name, 20)))
                .unwrap_or_else(|| format!("0x{:x}", id))
        }).unwrap_or_default();
        Row::new(vec![
            Cell::from(shorten(&fi.name, 22)).style(if is_sel { sel() } else { acc() }),
            Cell::from(fi.field_type.as_str()).style(dim()),
            Cell::from(if ref_info.is_empty() { value.to_string() } else { ref_info }).style(rs),
        ]).height(1).style(rs)
    }).collect();

    let title = format!("Fields  ({} total)", fields.len());
    let table = Table::new(rows, [Constraint::Min(16), Constraint::Length(9), Constraint::Min(16)])
        .header(header_row(["Field", "Type", "Value / Ref"]))
        .block(if focused { panel_focused(&title) } else { panel(&title) })
        .column_spacing(1);
    f.render_widget(table, area);
    scroll_info(f, area, app.inspector.field_sel + 1, fields.len());
}

fn draw_referrers_panel(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.inspector.focus == InspectorFocus::Referrers;
    let refs = &app.inspector.referrers;
    if refs.is_empty() {
        f.render_widget(Paragraph::new("  No referrers found.").style(dim()).block(if focused { panel_focused("Referrers") } else { panel("Referrers") }), area);
        return;
    }

    let vis = area.height.saturating_sub(5) as usize;
    let rows: Vec<Row> = refs.iter().enumerate().skip(app.inspector.ref_scroll).take(vis).map(|(i, obj)| {
        let is_sel = focused && i == app.inspector.ref_sel;
        let rs = if is_sel { sel() } else { sty() };
        Row::new(vec![
            Cell::from(shorten(&obj.class_name, 24)).style(rs),
            Cell::from(if obj.object_id > 0 { format!("0x{:x}", obj.object_id) } else { "—".into() }).style(dim()),
            Cell::from(fmt_bytes(obj.retained_size)).style(if is_sel { sel() } else { acc() }),
        ]).height(1).style(rs)
    }).collect();

    let title = format!("Referrers  ({})  Enter=jump", refs.len());
    let table = Table::new(rows, [Constraint::Min(18), Constraint::Length(12), Constraint::Length(11)])
        .header(header_row(["Class", "Object ID", "Retained"]))
        .block(if focused { panel_focused(&title) } else { panel(&title) })
        .column_spacing(1);
    f.render_widget(table, area);
    scroll_info(f, area, app.inspector.ref_sel + 1, refs.len());
}

fn draw_gcpath_panel(f: &mut Frame, app: &App, area: Rect) {
    let focused = app.inspector.focus == InspectorFocus::GcPath;
    let path = &app.inspector.gc_path;
    if path.is_empty() {
        f.render_widget(
            Paragraph::new("  No GC root path found.\n  (Phase-1-only mode has no path data)").style(dim())
                .block(if focused { panel_focused("GC Root Path") } else { panel("GC Root Path") }),
            area,
        );
        return;
    }

    let vis = area.height.saturating_sub(5) as usize;
    let rows: Vec<Row> = path.iter().enumerate().skip(app.inspector.gc_scroll).take(vis).map(|(i, obj)| {
        let is_sel = focused && i == app.inspector.gc_sel;
        let rs = if is_sel { sel() } else { sty() };
        let is_root = obj.node_type == "Root" || obj.node_type == "SuperRoot";
        let is_target = i == path.len().saturating_sub(1);
        let marker = if is_root { "⚓" } else if is_target { "◉" } else { "│" };
        Row::new(vec![
            Cell::from(format!("{} {}", marker, shorten(&obj.class_name, 22))).style(if is_root { ok() } else if is_target { teal().add_modifier(Modifier::BOLD) } else { rs }),
            Cell::from(if obj.object_id > 0 { format!("0x{:x}", obj.object_id) } else { "—".into() }).style(dim()),
        ]).height(1).style(rs)
    }).collect();

    let title = format!("GC Path  (depth {})  Enter=jump", path.len());
    let table = Table::new(rows, [Constraint::Min(24), Constraint::Length(12)])
        .header(header_row(["Class", "Object ID"]))
        .block(if focused { panel_focused(&title) } else { panel(&title) })
        .column_spacing(1);
    f.render_widget(table, area);
    scroll_info(f, area, app.inspector.gc_sel + 1, path.len());
}

// ── TAB 9: Help ──────────────────────────────────────────────────────────────
fn tab_help(f: &mut Frame, area: Rect) {
    let lines = vec![
        Line::from(Span::styled("  hprof-tui  v0.4  —  HeapLens engine edition", Style::default().fg(ACCENT).add_modifier(Modifier::BOLD))),
        Line::from(""),
        Line::from(Span::styled("  Navigation", teal().add_modifier(Modifier::BOLD))),
        hr("Tab / → / l", "Next tab"),
        hr("Shift+Tab / ← / h", "Previous tab"),
        hr("1 – 8", "Jump to tab directly (9 = this help, ? also works)"),
        hr("↑↓ / j k", "Scroll one row"),
        hr("PgUp/PgDn / u d", "Scroll 10 rows"),
        hr("g / Home", "Jump to top"),
        hr("q / Ctrl-C", "Quit"),
        Line::from(""),
        Line::from(Span::styled("  Actions", teal().add_modifier(Modifier::BOLD))),
        hr("s  (Histogram/Retained)", "Toggle sort: retained ↔ shallow"),
        hr("s  (Leak Suspects)", "Toggle class-level ↔ object-level suspects"),
        hr("s  (Waste)", "Cycle sub-view: Dup Strings → Empty → Over-alloc → Boxed"),
        hr("Enter / i", "Dom Tree: drill in  │  Query/Inspector: start editing"),
        hr("Enter  (Inspector panels)", "Fields: jump to ref'd object  │  Refs/Path: jump to object"),
        hr("Esc / o", "Dom Tree: go up  │  Query/Inspector: cancel edit"),
        hr("x", "Dom Tree: open selected object in Inspector"),
        hr("p", "Inspector: cycle panel focus (Fields → Referrers → GC Path)"),
        hr("e", "Query / Inspector: start editing input"),
        hr("↑↓ (while editing)", "Query: cycle history"),
        hr("Ctrl-U (while editing)", "Query / Inspector: clear input"),
        Line::from(""),
        Line::from(Span::styled("  Tabs", teal().add_modifier(Modifier::BOLD))),
        hr("1  Overview", "Heap stats + top-15 retained bar chart"),
        hr("2  Histogram", "All classes: shallow + retained, sortable"),
        hr("3  Retained", "Classes ranked by retained + overhead ratio"),
        hr("4  Leak Suspects", "Classes/objects retaining >10% heap"),
        hr("5  Waste", "Dup strings, empty/over-alloc collections, boxed primitives"),
        hr("6  Dom Tree", "Interactive dominator tree drill-down"),
        hr("7  HeapQL", "SQL-like queries: SELECT, WHERE, ORDER BY, JOIN, aggregates"),
        hr("8  Inspector", "Object fields, referrers, and GC root path"),
        Line::from(""),
        Line::from(Span::styled("  HeapQL Examples", teal().add_modifier(Modifier::BOLD))),
        hr("SELECT * FROM class_histogram LIMIT 20", "Top 20 classes by retained"),
        hr("SELECT * FROM instances WHERE class_name LIKE '%Cache%'", "Filter instances"),
        hr("SELECT * FROM instances WHERE shallow_size > 1MB", "Large objects"),
        hr(":path 0x1234abcd", "GC root path to object"),
        hr(":refs 0x1234abcd", "Objects referencing this one"),
        hr(":info 0x1234abcd", "Object metadata"),
    ];
    f.render_widget(Paragraph::new(lines).block(panel("Help")).wrap(Wrap { trim: false }), area);
}

// ── Shared widgets ────────────────────────────────────────────────────────────

fn no_dom_msg(title: &str) -> Paragraph<'static> {
    Paragraph::new(vec![
        Line::from(""),
        Line::from(Span::styled("  Dominator tree not available.", warn())),
        Line::from(Span::styled("  Re-run without --phase1-only to enable retained sizes and dominator tree.", dim())),
    ]).block(Block::default().borders(Borders::ALL).border_type(BorderType::Rounded).border_style(Style::default().fg(BORDER)).style(Style::default().bg(PANEL)).title(Span::styled(format!(" {} ", title), acc())))
}

fn scroll_info(f: &mut Frame, area: Rect, cur: usize, total: usize) {
    if total == 0 { return; }
    let txt = format!(" {}/{} ", cur, total);
    let x = area.x + 2;
    let y = area.y + area.height.saturating_sub(1);
    if y < area.y + area.height {
        f.render_widget(Paragraph::new(Span::styled(txt, dim())), Rect { x, y, width: 20, height: 1 });
    }
}

/// Split a string at a byte cursor into (before, cursor_char_or_space, after).
fn split_at_cursor(s: &str, cursor: usize) -> (&str, &str, &str) {
    let len = s.len();
    if cursor >= len {
        (s, " ", "")
    } else {
        let char_end = s[cursor..].char_indices().nth(1).map(|(i, _)| cursor + i).unwrap_or(len);
        (&s[..cursor], &s[cursor..char_end], &s[char_end..])
    }
}

// ── Format helpers ────────────────────────────────────────────────────────────

fn retained_pct(retained: u64, heap: u64) -> (String, bool) {
    if heap == 0 { return ("—".into(), false); }
    let pct = retained as f64 / heap as f64 * 100.0;
    if pct > 100.0 { ("⚠>100%".into(), true) } else { (format!("{:.2}%", pct), false) }
}

fn pct_display_style(pct_str: &str, is_abnormal: bool, is_sel: bool) -> Style {
    if is_sel { return sel(); }
    if is_abnormal { return danger().add_modifier(Modifier::RAPID_BLINK); }
    let pct: f64 = pct_str.trim_start_matches('⚠').trim_end_matches('%').parse().unwrap_or(0.0);
    pct_style(pct)
}

pub fn fmt_bytes(b: u64) -> String {
    if b == 0 { return "0 B".into(); }
    const U: &[&str] = &["B", "KB", "MB", "GB", "TB"];
    let i = ((b as f64).log(1024.0).floor() as usize).min(U.len() - 1);
    let v = b as f64 / 1024f64.powi(i as i32);
    if i > 1 { format!("{:.2} {}", v, U[i]) } else { format!("{:.0} {}", v, U[i]) }
}

fn fmt_n(n: u64) -> String {
    let s = n.to_string();
    let mut r = String::new();
    for (j, c) in s.chars().rev().enumerate() {
        if j > 0 && j % 3 == 0 { r.push(','); }
        r.push(c);
    }
    r.chars().rev().collect()
}

fn pct_style(pct: f64) -> Style {
    if pct > 10.0 { danger() } else if pct > 3.0 { warn() } else if pct > 0.5 { acc() } else { dim() }
}

fn severity_label(pct: f64) -> (&'static str, Color) {
    if pct >= 30.0 { ("HIGH", DANGER) } else if pct >= 15.0 { ("MED ", WARN) } else { ("LOW ", OK) }
}

fn lkv(k: &str, v: String) -> ListItem<'static> {
    ListItem::new(Line::from(vec![Span::styled(format!("  {:<18}", k), dim()), Span::styled(v, sty())]))
}

fn badge(label: &str, col: Color, count: usize, desc: &str) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::styled(format!("  {} ", label), Style::default().fg(col).add_modifier(Modifier::BOLD)),
        Span::styled(count.to_string(), sty()),
        Span::styled(format!("  {}", desc), dim()),
    ]))
}

fn waste_li(label: &str, bytes: u64, groups: usize) -> ListItem<'static> {
    ListItem::new(Line::from(vec![
        Span::styled(format!("  {:<32}", label), dim()),
        Span::styled(format!("{:>11}", fmt_bytes(bytes)), warn()),
        if groups > 0 { Span::styled(format!("  ({} groups)", groups), dim()) } else { Span::raw("") },
    ]))
}

fn hr(key: &str, desc: &str) -> Line<'static> {
    Line::from(vec![Span::styled(format!("  {:<40}", key), acc()), Span::styled(desc.to_string(), sty())])
}

fn retention_tip(class: &str) -> String {
    let c = class.to_lowercase();
    if c.contains("finalizer") { "Finalizer dominates all finalizable objects. Close JDBC/IO resources explicitly.".into() }
    else if c.contains("string") { "Use String.intern() or byte[]; audit caches and ThreadLocals.".into() }
    else if c.contains("statement") || c.contains("jdbc") { "Close PreparedStatement in try-with-resources; limit statement cache.".into() }
    else if c.contains("hashmap") || c.contains("map") { "Bound map sizes; use WeakHashMap or Caffeine with eviction.".into() }
    else if c.contains("list") || c.contains("array") { "Enable pagination/virtualization; trim after bulk ops.".into() }
    else if c.contains("thread") { "Check ThreadLocals and pool sizes; call ThreadLocal.remove().".into() }
    else if c.contains("byte[") || c.contains("char[") { "Large byte/char arrays may be I/O buffers — flush and close streams.".into() }
    else if c.contains("session") { "Shorten session TTL; call session.invalidate() on logout.".into() }
    else if c.contains("cache") { "Set max size and TTL; use SoftReference or Caffeine.".into() }
    else { format!("Review static fields and long-lived caches holding {}.", class) }
}
