//! Key-event handling for the TUI. Pure transitions on [`AppState`].

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::agent_events;
use crate::flow::AgentProvider;
use crate::paths::grammar_fix_log_root;
use crate::tui::state::{AppState, FocusPane, HistoryEntry, TimelineKind, TimelineRow};

pub enum Action {
    None,
    Quit,
    Copy(CopyTarget),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CopyTarget {
    Output,
    Card,
    Steps,
    All,
    Visual,
}

pub fn handle(key: KeyEvent, state: &mut AppState) -> Action {
    let scroll_step = 1u16;
    let page_step = 10u16;
    if matches!(key.code, KeyCode::Char('c')) && key.modifiers == KeyModifiers::CONTROL {
        return Action::Quit;
    }
    if state.search.editing {
        return handle_search_key(key, state);
    }
    if state.history.open {
        return handle_history_key(key, state);
    }
    if state.visual.active {
        return handle_visual_key(key, state);
    }
    if state.copy_mode {
        return handle_copy_key(key, state);
    }
    match (key.code, key.modifiers) {
        (KeyCode::Char('q'), _) | (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::Quit,
        (KeyCode::Tab, _) => {
            state.focus = match state.focus {
                FocusPane::Output => FocusPane::Card,
                FocusPane::Card | FocusPane::Modal => FocusPane::Output,
            };
            Action::None
        }

        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
            state.autoscroll = false;
            scroll_focused(state, false, scroll_step);
            Action::None
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
            state.autoscroll = false;
            scroll_focused(state, true, scroll_step);
            Action::None
        }
        (KeyCode::PageUp, _) => {
            state.autoscroll = false;
            scroll_focused(state, false, page_step);
            Action::None
        }
        (KeyCode::PageDown, _) => {
            state.autoscroll = false;
            scroll_focused(state, true, page_step);
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
        (KeyCode::Char('c'), m) if m.is_empty() => {
            state.copy_mode = true;
            Action::None
        }
        (KeyCode::Char('v'), _) | (KeyCode::Char('V'), _) => {
            state.autoscroll = false;
            state.visual.start(state.scroll);
            Action::None
        }
        (KeyCode::Char('/'), _) => {
            state.search.editing = true;
            Action::None
        }
        (KeyCode::Char('f'), _) => {
            state.search.filter_mode = !state.search.filter_mode;
            if state.search.query.is_empty() {
                state.search.editing = true;
            }
            Action::None
        }
        (KeyCode::Char('n'), _) => {
            state.search.current_match = state.search.current_match.saturating_add(1);
            Action::None
        }
        (KeyCode::Char('N'), _) => {
            state.search.current_match = state.search.current_match.saturating_sub(1);
            Action::None
        }
        (KeyCode::Char('H'), _) => {
            state.history.entries = load_history_entries();
            state.history.open = true;
            state.focus = FocusPane::Modal;
            Action::None
        }
        (KeyCode::Char(c), _) if ('1'..='9').contains(&c) => {
            jump_to_step(state, c as u8 - b'0');
            Action::None
        }
        (KeyCode::Char('i'), _) => {
            state.scroll = 0;
            state.autoscroll = false;
            Action::None
        }

        _ => Action::None,
    }
}

fn handle_visual_key(key: KeyEvent, state: &mut AppState) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.visual.cancel();
            Action::None
        }
        KeyCode::Char('y') => {
            state.visual.cancel();
            Action::Copy(CopyTarget::Visual)
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.autoscroll = false;
            state.scroll = state.scroll.saturating_sub(1);
            state.visual.cursor = state.scroll;
            Action::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.autoscroll = false;
            state.scroll = state.scroll.saturating_add(1);
            state.visual.cursor = state.scroll;
            Action::None
        }
        KeyCode::PageUp => {
            state.autoscroll = false;
            state.scroll = state.scroll.saturating_sub(10);
            state.visual.cursor = state.scroll;
            Action::None
        }
        KeyCode::PageDown => {
            state.autoscroll = false;
            state.scroll = state.scroll.saturating_add(10);
            state.visual.cursor = state.scroll;
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_copy_key(key: KeyEvent, state: &mut AppState) -> Action {
    let target = match key.code {
        KeyCode::Esc => {
            state.copy_mode = false;
            return Action::None;
        }
        KeyCode::Char('o') => CopyTarget::Output,
        KeyCode::Char('c') => CopyTarget::Card,
        KeyCode::Char('s') => CopyTarget::Steps,
        KeyCode::Char('a') => CopyTarget::All,
        _ => return Action::None,
    };
    state.copy_mode = false;
    Action::Copy(target)
}

fn handle_search_key(key: KeyEvent, state: &mut AppState) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.search.editing = false;
            state.search.query.clear();
            Action::None
        }
        KeyCode::Enter => {
            state.search.editing = false;
            Action::None
        }
        KeyCode::Backspace => {
            state.search.query.pop();
            Action::None
        }
        KeyCode::Char(c) => {
            state.search.query.push(c);
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_history_key(key: KeyEvent, state: &mut AppState) -> Action {
    match key.code {
        KeyCode::Esc | KeyCode::Char('H') => {
            state.history.open = false;
            state.focus = FocusPane::Output;
        }
        KeyCode::Up | KeyCode::Char('k') => {
            state.history.selected = state.history.selected.saturating_sub(1);
        }
        KeyCode::Down | KeyCode::Char('j') => {
            if !state.history.entries.is_empty() {
                state.history.selected =
                    (state.history.selected + 1).min(state.history.entries.len() - 1);
            }
        }
        KeyCode::Enter => {
            if let Some(entry) = state.history.entries.get(state.history.selected).cloned() {
                load_history_transcript(state, &entry);
                state.search.filter_mode = true;
                state.history.open = false;
                state.focus = FocusPane::Output;
            }
        }
        _ => {}
    }
    Action::None
}

fn load_history_transcript(state: &mut AppState, entry: &HistoryEntry) {
    let Ok(text) = std::fs::read_to_string(&entry.path) else {
        return;
    };
    state.events.push(TimelineRow {
        iteration_index: 0,
        delta: 0,
        kind: TimelineKind::Note {
            level: crate::flow::NoteLevel::Info,
            text: format!("loaded history {}", entry.name),
        },
    });
    for line in text.lines() {
        let Ok(raw) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        for parsed in agent_events::parse(AgentProvider::Claude, &raw) {
            state.events.push(TimelineRow {
                iteration_index: 0,
                delta: 0,
                kind: TimelineKind::Agent {
                    provider: AgentProvider::Claude,
                    parsed,
                },
            });
        }
    }
    state.scroll = state.events.len().saturating_sub(1) as u16;
}

fn scroll_focused(state: &mut AppState, down: bool, amount: u16) {
    let slot = match state.focus {
        FocusPane::Output | FocusPane::Modal => &mut state.scroll,
        FocusPane::Card => &mut state.card_scroll,
    };
    if down {
        *slot = slot.saturating_add(amount);
    } else {
        *slot = slot.saturating_sub(amount);
    }
}

fn jump_to_step(state: &mut AppState, step: u8) {
    if let Some(pos) = state.events.iter().position(|row| {
        matches!(
            row.kind,
            crate::tui::state::TimelineKind::StepHeader { index, .. } if index == step
        )
    }) {
        state.scroll = pos as u16;
        state.autoscroll = false;
    }
}

fn load_history_entries() -> Vec<HistoryEntry> {
    let mut entries = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(grammar_fix_log_root()) else {
        return entries;
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let transcript = path.join("transcript.ndjson");
        if transcript.exists() {
            entries.push(HistoryEntry {
                name: path
                    .file_name()
                    .map(|n| n.to_string_lossy().to_string())
                    .unwrap_or_else(|| path.display().to_string()),
                path: transcript,
            });
        }
    }
    entries.sort_by(|a, b| b.name.cmp(&a.name));
    entries
}
