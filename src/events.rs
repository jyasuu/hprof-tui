use crate::app::{App, Tab};
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_key(app: &mut App, key: KeyEvent) {
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
        // Sort toggle (histogram)
        KeyCode::Char('s') => app.toggle_hist_sort(),
        // Jump to tab by number
        KeyCode::Char('1') => app.active_tab = Tab::Overview,
        KeyCode::Char('2') => app.active_tab = Tab::Histogram,
        KeyCode::Char('3') => app.active_tab = Tab::Retained,
        KeyCode::Char('4') => app.active_tab = Tab::LeakSuspects,
        KeyCode::Char('5') => app.active_tab = Tab::Waste,
        KeyCode::Char('6') => app.active_tab = Tab::DomTree,
        KeyCode::Char('?') => app.active_tab = Tab::Help,
        // Dominator tree drill-down (only active on DomTree tab)
        KeyCode::Enter | KeyCode::Char('i') => {
            if app.active_tab == Tab::DomTree {
                app.dom_drill_in();
            }
        }
        KeyCode::Esc | KeyCode::Backspace | KeyCode::Char('o') => {
            if app.active_tab == Tab::DomTree {
                app.dom_drill_out();
            }
        }
        _ => {}
    }
}
