//! Application state — wraps HeapLens `IndexedAnalysisState`.

use anyhow::Result;
use hprof_analyzer::{
    indexed::{
        analysis::{IndexedAnalysisState, Phase1AnalysisState},
        parse::{parse_indexed_phase1, parse_indexed_phase2},
        types::HeapAnalysis,
    },
    FieldInfo, HprofLoader, ObjectReport,
};
use std::sync::Arc;

// ── Tabs ──────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Overview,
    Histogram,
    Retained,
    LeakSuspects,
    Waste,
    DomTree,
    Query,
    Inspector,
    Help,
}

impl Tab {
    pub const ALL: &'static [Tab] = &[
        Tab::Overview,
        Tab::Histogram,
        Tab::Retained,
        Tab::LeakSuspects,
        Tab::Waste,
        Tab::DomTree,
        Tab::Query,
        Tab::Inspector,
        Tab::Help,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Histogram => "Histogram",
            Tab::Retained => "Retained",
            Tab::LeakSuspects => "Leak Suspects",
            Tab::Waste => "Waste",
            Tab::DomTree => "Dom Tree",
            Tab::Query => "HeapQL",
            Tab::Inspector => "Inspector",
            Tab::Help => "Help",
        }
    }
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

// ── Input mode (shared by Query and Inspector) ────────────────────────────────

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    Editing,
}

// ── Query tab state ───────────────────────────────────────────────────────────

pub struct QueryState {
    pub input: String,
    pub mode: InputMode,
    pub columns: Vec<String>,
    pub rows: Vec<Vec<String>>,
    pub error: Option<String>,
    pub sel: usize,
    pub scroll: usize,
    pub stats: String,
    pub cursor: usize,
    pub history: Vec<String>,
    pub history_idx: Option<usize>,
}

impl QueryState {
    pub fn new() -> Self {
        let default = "SELECT * FROM class_histogram LIMIT 20".to_string();
        let len = default.len();
        Self {
            input: default,
            mode: InputMode::Normal,
            columns: vec![],
            rows: vec![],
            error: None,
            sel: 0,
            scroll: 0,
            stats: String::new(),
            cursor: len,
            history: vec![],
            history_idx: None,
        }
    }
}

// ── Inspector tab state ───────────────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct InspectorEntry {
    pub object_id: u64,
    pub class_name: String,
    pub node_type: String,
    pub shallow_size: u64,
    pub retained_size: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum InspectorFocus {
    Input,
    Fields,
    Referrers,
    GcPath,
}

pub struct InspectorState {
    pub input: String,
    pub mode: InputMode,
    pub cursor: usize,
    pub current: Option<InspectorEntry>,
    pub fields: Vec<FieldInfo>,
    pub field_sel: usize,
    pub field_scroll: usize,
    pub referrers: Vec<ObjectReport>,
    pub ref_sel: usize,
    pub ref_scroll: usize,
    pub gc_path: Vec<ObjectReport>,
    pub gc_sel: usize,
    pub gc_scroll: usize,
    pub focus: InspectorFocus,
    pub error: Option<String>,
    pub hprof_bytes: Arc<Vec<u8>>,
}

