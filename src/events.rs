use crate::app::App;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    // Clear status message on any keypress
    app.status_message = None;

    match key.code {
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => app.next_tab(),
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => app.prev_tab(),
        KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
        KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
        KeyCode::PageDown | KeyCode::Char('d') => app.page_down(),
        KeyCode::PageUp | KeyCode::Char('u') => app.page_up(),
        KeyCode::Char('s') => app.toggle_sort(),
        KeyCode::Char('1') => app.active_tab = crate::app::Tab::Overview,
        KeyCode::Char('2') => app.active_tab = crate::app::Tab::Histogram,
        KeyCode::Char('3') => app.active_tab = crate::app::Tab::LeakSuspects,
        KeyCode::Char('4') => app.active_tab = crate::app::Tab::GcRoots,
        KeyCode::Char('5') => app.active_tab = crate::app::Tab::DuplicateStrings,
        KeyCode::Char('?') => app.active_tab = crate::app::Tab::Help,
        KeyCode::Home | KeyCode::Char('g') => app.go_to_top(),
        KeyCode::End | KeyCode::Char('G') => app.go_to_bottom(),
        _ => {}
    }
}
