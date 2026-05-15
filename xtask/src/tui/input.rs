//! Key-event handling for the TUI. Pure transitions on [`AppState`].

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::agent_events;
use crate::flow::AgentProvider;
use crate::paths::add_card_log_root;
use crate::tui::state::{AppState, FocusPane, HistoryEntry, Iteration, TimelineKind, TimelineRow};

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
            state.materialize_output_scroll();
            state.autoscroll = false;
            scroll_focused(state, false, scroll_step);
            Action::None
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
            state.materialize_output_scroll();
            state.autoscroll = false;
            scroll_focused(state, true, scroll_step);
            Action::None
        }
        (KeyCode::PageUp, _) => {
            state.materialize_output_scroll();
            state.autoscroll = false;
            scroll_focused(state, false, page_step);
            Action::None
        }
        (KeyCode::PageDown, _) => {
            state.materialize_output_scroll();
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
            state.scroll = state.output_bottom_scroll();
            state.autoscroll = true;
            Action::None
        }

        (KeyCode::Char('p'), _) => {
            if state.autoscroll {
                state.pause_output();
            } else {
                state.scroll = state.output_bottom_scroll();
                state.autoscroll = true;
            }
            Action::None
        }
        (KeyCode::Char('c'), m) if m.is_empty() => {
            state.copy_mode = true;
            Action::None
        }
        (KeyCode::Char('v'), _) | (KeyCode::Char('V'), _) => {
            state.materialize_output_scroll();
            state.autoscroll = false;
            state
                .visual
                .start(state.scroll.min(state.output_line_count.saturating_sub(1)));
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

pub fn handle_mouse(mouse: MouseEvent, state: &mut AppState) -> Action {
    let scroll_step = 3u16;
    match mouse.kind {
        MouseEventKind::ScrollUp => {
            if state.history.open {
                state.history.selected = state.history.selected.saturating_sub(1);
            } else {
                state.materialize_output_scroll();
                state.autoscroll = false;
                scroll_focused(state, false, scroll_step);
                if state.visual.active {
                    state.visual.cursor = state.scroll;
                }
            }
        }
        MouseEventKind::ScrollDown => {
            if state.history.open {
                if !state.history.entries.is_empty() {
                    state.history.selected =
                        (state.history.selected + 1).min(state.history.entries.len() - 1);
                }
            } else {
                state.materialize_output_scroll();
                state.autoscroll = false;
                scroll_focused(state, true, scroll_step);
                if state.visual.active {
                    state.visual.cursor = state.scroll;
                }
            }
        }
        _ => {}
    }
    Action::None
}

fn handle_visual_key(key: KeyEvent, state: &mut AppState) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.visual.cancel();
            Action::None
        }
        KeyCode::Char('y') => Action::Copy(CopyTarget::Visual),
        KeyCode::Up | KeyCode::Char('k') => {
            state.materialize_output_scroll();
            state.autoscroll = false;
            state.scroll = state.scroll.saturating_sub(1);
            state.visual.cursor = state.scroll;
            Action::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            state.materialize_output_scroll();
            state.autoscroll = false;
            state.scroll = state
                .scroll
                .saturating_add(1)
                .min(state.output_bottom_scroll());
            state.visual.cursor = state.scroll;
            Action::None
        }
        KeyCode::PageUp => {
            state.materialize_output_scroll();
            state.autoscroll = false;
            state.scroll = state.scroll.saturating_sub(10);
            state.visual.cursor = state.scroll;
            Action::None
        }
        KeyCode::PageDown => {
            state.materialize_output_scroll();
            state.autoscroll = false;
            state.scroll = state
                .scroll
                .saturating_add(10)
                .min(state.output_bottom_scroll());
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
        KeyCode::Down | KeyCode::Char('j') if !state.history.entries.is_empty() => {
            state.history.selected =
                (state.history.selected + 1).min(state.history.entries.len() - 1);
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
    state.events.clear();
    state.iterations.clear();
    state.session_end = None;
    state.autoscroll = true;
    state.iterations.push(Iteration {
        index: entry.iteration_index,
        max_iterations: 0,
        card: Some(entry.card.clone()),
        ..Iteration::default()
    });
    state.events.push(TimelineRow {
        iteration_index: entry.iteration_index,
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
        let provider = infer_history_provider(&raw);
        for parsed in agent_events::parse(provider, &raw) {
            state.events.push(TimelineRow {
                iteration_index: entry.iteration_index,
                delta: 0,
                kind: TimelineKind::Agent { provider, parsed },
            });
        }
    }
    state.scroll = 0;
}

fn infer_history_provider(raw: &serde_json::Value) -> AgentProvider {
    let kind = raw
        .get("type")
        .or_else(|| raw.get("event"))
        .or_else(|| raw.get("msg_type"))
        .and_then(|v| v.as_str())
        .unwrap_or("");
    if kind.contains('.')
        || raw.get("item").is_some()
        || raw.get("event").is_some()
        || raw.get("msg_type").is_some()
    {
        AgentProvider::Codex
    } else {
        AgentProvider::Claude
    }
}

fn scroll_focused(state: &mut AppState, down: bool, amount: u16) {
    match state.focus {
        FocusPane::Output | FocusPane::Modal => {
            if down {
                state.scroll = state
                    .scroll
                    .saturating_add(amount)
                    .min(state.output_bottom_scroll());
            } else {
                state.scroll = state.scroll.saturating_sub(amount);
            }
        }
        FocusPane::Card => {
            if down {
                state.card_scroll = state.card_scroll.saturating_add(amount);
            } else {
                state.card_scroll = state.card_scroll.saturating_sub(amount);
            }
        }
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
    let mut dirs = Vec::new();
    let Ok(read_dir) = std::fs::read_dir(add_card_log_root()) else {
        return Vec::new();
    };
    for entry in read_dir.flatten() {
        let path = entry.path();
        if !path.is_dir() {
            continue;
        }
        let transcript = path.join("transcript.ndjson");
        if transcript.exists() {
            dirs.push((path, transcript));
        }
    }
    dirs.sort_by(|a, b| a.0.cmp(&b.0));
    let mut entries = dirs
        .into_iter()
        .enumerate()
        .filter_map(|(i, (path, transcript))| {
            let iteration_index = (i + 1) as u32;
            let card = history_card(&path)?;
            let card_name = card.name.clone();
            Some(HistoryEntry {
                name: format!("iteration {iteration_index} - {card_name}"),
                iteration_index,
                card,
                path: transcript,
            })
        })
        .collect::<Vec<_>>();
    entries.reverse();
    entries
}

fn history_card(path: &std::path::Path) -> Option<mtg_scryfall::Card> {
    let text = std::fs::read_to_string(path.join("card.json")).ok()?;
    serde_json::from_str(&text).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wheel(kind: MouseEventKind) -> MouseEvent {
        MouseEvent {
            kind,
            column: 0,
            row: 0,
            modifiers: KeyModifiers::empty(),
        }
    }

    fn key(code: KeyCode) -> KeyEvent {
        KeyEvent::new(code, KeyModifiers::empty())
    }

    #[test]
    fn mouse_wheel_scrolls_focused_output_and_disables_autoscroll() {
        let mut state = AppState::new();
        state.remember_output_view(100, 20);
        state.autoscroll = true;

        handle_mouse(wheel(MouseEventKind::ScrollDown), &mut state);

        assert!(!state.autoscroll);
        assert_eq!(state.scroll, 80);
    }

    #[test]
    fn mouse_wheel_scrolls_card_when_card_pane_is_focused() {
        let mut state = AppState::new();
        state.focus = FocusPane::Card;

        handle_mouse(wheel(MouseEventKind::ScrollDown), &mut state);

        assert_eq!(state.card_scroll, 3);
        assert_eq!(state.scroll, 0);
    }

    #[test]
    fn normal_scroll_up_from_autoscroll_moves_one_line_from_bottom() {
        let mut state = AppState::new();
        state.remember_output_view(100, 20);
        state.autoscroll = true;
        state.scroll = 0;

        let action = handle(key(KeyCode::Char('k')), &mut state);

        assert!(matches!(action, Action::None));
        assert!(!state.autoscroll);
        assert_eq!(state.scroll, 79);
    }

    #[test]
    fn visual_mode_starts_at_visible_bottom_when_autoscrolling() {
        let mut state = AppState::new();
        state.remember_output_view(100, 20);
        state.autoscroll = true;
        state.scroll = 0;

        let action = handle(key(KeyCode::Char('v')), &mut state);

        assert!(matches!(action, Action::None));
        assert!(!state.autoscroll);
        assert!(state.visual.active);
        assert_eq!(state.visual.anchor, 80);
        assert_eq!(state.visual.cursor, 80);
    }

    #[test]
    fn visual_down_extends_selection_one_line() {
        let mut state = AppState::new();
        state.remember_output_view(100, 20);
        state.scroll = 40;
        state.autoscroll = false;
        state.visual.start(40);

        let action = handle(key(KeyCode::Char('j')), &mut state);

        assert!(matches!(action, Action::None));
        assert_eq!(state.scroll, 41);
        assert_eq!(state.visual.range(), Some((40, 41)));
    }
}