impl InspectorState {
    pub fn new(hprof_bytes: Arc<Vec<u8>>) -> Self {
        Self {
            input: String::new(),
            mode: InputMode::Normal,
            cursor: 0,
            current: None,
            fields: vec![],
            field_sel: 0,
            field_scroll: 0,
            referrers: vec![],
            ref_sel: 0,
            ref_scroll: 0,
            gc_path: vec![],
            gc_sel: 0,
            gc_scroll: 0,
            focus: InspectorFocus::Input,
            error: None,
            hprof_bytes,
        }
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub state: Arc<dyn HeapAnalysis>,
    pub path: String,
    pub has_dominators: bool,
    pub active_tab: Tab,

    pub hist_sel: usize,
    pub hist_scroll: usize,
    pub hist_sort_retained: bool,

    pub ret_sel: usize,
    pub ret_scroll: usize,

    pub leak_sel: usize,
    pub leak_scroll: usize,

    pub waste_sel: usize,
    pub waste_scroll: usize,
    /// Which waste sub-table is shown: 0=dup strings 1=empty colls 2=over-alloc 3=boxed
    pub waste_sub: usize,

    /// false = class-level suspects (default), true = individual object suspects
    pub leak_show_objects: bool,
    pub obj_leak_sel: usize,
    pub obj_leak_scroll: usize,

    pub dom_sel: usize,
    pub dom_scroll: usize,
    pub dom_children: Vec<ObjectReport>,
    pub dom_stack: Vec<(u64, String, usize)>,

    pub query: QueryState,
    pub inspector: InspectorState,

    pub status: Option<String>,
}

impl App {
    pub fn new(path: &str, phase1_only: bool) -> Result<Self> {
        let loader = HprofLoader::new(path.into());
        let mmap = loader.map_file()?;
        let hprof_bytes = Arc::new(mmap[..].to_vec());

        let state: Arc<dyn HeapAnalysis> = if phase1_only {
            eprintln!("Phase 1 only (no dominator tree)…");
            let (p1, _) = parse_indexed_phase1(&mmap[..])?;
            Arc::new(Phase1AnalysisState::new(p1))
        } else {
            eprintln!("Phase 1: parsing nodes…");
            let (p1, deferred) = parse_indexed_phase1(&mmap[..])?;
            eprintln!("Phase 2: edges + Lengauer-Tarjan dominators…");
            let p2 = parse_indexed_phase2(&p1, deferred)?;
            Arc::new(IndexedAnalysisState::from_phases(p1, p2)?)
        };

        let dom_children = state.get_children(0).unwrap_or_default();

        Ok(App {
            state,
            path: path.to_string(),
            has_dominators: !phase1_only,
            active_tab: Tab::Overview,
            hist_sel: 0,
            hist_scroll: 0,
            hist_sort_retained: true,
            ret_sel: 0,
            ret_scroll: 0,
            leak_sel: 0,
            leak_scroll: 0,
            waste_sel: 0,
            waste_scroll: 0,
            waste_sub: 0,
            leak_show_objects: false,
            obj_leak_sel: 0,
            obj_leak_scroll: 0,
            dom_sel: 0,
            dom_scroll: 0,
            dom_children,
            dom_stack: vec![],
            query: QueryState::new(),
            inspector: InspectorState::new(hprof_bytes),
            status: None,
        })
    }

    // ── Tab navigation ────────────────────────────────────────────────────────
    pub fn next_tab(&mut self) {
        let i = self.active_tab.index();
        self.active_tab = Tab::ALL[(i + 1) % Tab::ALL.len()];
    }
    pub fn prev_tab(&mut self) {
        let i = self.active_tab.index();
        self.active_tab = Tab::ALL[(i + Tab::ALL.len() - 1) % Tab::ALL.len()];
    }

    // ── Generic scroll ────────────────────────────────────────────────────────
    pub fn scroll_down(&mut self) {
        match self.active_tab {
            Tab::Histogram => {
                let n = self.state.get_class_histogram().len();
                sel_dn(&mut self.hist_sel, &mut self.hist_scroll, n);
            }
            Tab::Retained => {
                let n = self.state.get_class_histogram().len();
                sel_dn(&mut self.ret_sel, &mut self.ret_scroll, n);
            }
            Tab::LeakSuspects if self.leak_show_objects => {
                let n = self.state.get_object_leak_suspects().len();
                sel_dn(&mut self.obj_leak_sel, &mut self.obj_leak_scroll, n);
            }
            Tab::LeakSuspects => {
                let n = self.state.get_leak_suspects().len();
                sel_dn(&mut self.leak_sel, &mut self.leak_scroll, n);
            }
            Tab::Waste => {
                let n = self.waste_sub_len();
                sel_dn(&mut self.waste_sel, &mut self.waste_scroll, n);
            }
            Tab::DomTree => {
                let n = self.dom_children.len();
                sel_dn(&mut self.dom_sel, &mut self.dom_scroll, n);
            }
            Tab::Query => {
                let n = self.query.rows.len();
                sel_dn(&mut self.query.sel, &mut self.query.scroll, n);
            }
            Tab::Inspector => self.insp_scroll_dn(),
            _ => {}
        }
    }

    pub fn scroll_up(&mut self) {
        match self.active_tab {
            Tab::Histogram => sel_up(&mut self.hist_sel, &mut self.hist_scroll),
            Tab::Retained => sel_up(&mut self.ret_sel, &mut self.ret_scroll),
            Tab::LeakSuspects if self.leak_show_objects => sel_up(&mut self.obj_leak_sel, &mut self.obj_leak_scroll),
            Tab::LeakSuspects => sel_up(&mut self.leak_sel, &mut self.leak_scroll),
            Tab::Waste => sel_up(&mut self.waste_sel, &mut self.waste_scroll),
            Tab::DomTree => sel_up(&mut self.dom_sel, &mut self.dom_scroll),
            Tab::Query => sel_up(&mut self.query.sel, &mut self.query.scroll),
            Tab::Inspector => self.insp_scroll_up(),
            _ => {}
        }
    }

