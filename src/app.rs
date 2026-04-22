use crate::parser::{analyze_hprof, HprofAnalysis};
use anyhow::Result;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Overview,
    Histogram,
    LeakSuspects,
    GcRoots,
    DuplicateStrings,
    Help,
}

impl Tab {
    pub const ALL: &'static [Tab] = &[
        Tab::Overview,
        Tab::Histogram,
        Tab::LeakSuspects,
        Tab::GcRoots,
        Tab::DuplicateStrings,
        Tab::Help,
    ];

    pub fn title(&self) -> &str {
        match self {
            Tab::Overview => "Overview",
            Tab::Histogram => "Histogram",
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

pub struct App {
    pub analysis: HprofAnalysis,
    pub active_tab: Tab,
    pub histogram_scroll: usize,
    pub histogram_selected: usize,
    pub leak_scroll: usize,
    pub leak_selected: usize,
    pub gc_scroll: usize,
    pub gc_selected: usize,
    pub dup_scroll: usize,
    pub dup_selected: usize,
    pub sort_by_count: bool, // false = sort by size
    pub status_message: Option<String>,
}

impl App {
    pub fn new(path: &str) -> Result<Self> {
        let analysis = analyze_hprof(path)?;
        Ok(App {
            analysis,
            active_tab: Tab::Overview,
            histogram_scroll: 0,
            histogram_selected: 0,
            leak_scroll: 0,
            leak_selected: 0,
            gc_scroll: 0,
            gc_selected: 0,
            dup_scroll: 0,
            dup_selected: 0,
            sort_by_count: false,
            status_message: None,
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
            Tab::Histogram => {
                let max = self.analysis.class_histogram.len().saturating_sub(1);
                if self.histogram_selected < max {
                    self.histogram_selected += 1;
                    if self.histogram_selected >= self.histogram_scroll + visible_rows() {
                        self.histogram_scroll += 1;
                    }
                }
            }
            Tab::LeakSuspects => {
                let max = self.analysis.leak_suspects.len().saturating_sub(1);
                if self.leak_selected < max {
                    self.leak_selected += 1;
                    if self.leak_selected >= self.leak_scroll + visible_rows() {
                        self.leak_scroll += 1;
                    }
                }
            }
            Tab::GcRoots => {
                let max = self.analysis.gc_roots.len().saturating_sub(1);
                if self.gc_selected < max {
                    self.gc_selected += 1;
                    if self.gc_selected >= self.gc_scroll + visible_rows() {
                        self.gc_scroll += 1;
                    }
                }
            }
            Tab::DuplicateStrings => {
                let max = self.analysis.duplicate_strings.len().saturating_sub(1);
                if self.dup_selected < max {
                    self.dup_selected += 1;
                    if self.dup_selected >= self.dup_scroll + visible_rows() {
                        self.dup_scroll += 1;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn scroll_up(&mut self) {
        match self.active_tab {
            Tab::Histogram => {
                if self.histogram_selected > 0 {
                    self.histogram_selected -= 1;
                    if self.histogram_selected < self.histogram_scroll {
                        self.histogram_scroll = self.histogram_selected;
                    }
                }
            }
            Tab::LeakSuspects => {
                if self.leak_selected > 0 {
                    self.leak_selected -= 1;
                    if self.leak_selected < self.leak_scroll {
                        self.leak_scroll = self.leak_selected;
                    }
                }
            }
            Tab::GcRoots => {
                if self.gc_selected > 0 {
                    self.gc_selected -= 1;
                    if self.gc_selected < self.gc_scroll {
                        self.gc_scroll = self.gc_selected;
                    }
                }
            }
            Tab::DuplicateStrings => {
                if self.dup_selected > 0 {
                    self.dup_selected -= 1;
                    if self.dup_selected < self.dup_scroll {
                        self.dup_scroll = self.dup_selected;
                    }
                }
            }
            _ => {}
        }
    }

    pub fn page_down(&mut self) {
        for _ in 0..10 {
            self.scroll_down();
        }
    }

    pub fn page_up(&mut self) {
        for _ in 0..10 {
            self.scroll_up();
        }
    }

    pub fn go_to_top(&mut self) {
        match self.active_tab {
            Tab::Histogram => {
                self.histogram_selected = 0;
                self.histogram_scroll = 0;
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
                self.histogram_scroll = n.saturating_sub(visible_rows() - 1);
            }
            Tab::LeakSuspects => {
                let n = self.analysis.leak_suspects.len().saturating_sub(1);
                self.leak_selected = n;
                self.leak_scroll = n.saturating_sub(visible_rows() - 1);
            }
            Tab::GcRoots => {
                let n = self.analysis.gc_roots.len().saturating_sub(1);
                self.gc_selected = n;
                self.gc_scroll = n.saturating_sub(visible_rows() - 1);
            }
            Tab::DuplicateStrings => {
                let n = self.analysis.duplicate_strings.len().saturating_sub(1);
                self.dup_selected = n;
                self.dup_scroll = n.saturating_sub(visible_rows() - 1);
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
            self.status_message = Some("Sorted by instance count".to_string());
        } else {
            self.analysis
                .class_histogram
                .sort_by(|a, b| b.shallow_size.cmp(&a.shallow_size));
            self.status_message = Some("Sorted by shallow size".to_string());
        }
        self.histogram_selected = 0;
        self.histogram_scroll = 0;
    }
}

fn visible_rows() -> usize {
    20 // approximate; real value would come from terminal size
}
// --- additional methods appended ---
