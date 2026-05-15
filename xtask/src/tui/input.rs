//! Key-event handling for the TUI. Pure transitions on [`AppState`].

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers, MouseEvent, MouseEventKind};

use crate::agent_events;
use crate::flow::AgentProvider;
use crate::paths::add_card_log_root;
use crate::tui::state::{AppState, FocusPane, HistoryEntry, Iteration, TimelineKind, TimelineRow};

pub enum Action {
    None,
    Quit,
    YankVisual,
    OpenCard,
}

pub fn handle(key: KeyEvent, state: &mut AppState) -> Action {
    let scroll_step = 1u16;
    let page_step = 10u16;
    if matches!(key.code, KeyCode::Char('c')) && key.modifiers == KeyModifiers::CONTROL {
        return Action::Quit;
    }
    if state.command.editing {
        return handle_command_key(key, state);
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
    match (key.code, key.modifiers) {
        (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::Quit,
        (KeyCode::Char(':'), m) if m.is_empty() => {
            state.normal.clear();
            state.command.editing = true;
            state.command.input.clear();
            Action::None
        }
        (KeyCode::Char(' '), m) if m.is_empty() => {
            state.normal.clear();
            state.normal.leader = true;
            Action::None
        }

        (KeyCode::Tab, _) if state.normal.leader => {
            state.normal.clear();
            switch_focus(
                state,
                match state.focus {
                    FocusPane::Output => FocusPane::Steps,
                    FocusPane::Steps => FocusPane::Card,
                    FocusPane::Card | FocusPane::Modal => FocusPane::Output,
                },
            );
            Action::None
        }
        (KeyCode::Char('h'), _) if state.normal.leader => {
            state.normal.clear();
            switch_focus(state, FocusPane::Card);
            Action::None
        }
        (KeyCode::Char('l'), _) if state.normal.leader => {
            state.normal.clear();
            switch_focus(state, FocusPane::Output);
            Action::None
        }
        (KeyCode::Char('s'), _) if state.normal.leader => {
            state.normal.clear();
            switch_focus(state, FocusPane::Steps);
            Action::None
        }
        (KeyCode::Char('H'), _) if state.normal.leader => {
            state.normal.clear();
            state.history.entries = load_history_entries();
            state.history.open = true;
            state.focus = FocusPane::Modal;
            Action::None
        }
        (KeyCode::Char('/'), _) if state.normal.leader => {
            state.normal.clear();
            state.search.editing = true;
            Action::None
        }
        (KeyCode::Char('f'), _) if state.normal.leader => {
            state.normal.clear();
            state.search.filter_mode = !state.search.filter_mode;
            if state.search.query.is_empty() {
                state.search.editing = true;
            }
            Action::None
        }
        (KeyCode::Char('c' | 'o'), m) if m.is_empty() && state.normal.leader => {
            state.normal.clear();
            Action::OpenCard
        }
        (KeyCode::Char('p'), _) if state.normal.leader => {
            state.normal.clear();
            if state.autoscroll {
                state.pause_output();
            } else {
                state.scroll = state.output_bottom_scroll();
                state.output_cursor = state.output_line_count.saturating_sub(1);
                state.autoscroll = true;
            }
            Action::None
        }

        (KeyCode::Char(c), _) if c.is_ascii_digit() && key.modifiers.is_empty() => {
            if c != '0' || !state.normal.count.is_empty() {
                state.normal.count.push(c);
            }
            state.normal.leader = false;
            state.normal.pending_g = false;
            Action::None
        }

        (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
            let amount = state.normal.take_count().unwrap_or(scroll_step);
            state.normal.clear();
            scroll_focused(state, false, amount);
            Action::None
        }
        (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
            let amount = state.normal.take_count().unwrap_or(scroll_step);
            state.normal.clear();
            scroll_focused(state, true, amount);
            Action::None
        }
        (KeyCode::PageUp, _) => {
            state.normal.clear();
            scroll_focused(state, false, page_step);
            Action::None
        }
        (KeyCode::PageDown, _) => {
            state.normal.clear();
            scroll_focused(state, true, page_step);
            Action::None
        }

        (KeyCode::Char('g'), _) if state.normal.pending_g => {
            let line = state.normal.take_count().unwrap_or(1).saturating_sub(1);
            state.normal.clear();
            goto_output_line(state, line);
            Action::None
        }
        (KeyCode::Char('g'), _) => {
            state.normal.leader = false;
            state.normal.pending_g = true;
            Action::None
        }
        (KeyCode::Char('G'), _) => {
            state.normal.clear();
            // Bottom + re-enable autoscroll so new events stay in view.
            state.output_cursor = state.output_line_count.saturating_sub(1);
            state.scroll = state.output_bottom_scroll();
            state.autoscroll = true;
            Action::None
        }

        (KeyCode::Char('v'), _) | (KeyCode::Char('V'), _) => {
            state.normal.clear();
            start_visual_for_focus(state);
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
        (KeyCode::Char('i'), _) => {
            state.normal.clear();
            state.scroll = 0;
            state.output_cursor = 0;
            state.autoscroll = false;
            Action::None
        }

        _ => {
            state.normal.clear();
            Action::None
        }
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
                    state.visual.cursor = state.output_cursor;
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
                    state.visual.cursor = state.output_cursor;
                }
            }
        }
        _ => {}
    }
    Action::None
}