    pub fn page_down(&mut self) {
        for _ in 0..15 { self.scroll_down(); }
    }
    pub fn page_up(&mut self) {
        for _ in 0..15 { self.scroll_up(); }
    }

    pub fn go_top(&mut self) {
        match self.active_tab {
            Tab::Histogram => { self.hist_sel = 0; self.hist_scroll = 0; }
            Tab::Retained => { self.ret_sel = 0; self.ret_scroll = 0; }
            Tab::LeakSuspects if self.leak_show_objects => { self.obj_leak_sel = 0; self.obj_leak_scroll = 0; }
            Tab::LeakSuspects => { self.leak_sel = 0; self.leak_scroll = 0; }
            Tab::Waste => { self.waste_sel = 0; self.waste_scroll = 0; }
            Tab::DomTree => { self.dom_sel = 0; self.dom_scroll = 0; }
            Tab::Query => { self.query.sel = 0; self.query.scroll = 0; }
            Tab::Inspector => match self.inspector.focus {
                InspectorFocus::Fields => { self.inspector.field_sel = 0; self.inspector.field_scroll = 0; }
                InspectorFocus::Referrers => { self.inspector.ref_sel = 0; self.inspector.ref_scroll = 0; }
                InspectorFocus::GcPath => { self.inspector.gc_sel = 0; self.inspector.gc_scroll = 0; }
                _ => {}
            },
            _ => {}
        }
    }

    fn insp_scroll_dn(&mut self) {
        match self.inspector.focus {
            InspectorFocus::Fields => {
                let n = self.inspector.fields.len();
                sel_dn(&mut self.inspector.field_sel, &mut self.inspector.field_scroll, n);
            }
            InspectorFocus::Referrers => {
                let n = self.inspector.referrers.len();
                sel_dn(&mut self.inspector.ref_sel, &mut self.inspector.ref_scroll, n);
            }
            InspectorFocus::GcPath => {
                let n = self.inspector.gc_path.len();
                sel_dn(&mut self.inspector.gc_sel, &mut self.inspector.gc_scroll, n);
            }
            _ => {}
        }
    }

    fn insp_scroll_up(&mut self) {
        match self.inspector.focus {
            InspectorFocus::Fields => sel_up(&mut self.inspector.field_sel, &mut self.inspector.field_scroll),
            InspectorFocus::Referrers => sel_up(&mut self.inspector.ref_sel, &mut self.inspector.ref_scroll),
            InspectorFocus::GcPath => sel_up(&mut self.inspector.gc_sel, &mut self.inspector.gc_scroll),
            _ => {}
        }
    }

    // ── Inspector: cycle panel focus ─────────────────────────────────────────
    pub fn inspector_cycle_panel(&mut self) {
        self.inspector.focus = match self.inspector.focus {
            InspectorFocus::Input => InspectorFocus::Fields,
            InspectorFocus::Fields => InspectorFocus::Referrers,
            InspectorFocus::Referrers => InspectorFocus::GcPath,
            InspectorFocus::GcPath => InspectorFocus::Fields,
        };
    }

    // ── Inspector: load an object by ID ──────────────────────────────────────
    pub fn inspector_load(&mut self) {
        self.inspector.error = None;
        let raw = self.inspector.input.trim().to_string();
        let id = if raw.starts_with("0x") || raw.starts_with("0X") {
            u64::from_str_radix(&raw[2..], 16).ok()
        } else {
            raw.parse::<u64>().ok()
        };
        let Some(oid) = id else {
            self.inspector.error = Some(format!("'{}' is not a valid object ID (decimal or 0x hex)", raw));
            return;
        };

        let Some((report, _, _)) = self.state.get_object_info(oid) else {
            self.inspector.error = Some(format!("Object 0x{:x} not found in heap", oid));
            return;
        };

        self.inspector.current = Some(InspectorEntry {
            object_id: report.object_id,
            class_name: report.class_name.clone(),
            node_type: report.node_type.clone(),
            shallow_size: report.shallow_size,
            retained_size: report.retained_size,
        });

        let bytes = Arc::clone(&self.inspector.hprof_bytes);
        self.inspector.fields = self.state.inspect_object_bytes(&bytes, oid).unwrap_or_default();
        self.inspector.referrers = self.state.get_referrers(oid).unwrap_or_default();
        self.inspector.gc_path = self.state.gc_root_path(oid, 200).unwrap_or_default();

        self.inspector.field_sel = 0; self.inspector.field_scroll = 0;
        self.inspector.ref_sel = 0;   self.inspector.ref_scroll = 0;
        self.inspector.gc_sel = 0;    self.inspector.gc_scroll = 0;
        self.inspector.focus = InspectorFocus::Fields;

        self.status = Some(format!(
            "Inspecting {} 0x{:x}  —  {} fields · {} referrers · GC path depth {}",
            report.class_name, oid,
            self.inspector.fields.len(),
            self.inspector.referrers.len(),
            self.inspector.gc_path.len(),
        ));
    }

