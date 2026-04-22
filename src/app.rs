use crate::parser::{analyze_hprof, HprofAnalysis};
use crate::retained::{RetainedAnalysis, RetainedClassEntry};
use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Overview,
    Histogram,
    RetainedGraph,
    LeakSuspects,
    GcRoots,
    DuplicateStrings,
    Help,
}

impl Tab {
    pub const ALL: &'static [Tab] = &[
        Tab::Overview,
        Tab::Histogram,
        Tab::RetainedGraph,
        Tab::LeakSuspects,
        Tab::GcRoots,
        Tab::DuplicateStrings,
        Tab::Help,
    ];

    pub fn title(&self) -> &str {
        match self {
            Tab::Overview => "Overview",
            Tab::Histogram => "Histogram",
            Tab::RetainedGraph => "Retained/Graph",
            Tab::LeakSuspects => "Leak Suspects",
            Tab::GcRoots => "GC Roots",
            Tab::DuplicateStrings => "Dup Strings",
            Tab::Help => "Help",
        }
    }

    pub fn index(&self) -> usize {
        Self::ALL.iter().position(|t| t == self).unwrap_or(0)
    }
}

pub enum RetainedState {
    NotStarted,
    Computing,
    Done(RetainedAnalysis),
    Error(String),
}

pub struct App {
    pub analysis: HprofAnalysis,
    pub active_tab: Tab,
    pub retained_state: RetainedState,

    // per-tab scroll/selection
    pub histogram_scroll: usize,
    pub histogram_selected: usize,
    pub retained_scroll: usize,
    pub retained_selected: usize,
    pub leak_scroll: usize,
    pub leak_selected: usize,
    pub gc_scroll: usize,
    pub gc_selected: usize,
    pub dup_scroll: usize,
    pub dup_selected: usize,

    pub sort_by_count: bool,
    pub status_message: Option<String>,
    pub hprof_path: String,
}

impl App {
    pub fn new(path: &str) -> Result<Self> {
        let analysis = analyze_hprof(path)?;
        Ok(App {
            analysis,
            active_tab: Tab::Overview,
            retained_state: RetainedState::NotStarted,
            histogram_scroll: 0,
            histogram_selected: 0,
            retained_scroll: 0,
            retained_selected: 0,
            leak_scroll: 0,
            leak_selected: 0,
            gc_scroll: 0,
            gc_selected: 0,
            dup_scroll: 0,
            dup_selected: 0,
            sort_by_count: false,
            status_message: None,
            hprof_path: path.to_string(),
        })
    }

    pub fn next_tab(&mut self) {
        let idx = self.active_tab.index();
        self.active_tab = Tab::ALL[(idx + 1) % Tab::ALL.len()];
    }

    pub fn prev_tab(&mut self) {
        let idx = self.active_tab.index();
        self.active_tab = Tab::ALL[(idx + Tab::ALL.len() - 1) % Tab::ALL.len()];
    }

    pub fn scroll_down(&mut self) {
        match self.active_tab {
            Tab::Histogram => scroll_sel(
                &mut self.histogram_selected,
                &mut self.histogram_scroll,
                self.analysis.class_histogram.len(),
            ),
            Tab::RetainedGraph => {
                let len = retained_len(&self.retained_state);
                scroll_sel(&mut self.retained_selected, &mut self.retained_scroll, len);
            }
            Tab::LeakSuspects => scroll_sel(
                &mut self.leak_selected,
                &mut self.leak_scroll,
                self.analysis.leak_suspects.len(),
            ),
            Tab::GcRoots => scroll_sel(
                &mut self.gc_selected,
                &mut self.gc_scroll,
                self.analysis.gc_roots.len(),
            ),
            Tab::DuplicateStrings => scroll_sel(
                &mut self.dup_selected,
                &mut self.dup_scroll,
                self.analysis.duplicate_strings.len(),
            ),
            _ => {}
        }
    }