fn handle_visual_key(key: KeyEvent, state: &mut AppState) -> Action {
    if state.normal.leader {
        state.normal.clear();
        match key.code {
            KeyCode::Tab => {
                switch_focus(
                    state,
                    match state.focus {
                        FocusPane::Output => FocusPane::Steps,
                        FocusPane::Steps => FocusPane::Card,
                        FocusPane::Card | FocusPane::Modal => FocusPane::Output,
                    },
                );
                return Action::None;
            }
            KeyCode::Char('h') => {
                switch_focus(state, FocusPane::Card);
                return Action::None;
            }
            KeyCode::Char('l') => {
                switch_focus(state, FocusPane::Output);
                return Action::None;
            }
            KeyCode::Char('s') => {
                switch_focus(state, FocusPane::Steps);
                return Action::None;
            }
            _ => {}
        }
    }

    match key.code {
        KeyCode::Esc => {
            state.visual.cancel();
            Action::None
        }
        KeyCode::Char(' ') => {
            state.normal.leader = true;
            Action::None
        }
        KeyCode::Char('y') => Action::YankVisual,
        KeyCode::Char('G') => {
            match state.focus {
                FocusPane::Output | FocusPane::Modal => {
                    state.materialize_output_scroll();
                    state.autoscroll = false;
                    state.output_cursor = state.output_line_count.saturating_sub(1);
                    state.visual.cursor = state.output_cursor;
                    keep_output_cursor_visible(state);
                }
                FocusPane::Steps => {
                    state.steps_cursor = state.steps_line_count.saturating_sub(1);
                    state.visual.cursor = state.steps_cursor;
                }
                FocusPane::Card => {}
            }
            Action::None
        }
        KeyCode::Up | KeyCode::Char('k') => {
            visual_move(state, false, 1);
            Action::None
        }
        KeyCode::Down | KeyCode::Char('j') => {
            visual_move(state, true, 1);
            Action::None
        }
        KeyCode::PageUp => {
            visual_move(state, false, 10);
            Action::None
        }
        KeyCode::PageDown => {
            visual_move(state, true, 10);
            Action::None
        }
        _ => Action::None,
    }
}