    /// Jump to Inspector tab pre-loaded with the selected dominator tree node.
    pub fn inspect_dom_selection(&mut self) {
        let Some(child) = self.dom_children.get(self.dom_sel) else { return; };
        let oid = child.object_id;
        self.inspector.input = format!("0x{:x}", oid);
        self.inspector.cursor = self.inspector.input.len();
        self.inspector.mode = InputMode::Normal;
        self.active_tab = Tab::Inspector;
        self.inspector_load();
    }

    // ── Dominator tree ────────────────────────────────────────────────────────
    pub fn dom_drill_in(&mut self) {
        let Some(child) = self.dom_children.get(self.dom_sel) else { return; };
        let oid = child.object_id;
        let label = if child.class_name.is_empty() { child.node_type.clone() } else { child.class_name.clone() };
        let Some(children) = self.state.get_children(oid) else {
            self.status = Some(format!("'{}' is a leaf — no dominator children", shorten(&label, 40)));
            return;
        };
        self.dom_stack.push((oid, label.clone(), self.dom_sel));
        self.dom_children = children;
        self.dom_sel = 0;
        self.dom_scroll = 0;
        self.status = Some(format!("Drilled into '{}' — {} children", shorten(&label, 40), self.dom_children.len()));
    }

    pub fn dom_drill_out(&mut self) {
        let Some((_, label, saved_sel)) = self.dom_stack.pop() else {
            self.status = Some("Already at root".into());
            return;
        };
        let parent_oid = self.dom_stack.last().map(|(oid, _, _)| *oid).unwrap_or(0);
        self.dom_children = self.state.get_children(parent_oid).unwrap_or_default();
        self.dom_sel = saved_sel;
        self.dom_scroll = saved_sel.saturating_sub(VISIBLE / 2);
        self.status = Some(format!("Back from '{}'", shorten(&label, 40)));
    }

    pub fn dom_breadcrumb(&self) -> String {
        if self.dom_stack.is_empty() {
            "<GC roots>".to_string()
        } else {
            std::iter::once("<GC roots>".to_string())
                .chain(self.dom_stack.iter().map(|(_, lbl, _)| shorten(lbl, 28)))
                .collect::<Vec<_>>()
                .join(" › ")
        }
    }

    // ── Sort toggle ───────────────────────────────────────────────────────────
    pub fn toggle_hist_sort(&mut self) {
        self.hist_sort_retained = !self.hist_sort_retained;
        self.hist_sel = 0;
        self.hist_scroll = 0;
        self.status = Some(if self.hist_sort_retained { "Sorted by retained size".into() } else { "Sorted by shallow size".into() });
    }

    // ── HeapQL ────────────────────────────────────────────────────────────────
    pub fn query_execute(&mut self) {
        let q = self.query.input.trim().to_string();
        if q.is_empty() { return; }
        if self.query.history.last().map(|s| s.as_str()) != Some(&q) {
            self.query.history.push(q.clone());
        }
        self.query.history_idx = None;
        self.query.sel = 0;
        self.query.scroll = 0;
        self.query.error = None;

        match self.state.execute_query_paged(&q, 1, 500) {
            Ok(result) => {
                self.query.columns = result.columns.clone();
                self.query.rows = result.rows.iter().map(|row| {
                    row.iter().map(|v| match v {
                        serde_json::Value::String(s) => s.clone(),
                        serde_json::Value::Number(n) => n.to_string(),
                        serde_json::Value::Null => "null".into(),
                        serde_json::Value::Bool(b) => b.to_string(),
                        other => other.to_string(),
                    }).collect()
                }).collect();
                self.query.stats = format!(
                    "{} rows matched  ·  scanned {}  ·  {:.1} ms",
                    result.total_matched, result.total_scanned, result.execution_time_ms,
                );
                self.status = Some(format!("HeapQL: {} rows", self.query.rows.len()));
            }
            Err(e) => {
                self.query.columns = vec![];
                self.query.rows = vec![];
                self.query.error = Some(e.to_string());
                self.status = Some(format!("HeapQL error: {}", e));
            }
        }
    }

