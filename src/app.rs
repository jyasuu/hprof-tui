//! Application state — wraps HeapLens `IndexedAnalysisState`.

use anyhow::Result;
use hprof_analyzer::{
    indexed::{
        analysis::{IndexedAnalysisState, Phase1AnalysisState},
        parse::{parse_indexed_phase1, parse_indexed_phase2},
        types::HeapAnalysis,
    },
    HprofLoader, ObjectReport,
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
        Tab::Help,
    ];
    pub fn title(self) -> &'static str {
        match self {
            Tab::Overview => "Overview",
            Tab::Histogram => "Histogram",
            Tab::Retained => "Retained",
            Tab::LeakSuspects => "Leak Suspects",
            Tab::Waste => "Waste",
            Tab::DomTree => "Dominator Tree",
            Tab::Help => "Help",
        }
    }
    pub fn index(self) -> usize {
        Self::ALL.iter().position(|t| *t == self).unwrap_or(0)
    }
}

// ── App ───────────────────────────────────────────────────────────────────────

pub struct App {
    pub state: Arc<dyn HeapAnalysis>,
    pub path: String,
    pub has_dominators: bool,
    pub active_tab: Tab,

    // Histogram tab
    pub hist_sel: usize,
    pub hist_scroll: usize,
    pub hist_sort_retained: bool, // true = sort by retained (default), false = shallow

    // Retained tab (same data, different scroll state)
    pub ret_sel: usize,
    pub ret_scroll: usize,

    // Leak suspects tab
    pub leak_sel: usize,
    pub leak_scroll: usize,

    // Waste tab
    pub waste_sel: usize,
    pub waste_scroll: usize,

    // Dominator Tree tab
    pub dom_sel: usize,
    pub dom_scroll: usize,
    /// Current children being shown (changes as user drills in/out)
    pub dom_children: Vec<ObjectReport>,
    /// Breadcrumb stack: (object_id, label, saved_selection)
    pub dom_stack: Vec<(u64, String, usize)>,

    pub status: Option<String>,
}

impl App {
    pub fn new(path: &str, phase1_only: bool) -> Result<Self> {
        let loader = HprofLoader::new(path.into());
        let mmap = loader.map_file()?;

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

        // Root level of dominator tree = children of super-root (object_id 0)
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
            dom_sel: 0,
            dom_scroll: 0,
            dom_children,
            dom_stack: vec![],
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

    // ── Scroll ────────────────────────────────────────────────────────────────
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
            Tab::LeakSuspects => {
                let n = self.state.get_leak_suspects().len();
                sel_dn(&mut self.leak_sel, &mut self.leak_scroll, n);
            }
            Tab::Waste => {
                let n = self.waste_total_rows();
                sel_dn(&mut self.waste_sel, &mut self.waste_scroll, n);
            }
            Tab::DomTree => {
                let n = self.dom_children.len();
                sel_dn(&mut self.dom_sel, &mut self.dom_scroll, n);
            }
            _ => {}
        }
    }
    pub fn scroll_up(&mut self) {
        match self.active_tab {
            Tab::Histogram => sel_up(&mut self.hist_sel, &mut self.hist_scroll),
            Tab::Retained => sel_up(&mut self.ret_sel, &mut self.ret_scroll),
            Tab::LeakSuspects => sel_up(&mut self.leak_sel, &mut self.leak_scroll),
            Tab::Waste => sel_up(&mut self.waste_sel, &mut self.waste_scroll),
            Tab::DomTree => sel_up(&mut self.dom_sel, &mut self.dom_scroll),
            _ => {}
        }
    }
    pub fn page_down(&mut self) {
        for _ in 0..15 {
            self.scroll_down();
        }
    }
    pub fn page_up(&mut self) {
        for _ in 0..15 {
            self.scroll_up();
        }
    }
    pub fn go_top(&mut self) {
        match self.active_tab {
            Tab::Histogram => {
                self.hist_sel = 0;
                self.hist_scroll = 0;
            }
            Tab::Retained => {
                self.ret_sel = 0;
                self.ret_scroll = 0;
            }
            Tab::LeakSuspects => {
                self.leak_sel = 0;
                self.leak_scroll = 0;
            }
            Tab::Waste => {
                self.waste_sel = 0;
                self.waste_scroll = 0;
            }
            Tab::DomTree => {
                self.dom_sel = 0;
                self.dom_scroll = 0;
            }
            _ => {}
        }
    }

    // ── Dominator tree drill-down ─────────────────────────────────────────────
    pub fn dom_drill_in(&mut self) {
        let Some(child) = self.dom_children.get(self.dom_sel) else {
            return;
        };
        let oid = child.object_id;
        let label = if child.class_name.is_empty() {
            child.node_type.clone()
        } else {
            child.class_name.clone()
        };
        let Some(children) = self.state.get_children(oid) else {
            self.status = Some(format!(
                "'{}' is a leaf — no dominator children",
                shorten(&label, 40)
            ));
            return;
        };
        // Save current position on the stack, then dive in
        self.dom_stack.push((oid, label.clone(), self.dom_sel));
        self.dom_children = children;
        self.dom_sel = 0;
        self.dom_scroll = 0;
        self.status = Some(format!(
            "Drilled into '{}' — {} children",
            shorten(&label, 40),
            self.dom_children.len()
        ));
    }

    pub fn dom_drill_out(&mut self) {
        let Some((_, label, saved_sel)) = self.dom_stack.pop() else {
            self.status = Some("Already at root".into());
            return;
        };
        // Reload parent level
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
        self.status = Some(if self.hist_sort_retained {
            "Sorted by retained size".into()
        } else {
            "Sorted by shallow size".into()
        });
    }

    // ── Helpers ───────────────────────────────────────────────────────────────
    fn waste_total_rows(&self) -> usize {
        let w = self.state.get_waste_analysis();
        w.duplicate_strings.len() + w.empty_collections.len() + w.over_allocated_collections.len()
    }

    /// Histogram entries sorted according to current sort mode.
    pub fn sorted_histogram(&self) -> Vec<hprof_analyzer::ClassHistogramEntry> {
        let mut entries = self.state.get_class_histogram().to_vec();
        if !self.hist_sort_retained {
            entries.sort_by(|a, b| b.shallow_size.cmp(&a.shallow_size));
        }
        // Default from engine is retained-sorted already
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
    if s.len() <= max {
        return s.to_string();
    }
    if let Some(p) = s.rfind('.') {
        let tail = &s[p + 1..];
        if tail.len() + 2 <= max {
            return format!("…{}", tail);
        }
    }
    format!("{}…", &s[..max.saturating_sub(1)])
}