    pub fn scroll_up(&mut self) {
        match self.active_tab {
            Tab::Histogram => {
                scroll_up_sel(&mut self.histogram_selected, &mut self.histogram_scroll)
            }
            Tab::RetainedGraph => {
                scroll_up_sel(&mut self.retained_selected, &mut self.retained_scroll)
            }
            Tab::LeakSuspects => scroll_up_sel(&mut self.leak_selected, &mut self.leak_scroll),
            Tab::GcRoots => scroll_up_sel(&mut self.gc_selected, &mut self.gc_scroll),
            Tab::DuplicateStrings => scroll_up_sel(&mut self.dup_selected, &mut self.dup_scroll),
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

    pub fn go_to_top(&mut self) {
        match self.active_tab {
            Tab::Histogram => {
                self.histogram_selected = 0;
                self.histogram_scroll = 0;
            }
            Tab::RetainedGraph => {
                self.retained_selected = 0;
                self.retained_scroll = 0;
            }
            Tab::LeakSuspects => {
                self.leak_selected = 0;
                self.leak_scroll = 0;
            }
            Tab::GcRoots => {
                self.gc_selected = 0;
                self.gc_scroll = 0;
            }
            Tab::DuplicateStrings => {
                self.dup_selected = 0;
                self.dup_scroll = 0;
            }
            _ => {}
        }
    }

    pub fn go_to_bottom(&mut self) {
        match self.active_tab {
            Tab::Histogram => {
                let n = self.analysis.class_histogram.len().saturating_sub(1);
                self.histogram_selected = n;
                self.histogram_scroll = n.saturating_sub(VISIBLE - 1);
            }
            Tab::RetainedGraph => {
                let n = retained_len(&self.retained_state).saturating_sub(1);
                self.retained_selected = n;
                self.retained_scroll = n.saturating_sub(VISIBLE - 1);
            }
            Tab::LeakSuspects => {
                let n = self.analysis.leak_suspects.len().saturating_sub(1);
                self.leak_selected = n;
                self.leak_scroll = n.saturating_sub(VISIBLE - 1);
            }
            Tab::GcRoots => {
                let n = self.analysis.gc_roots.len().saturating_sub(1);
                self.gc_selected = n;
                self.gc_scroll = n.saturating_sub(VISIBLE - 1);
            }
            Tab::DuplicateStrings => {
                let n = self.analysis.duplicate_strings.len().saturating_sub(1);
                self.dup_selected = n;
                self.dup_scroll = n.saturating_sub(VISIBLE - 1);
            }
            _ => {}
        }
    }

    pub fn toggle_sort(&mut self) {
        self.sort_by_count = !self.sort_by_count;
        if self.sort_by_count {
            self.analysis
                .class_histogram
                .sort_by(|a, b| b.instance_count.cmp(&a.instance_count));
            self.status_message = Some("Histogram sorted by instance count".to_string());
        } else {
            self.analysis
                .class_histogram
                .sort_by(|a, b| b.shallow_size.cmp(&a.shallow_size));
            self.status_message = Some("Histogram sorted by shallow size".to_string());
        }
        self.histogram_selected = 0;
        self.histogram_scroll = 0;
    }

    /// Returns the currently selected RetainedClassEntry if available
    pub fn selected_retained(&self) -> Option<&RetainedClassEntry> {
        if let RetainedState::Done(ref ra) = self.retained_state {
            ra.entries.get(self.retained_selected)
        } else {
            None
        }
    }
}

// ── Helpers ───────────────────────────────────────────────────────────────────

const VISIBLE: usize = 20;

fn scroll_sel(selected: &mut usize, scroll: &mut usize, len: usize) {
    let max = len.saturating_sub(1);
    if *selected < max {
        *selected += 1;
        if *selected >= *scroll + VISIBLE {
            *scroll += 1;
        }
    }
}

fn scroll_up_sel(selected: &mut usize, scroll: &mut usize) {
    if *selected > 0 {
        *selected -= 1;
        if *selected < *scroll {
            *scroll = *selected;
        }
    }
}

fn retained_len(state: &RetainedState) -> usize {
    if let RetainedState::Done(ref ra) = state {
        ra.entries.len()
    } else {
        0
    }
}
