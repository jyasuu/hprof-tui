use crate::app::{App, InputMode, InspectorFocus, Tab};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    // Route to tab-specific input handlers first
    match app.active_tab {
        Tab::Query if app.query.mode == InputMode::Editing => {
            handle_query_input(app, key);
            return;
        }
        Tab::Inspector if app.inspector.mode == InputMode::Editing => {
            handle_inspector_input(app, key);
            return;
        }
        _ => {}
    }

    app.status = None;

    match key.code {
        // Tab navigation
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => app.next_tab(),
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => app.prev_tab(),

        // Scroll
        KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
        KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
        KeyCode::PageDown | KeyCode::Char('d') => app.page_down(),
        KeyCode::PageUp | KeyCode::Char('u') => app.page_up(),
        KeyCode::Home | KeyCode::Char('g') => app.go_top(),

        // Sort / sub-view toggle — context-sensitive
        KeyCode::Char('s') => match app.active_tab {
            Tab::Waste => app.waste_cycle_sub(),
            Tab::LeakSuspects => app.toggle_leak_view(),
            _ => app.toggle_hist_sort(),
        },

        // Jump to tab by number
        KeyCode::Char('1') => app.active_tab = Tab::Overview,
        KeyCode::Char('2') => app.active_tab = Tab::Histogram,
        KeyCode::Char('3') => app.active_tab = Tab::Retained,
        KeyCode::Char('4') => app.active_tab = Tab::LeakSuspects,
        KeyCode::Char('5') => app.active_tab = Tab::Waste,
        KeyCode::Char('6') => app.active_tab = Tab::DomTree,
        KeyCode::Char('7') => app.active_tab = Tab::Query,
        KeyCode::Char('8') => app.active_tab = Tab::Inspector,
        KeyCode::Char('?') => app.active_tab = Tab::Help,

        // Dominator tree drill-down
        KeyCode::Enter | KeyCode::Char('i') => match app.active_tab {
            Tab::DomTree => app.dom_drill_in(),
            Tab::Query => {
                // Enter edit mode for query
                app.query.mode = InputMode::Editing;
            }
            Tab::Inspector => {
                match app.inspector.focus {
                    InspectorFocus::Input => {
                        app.inspector.mode = InputMode::Editing;
                    }
                    _ => {
                        // Drill into selected referrer or path item
                        app.inspector_enter_selection();
                    }
                }
            }
            _ => {}
        },

        KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('o') => match app.active_tab {
            Tab::DomTree => app.dom_drill_out(),
            Tab::Inspector => {
                app.inspector.focus = InspectorFocus::Input;
            }
            _ => {}
        },

        // Inspector: jump to inspector from dominator tree
        KeyCode::Char('x') => {
            if app.active_tab == Tab::DomTree {
                app.inspect_dom_selection();
            }
        }

        // Inspector: cycle panels with Tab when on Inspector tab (normal mode)
        KeyCode::Char('p') => {
            if app.active_tab == Tab::Inspector {
                app.inspector_cycle_panel();
            }
        }

        // HeapQL: open editor
        KeyCode::Char('e') => {
            if app.active_tab == Tab::Query {
                app.query.mode = InputMode::Editing;
            }
            if app.active_tab == Tab::Inspector {
                app.inspector.focus = InspectorFocus::Input;
                app.inspector.mode = InputMode::Editing;
            }
        }

        _ => {}
    }
}

fn handle_query_input(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.query.mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            app.query.mode = InputMode::Normal;
            app.query_execute();
        }
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => match c {
            'c' => app.query.mode = InputMode::Normal,
            'u' => {
                app.query.input.clear();
                app.query.cursor = 0;
            }
            _ => {}
        },
        KeyCode::Up => {
            app.query_history_nav(true);
        }
        KeyCode::Down => {
            app.query_history_nav(false);
        }
        KeyCode::Left => {
            if app.query.cursor > 0 {
                app.query.cursor -= 1;
            }
        }
        KeyCode::Right => {
            if app.query.cursor < app.query.input.len() {
                app.query.cursor += 1;
            }
        }
        KeyCode::Home => {
            app.query.cursor = 0;
        }
        KeyCode::End => {
            app.query.cursor = app.query.input.len();
        }
        KeyCode::Backspace => {
            if app.query.cursor > 0 {
                let pos = app.query.cursor - 1;
                app.query.input.remove(pos);
                app.query.cursor = pos;
            }
        }
        KeyCode::Delete => {
            if app.query.cursor < app.query.input.len() {
                app.query.input.remove(app.query.cursor);
            }
        }
        KeyCode::Char(c) => {
            app.query.input.insert(app.query.cursor, c);
            app.query.cursor += c.len_utf8();
        }
        _ => {}
    }
}

fn handle_inspector_input(app: &mut App, key: KeyEvent) {
    match key.code {
        KeyCode::Esc => {
            app.inspector.mode = InputMode::Normal;
        }
        KeyCode::Enter => {
            app.inspector.mode = InputMode::Normal;
            app.inspector_load();
        }
        KeyCode::Char(c) if key.modifiers.contains(KeyModifiers::CONTROL) => match c {
            'c' => app.inspector.mode = InputMode::Normal,
            'u' => {
                app.inspector.input.clear();
                app.inspector.cursor = 0;
            }
            _ => {}
        },
        KeyCode::Left => {
            if app.inspector.cursor > 0 {
                app.inspector.cursor -= 1;
            }
        }
        KeyCode::Right => {
            if app.inspector.cursor < app.inspector.input.len() {
                app.inspector.cursor += 1;
            }
        }
        KeyCode::Home => { app.inspector.cursor = 0; }
        KeyCode::End => { app.inspector.cursor = app.inspector.input.len(); }
        KeyCode::Backspace => {
            if app.inspector.cursor > 0 {
                let pos = app.inspector.cursor - 1;
                app.inspector.input.remove(pos);
                app.inspector.cursor = pos;
            }
        }
        KeyCode::Delete => {
            if app.inspector.cursor < app.inspector.input.len() {
                app.inspector.input.remove(app.inspector.cursor);
            }
        }
        KeyCode::Char(c) => {
            app.inspector.input.insert(app.inspector.cursor, c);
            app.inspector.cursor += c.len_utf8();
        }
        _ => {}
    }
}

impl App {
    /// When Enter is pressed on a referrer or GC-path row, jump to that object.
    pub fn inspector_enter_selection(&mut self) {
        let oid = match self.inspector.focus {
            InspectorFocus::Fields => {
                // If the selected field is a ref, jump to the referenced object
                self.inspector.fields.get(self.inspector.field_sel)
                    .and_then(|f| f.ref_object_id)
            }
            InspectorFocus::Referrers => {
                self.inspector.referrers.get(self.inspector.ref_sel).map(|r| r.object_id)
            }
            InspectorFocus::GcPath => {
                self.inspector.gc_path.get(self.inspector.gc_sel).map(|r| r.object_id)
            }
            _ => None,
        };
        if let Some(id) = oid {
            if id > 0 {
                self.inspector.input = format!("0x{:x}", id);
                self.inspector.cursor = self.inspector.input.len();
                self.inspector_load();
            } else {
                self.status = Some("null reference — nothing to navigate to".into());
            }
        } else {
            self.status = Some("Selected field is a primitive — no object to navigate to".into());
        }
    }

}
