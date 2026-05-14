//! Key-event handling for the TUI. Pure transitions on [`AppState`].

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::tui::state::AppState;

pub enum Action {
    None,
    Quit,
}

pub fn handle(key: KeyEvent, state: &mut AppState) -> Action {
    let scroll_step = 1u16;
    let page_step = 10u16;
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::Quit,

        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
            state.autoscroll = false;
            state.scroll = state.scroll.saturating_sub(scroll_step);
            Action::None
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
            state.autoscroll = false;
            state.scroll = state.scroll.saturating_add(scroll_step);
            Action::None
        }
        (KeyCode::PageUp, _) => {
            state.autoscroll = false;
            state.scroll = state.scroll.saturating_sub(page_step);
            Action::None
        }
        (KeyCode::PageDown, _) => {
            state.autoscroll = false;
            state.scroll = state.scroll.saturating_add(page_step);
            Action::None
        }

        (KeyCode::Char('g'), _) => {
            state.autoscroll = false;
            state.scroll = 0;
            Action::None
        }
        (KeyCode::Char('G'), _) => {
            // Bottom + re-enable autoscroll so new events stay in view.
            state.autoscroll = true;
            Action::None
        }

        (KeyCode::Char('p'), _) => {
            // Toggle autoscroll. If we're turning it on, snap to bottom.
            state.autoscroll = !state.autoscroll;
            Action::None
        }

        _ => Action::None,
    }
}