    pub fn query_history_nav(&mut self, older: bool) {
        let len = self.query.history.len();
        if len == 0 { return; }
        let new_idx = match self.query.history_idx {
            None => if older { Some(len.saturating_sub(1)) } else { None },
            Some(i) => {
                if older {
                    Some(i.saturating_sub(1))
                } else if i + 1 < len {
                    Some(i + 1)
                } else {
                    None
                }
            }
        };
        self.query.history_idx = new_idx;
        if let Some(i) = new_idx {
            self.query.input = self.query.history[i].clone();
            self.query.cursor = self.query.input.len();
        }
    }

    // ── Waste sub-view cycling ────────────────────────────────────────────────
    /// Cycle to the next non-empty waste sub-view; wrap if at end.
    pub fn waste_cycle_sub(&mut self) {
        let w = self.state.get_waste_analysis();
        let counts = [
            w.duplicate_strings.len(),
            w.empty_collections.len(),
            w.over_allocated_collections.len(),
            w.boxed_primitives.len(),
        ];
        let start = self.waste_sub;
        for i in 1..=4 {
            let next = (start + i) % 4;
            // allow showing even if empty (so user knows it was checked)
            self.waste_sub = next;
            break;
        }
        self.waste_sel = 0;
        self.waste_scroll = 0;
        let labels = ["Duplicate Strings", "Empty Collections", "Over-Allocated", "Boxed Primitives"];
        let n = counts[self.waste_sub];
        self.status = Some(format!(
            "Waste: {} — {} group{}",
            labels[self.waste_sub], n, if n == 1 { "" } else { "s" }
        ));
    }

    pub fn waste_sub_len(&self) -> usize {
        let w = self.state.get_waste_analysis();
        match self.waste_sub {
            0 => w.duplicate_strings.len(),
            1 => w.empty_collections.len(),
            2 => w.over_allocated_collections.len(),
            3 => w.boxed_primitives.len(),
            _ => 0,
        }
    }

    // ── Leak suspects toggle ─────────────────────────────────────────────────
    pub fn toggle_leak_view(&mut self) {
        self.leak_show_objects = !self.leak_show_objects;
        self.obj_leak_sel = 0;
        self.obj_leak_scroll = 0;
        self.status = Some(if self.leak_show_objects {
            "Showing object-level suspects  [s] toggle back to class view".into()
        } else {
            "Showing class-level suspects  [s] toggle to object view".into()
        });
    }

    // ── Helpers ───────────────────────────────────────────────────────────────
    fn waste_total_rows(&self) -> usize {
        let w = self.state.get_waste_analysis();
        w.duplicate_strings.len() + w.empty_collections.len() + w.over_allocated_collections.len()
    }

    pub fn sorted_histogram(&self) -> Vec<hprof_analyzer::ClassHistogramEntry> {
        let mut entries = self.state.get_class_histogram().to_vec();
        if !self.hist_sort_retained {
            entries.sort_by(|a, b| b.shallow_size.cmp(&a.shallow_size));
        }
        entries
    }
}

// ── Scroll primitives ──────────────────────────────────────────────────────────
pub const VISIBLE: usize = 22;

fn sel_dn(sel: &mut usize, scroll: &mut usize, len: usize) {
    if *sel + 1 < len {
        *sel += 1;
        if *sel >= *scroll + VISIBLE {
            *scroll += 1;
        }
    }
}
fn sel_up(sel: &mut usize, scroll: &mut usize) {
    if *sel > 0 {
        *sel -= 1;
        if *sel < *scroll {
            *scroll = *sel;
        }
    }
}

pub fn shorten(s: &str, max: usize) -> String {
    if s.len() <= max { return s.to_string(); }
    if let Some(p) = s.rfind('.') {
        let tail = &s[p + 1..];
        if tail.len() + 2 <= max { return format!("…{}", tail); }
    }
    format!("{}…", &s[..max.saturating_sub(1)])
}