fn handle_command_key(key: KeyEvent, state: &mut AppState) -> Action {
    match key.code {
        KeyCode::Esc => {
            state.command.editing = false;
            state.command.input.clear();
            Action::None
        }
        KeyCode::Enter => {
            let command = state.command.input.trim().to_string();
            state.command.editing = false;
            state.command.input.clear();
            match command.as_str() {
                "q" | "quit" => Action::Quit,
                "open-card-on-scryfall" | "open-card" => Action::OpenCard,
                _ => Action::None,
            }
        }
        KeyCode::Backspace => {
            state.command.input.pop();
            Action::None
        }
        KeyCode::Char(c) => {
            state.command.input.push(c);
            Action::None
        }
        _ => Action::None,
    }
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
            state.materialize_output_scroll();
            state.autoscroll = false;
            let last_line = state.output_line_count.saturating_sub(1);
            state.output_cursor = if down {
                state.output_cursor.saturating_add(amount).min(last_line)
            } else {
                state.output_cursor.saturating_sub(amount)
            };
            keep_output_cursor_visible(state);
        }
        FocusPane::Steps => {
            let last_line = state.steps_line_count.saturating_sub(1);
            state.steps_cursor = if down {
                state.steps_cursor.saturating_add(amount).min(last_line)
            } else {
                state.steps_cursor.saturating_sub(amount)
            };
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

fn switch_focus(state: &mut AppState, focus: FocusPane) {
    state.focus = focus;
    if state.visual.active {
        match focus {
            FocusPane::Output | FocusPane::Steps => state.visual.start(state.focused_cursor()),
            FocusPane::Card | FocusPane::Modal => state.visual.cancel(),
        }
    }
}

fn start_visual_for_focus(state: &mut AppState) {
    match state.focus {
        FocusPane::Output | FocusPane::Modal => {
            state.materialize_output_scroll();
            state.autoscroll = false;
            state.visual.start(state.output_cursor);
        }
        FocusPane::Steps => {
            state.visual.start(state.steps_cursor);
        }
        FocusPane::Card => {}
    }
}

fn visual_move(state: &mut AppState, down: bool, amount: u16) {
    match state.focus {
        FocusPane::Output | FocusPane::Modal => {
            state.materialize_output_scroll();
            state.autoscroll = false;
            let last_line = state.output_line_count.saturating_sub(1);
            state.output_cursor = if down {
                state.output_cursor.saturating_add(amount).min(last_line)
            } else {
                state.output_cursor.saturating_sub(amount)
            };
            state.visual.cursor = state.output_cursor;
            keep_output_cursor_visible(state);
        }
        FocusPane::Steps => {
            let last_line = state.steps_line_count.saturating_sub(1);
            state.steps_cursor = if down {
                state.steps_cursor.saturating_add(amount).min(last_line)
            } else {
                state.steps_cursor.saturating_sub(amount)
            };
            state.visual.cursor = state.steps_cursor;
        }
        FocusPane::Card => {}
    }
}

fn goto_output_line(state: &mut AppState, line: u16) {
    state.materialize_output_scroll();
    state.autoscroll = false;
    state.output_cursor = line.min(state.output_line_count.saturating_sub(1));
    keep_output_cursor_visible(state);
}

fn keep_output_cursor_visible(state: &mut AppState) {
    let viewport_height = state.output_viewport_height.max(1);
    let bottom_scroll = state.output_bottom_scroll();
    if state.output_cursor < state.scroll {
        state.scroll = state.output_cursor;
    } else if state.output_cursor >= state.scroll.saturating_add(viewport_height) {
        state.scroll = state
            .output_cursor
            .saturating_sub(viewport_height.saturating_sub(1));
    }
    state.scroll = state.scroll.min(bottom_scroll);
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
        assert_eq!(state.output_cursor, 98);
        assert_eq!(state.scroll, 80);
    }

    #[test]
    fn normal_j_moves_cursor_one_line_without_wrapping() {
        let mut state = AppState::new();
        state.remember_output_view(100, 20);
        state.autoscroll = false;
        state.scroll = 80;
        state.output_cursor = 99;

        let action = handle(key(KeyCode::Char('j')), &mut state);

        assert!(matches!(action, Action::None));
        assert_eq!(state.output_cursor, 99);
        assert_eq!(state.scroll, 80);
    }

    #[test]
    fn normal_k_moves_cursor_one_line() {
        let mut state = AppState::new();
        state.remember_output_view(100, 20);
        state.autoscroll = false;
        state.scroll = 80;
        state.output_cursor = 99;

        let action = handle(key(KeyCode::Char('k')), &mut state);

        assert!(matches!(action, Action::None));
        assert_eq!(state.output_cursor, 98);
        assert_eq!(state.scroll, 80);
    }

    #[test]
    fn normal_count_gg_jumps_to_one_based_line() {
        let mut state = AppState::new();
        state.remember_output_view(100, 20);
        state.autoscroll = true;

        handle(key(KeyCode::Char('4')), &mut state);
        handle(key(KeyCode::Char('5')), &mut state);
        handle(key(KeyCode::Char('g')), &mut state);
        let action = handle(key(KeyCode::Char('g')), &mut state);

        assert!(matches!(action, Action::None));
        assert!(!state.autoscroll);
        assert_eq!(state.output_cursor, 44);
        assert_eq!(state.scroll, 44);
    }

    #[test]
    fn normal_q_does_not_quit_without_command_mode() {
        let mut state = AppState::new();

        let action = handle(key(KeyCode::Char('q')), &mut state);

        assert!(matches!(action, Action::None));
    }

    #[test]
    fn command_q_quits() {
        let mut state = AppState::new();

        handle(key(KeyCode::Char(':')), &mut state);
        handle(key(KeyCode::Char('q')), &mut state);
        let action = handle(key(KeyCode::Enter), &mut state);

        assert!(matches!(action, Action::Quit));
    }

    #[test]
    fn leader_h_opens_history() {
        let mut state = AppState::new();

        handle(key(KeyCode::Char(' ')), &mut state);
        let action = handle(key(KeyCode::Char('H')), &mut state);

        assert!(matches!(action, Action::None));
        assert!(state.history.open);
        assert_eq!(state.focus, FocusPane::Modal);
    }

    #[test]
    fn leader_c_opens_active_card() {
        let mut state = AppState::new();

        handle(key(KeyCode::Char(' ')), &mut state);
        let action = handle(key(KeyCode::Char('c')), &mut state);

        assert!(matches!(action, Action::OpenCard));
    }

    #[test]
    fn command_open_card_on_scryfall_opens_active_card() {
        let mut state = AppState::new();

        handle(key(KeyCode::Char(':')), &mut state);
        for ch in "open-card-on-scryfall".chars() {
            handle(key(KeyCode::Char(ch)), &mut state);
        }
        let action = handle(key(KeyCode::Enter), &mut state);

        assert!(matches!(action, Action::OpenCard));
    }

    #[test]
    fn visual_g_yank_sequence_selects_to_bottom() {
        let mut state = AppState::new();
        state.remember_output_view(100, 20);
        state.autoscroll = false;
        state.output_cursor = 0;

        handle(key(KeyCode::Char('v')), &mut state);
        handle(key(KeyCode::Char('G')), &mut state);
        let action = handle(key(KeyCode::Char('y')), &mut state);

        assert!(matches!(action, Action::YankVisual));
        assert_eq!(state.visual.range(), Some((0, 99)));
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
        assert_eq!(state.visual.anchor, 99);
        assert_eq!(state.visual.cursor, 99);
    }

    #[test]
    fn visual_down_extends_selection_one_line() {
        let mut state = AppState::new();
        state.remember_output_view(100, 20);
        state.scroll = 40;
        state.output_cursor = 40;
        state.autoscroll = false;
        state.visual.start(40);

        let action = handle(key(KeyCode::Char('j')), &mut state);

        assert!(matches!(action, Action::None));
        assert_eq!(state.scroll, 40);
        assert_eq!(state.visual.range(), Some((40, 41)));
    }

    #[test]
    fn visual_leader_tab_switches_focus_to_steps() {
        let mut state = AppState::new();
        state.remember_output_view(100, 20);
        state.remember_steps_view(4);
        state.output_cursor = 40;
        state.steps_cursor = 2;
        state.visual.start(40);

        handle(key(KeyCode::Char(' ')), &mut state);
        let action = handle(key(KeyCode::Tab), &mut state);

        assert!(matches!(action, Action::None));
        assert_eq!(state.focus, FocusPane::Steps);
        assert!(state.visual.active);
        assert_eq!(state.visual.range(), Some((2, 2)));
    }

    #[test]
    fn visual_steps_down_extends_step_selection() {
        let mut state = AppState::new();
        state.focus = FocusPane::Steps;
        state.remember_steps_view(4);
        state.steps_cursor = 1;
        state.visual.start(1);

        let action = handle(key(KeyCode::Char('j')), &mut state);

        assert!(matches!(action, Action::None));
        assert_eq!(state.steps_cursor, 2);
        assert_eq!(state.visual.range(), Some((1, 2)));
    }
}
