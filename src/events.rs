use crate::app::{App, RetainedState, Tab};
use crate::retained::compute_retained;
use crossterm::event::{KeyCode, KeyEvent};

pub fn handle_key(app: &mut App, key: KeyEvent) {
    app.status_message = None;

    match key.code {
        KeyCode::Tab | KeyCode::Right | KeyCode::Char('l') => app.next_tab(),
        KeyCode::BackTab | KeyCode::Left | KeyCode::Char('h') => app.prev_tab(),
        KeyCode::Down | KeyCode::Char('j') => app.scroll_down(),
        KeyCode::Up | KeyCode::Char('k') => app.scroll_up(),
        KeyCode::PageDown | KeyCode::Char('d') => app.page_down(),
        KeyCode::PageUp | KeyCode::Char('u') => app.page_up(),
        KeyCode::Home | KeyCode::Char('g') => app.go_to_top(),
        KeyCode::End | KeyCode::Char('G') => app.go_to_bottom(),
        KeyCode::Char('s') => app.toggle_sort(),
        KeyCode::Char('1') => app.active_tab = Tab::Overview,
        KeyCode::Char('2') => app.active_tab = Tab::Histogram,
        KeyCode::Char('3') => {
            app.active_tab = Tab::RetainedGraph;
            // Trigger retained-size computation on first visit
            trigger_retained(app);
        }
        KeyCode::Char('4') => app.active_tab = Tab::LeakSuspects,
        KeyCode::Char('5') => app.active_tab = Tab::GcRoots,
        KeyCode::Char('6') => app.active_tab = Tab::DuplicateStrings,
        KeyCode::Char('?') => app.active_tab = Tab::Help,
        KeyCode::Enter => {
            // On Retained tab, pressing Enter re-triggers if not started
            if app.active_tab == Tab::RetainedGraph {
                trigger_retained(app);
            }
        }
        _ => {}
    }
}

fn trigger_retained(app: &mut App) {
    match app.retained_state {
        RetainedState::NotStarted | RetainedState::Error(_) => {
            app.retained_state = RetainedState::Computing;
            // Run synchronously (blocking) — in a real app this would be a thread
            let path = app.hprof_path.clone();
            match compute_retained(&path) {
                Ok(ra) => {
                    let count = ra.entries.len();
                    app.retained_state = RetainedState::Done(ra);
                    app.status_message = Some(format!("Retained analysis done: {} classes", count));
                }
                Err(e) => {
                    app.retained_state = RetainedState::Error(e.to_string());
                    app.status_message = Some(format!("Retained analysis failed: {}", e));
                }
            }
        }
        _ => {} // Already computing or done
    }
}
