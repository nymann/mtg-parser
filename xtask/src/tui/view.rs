//! ratatui rendering. Pure function of `&AppState` → frame.

use ratatui::{
    layout::Alignment,
    layout::{Constraint, Direction, Layout, Position, Rect},
    style::{Color, Modifier, Style, Stylize},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, Paragraph, Wrap},
    Frame,
};

use crate::agent_events::ParsedAgentEvent;
use crate::flow::{AgentProvider, IterationOutcomeSummary, NoteLevel, SessionEndReason};
use crate::paths::repo_root;
use crate::tui::state::{
    format_tool_target, AppState, Iteration, SessionMeta, StepState, StepStatus, TimelineKind,
    TimelineRow,
};

// Color palette mirrors design/add-card-tui.html (mockup D).
const C_TITLE: Color = Color::White;
const C_DIM: Color = Color::Gray;
const C_FAINT: Color = Color::DarkGray;
const C_GOOD: Color = Color::Green;
const C_WARN: Color = Color::Yellow;
const C_BAD: Color = Color::Red;
const C_INFO: Color = Color::Cyan;
const C_TOOL: Color = Color::Magenta;
const C_TEXT: Color = Color::LightYellow;
const C_FILE: Color = Color::LightBlue;
const C_CMD: Color = Color::LightRed;

pub fn render(f: &mut Frame<'_>, state: &mut AppState) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title bar
            Constraint::Length(1), // session bar
            Constraint::Min(0),    // main (left + output)
            if state.search.editing || !state.search.query.is_empty() {
                Constraint::Length(1)
            } else {
                Constraint::Length(0)
            },
            Constraint::Length(1), // status bar
            Constraint::Length(1), // spacer above tmux/status lines outside the TUI
        ])
        .split(area);

    render_title_bar(f, chunks[0], state);
    render_session_bar(f, chunks[1], state);
    render_main(f, chunks[2], state);
    render_search_bar(f, chunks[3], state);
    render_status_bar(f, chunks[4], state);
    render_history_modal(f, area, state);
}

// ---------------------------------------------------------------------------
// title bar
// ---------------------------------------------------------------------------

fn render_title_bar(f: &mut Frame<'_>, area: Rect, state: &AppState) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(42),
            Constraint::Percentage(18),
            Constraint::Percentage(40),
        ])
        .split(area);

    let mut left = vec![Span::styled(
        "add-card",
        Style::default().fg(C_TITLE).add_modifier(Modifier::BOLD),
    )];
    let mut center = Vec::new();
    let mut right = Vec::new();
    if let Some(iter) = state.active_iteration() {
        if let Some(card) = &iter.card {
            left.push(Span::raw(" · "));
            left.push(Span::styled(
                card.name.clone(),
                Style::default().fg(C_TITLE).add_modifier(Modifier::BOLD),
            ));
            left.push(Span::raw(format!(
                " ({}/{})",
                card.set_code, card.collector_number
            )));
        }
        center.push(Span::styled(
            format!(
                "iter {}/{}",
                iter.index,
                format_max_iterations(iter.max_iterations)
            ),
            Style::default().fg(C_DIM),
        ));
        if let Some(step_idx) = iter.current_step {
            if let Some(step) = iter.step(step_idx) {
                let dur = step.duration_secs().unwrap_or(0);
                right.push(Span::styled(
                    format!("step {step_idx}/{} · {}", step.total, step.label),
                    Style::default().fg(C_WARN).add_modifier(Modifier::BOLD),
                ));
                right.push(Span::styled(
                    format!(" (+{dur}s)"),
                    Style::default().fg(C_WARN),
                ));
            }
        }
    } else if let Some(s) = &state.session {
        left.push(Span::raw("  "));
        left.push(Span::styled(
            format!(
                "set={} max-iter={}",
                s.set,
                format_max_iterations(s.max_iterations)
            ),
            Style::default().fg(C_DIM),
        ));
    }
    f.render_widget(Paragraph::new(Line::from(left)), cols[0]);
    f.render_widget(
        Paragraph::new(Line::from(center)).alignment(Alignment::Center),
        cols[1],
    );
    f.render_widget(
        Paragraph::new(Line::from(right)).alignment(Alignment::Right),
        cols[2],
    );
}

// ---------------------------------------------------------------------------
// session bar
// ---------------------------------------------------------------------------

fn render_session_bar(f: &mut Frame<'_>, area: Rect, state: &AppState) {
    let s = match &state.session {
        Some(s) => s,
        None => return,
    };
    let cards_done = state
        .iterations
        .iter()
        .filter(|i| i.committed_duration_secs.is_some())
        .count();
    let cards_active = state
        .iterations
        .iter()
        .filter(|i| i.outcome.is_none())
        .count();

    let avg = state.avg_iteration_secs();
    let elapsed_secs = state.elapsed().as_secs();

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(20),
            Constraint::Percentage(16),
            Constraint::Percentage(20),
            Constraint::Percentage(30),
            Constraint::Percentage(14),
        ])
        .split(area);

    render_bar_segment(
        f,
        cols[0],
        vec![
            label_span("cards"),
            Span::raw(format!("{cards_done} done · {cards_active} active")),
        ],
        Alignment::Left,
    );
    render_bar_segment(
        f,
        cols[1],
        vec![
            label_span("avg/card"),
            Span::raw(avg.map(|s| format!("{s}s")).unwrap_or_else(|| "—".into())),
        ],
        Alignment::Center,
    );
    render_bar_segment(
        f,
        cols[2],
        vec![
            label_span("grammar"),
            Span::raw(format!(
                "{}→{}",
                s.baseline_grammar_rules, s.current_grammar_rules
            )),
            delta_span(s.grammar_delta(), " rules"),
        ],
        Alignment::Center,
    );
    render_bar_segment(
        f,
        cols[3],
        vec![
            label_span("corpus"),
            Span::raw(format!(
                "{}/{}→{}/{}",
                s.baseline_corpus_passing,
                s.baseline_corpus_total,
                s.current_corpus_passing,
                s.current_corpus_total
            )),
            delta_span(s.corpus_delta(), " passes"),
        ],
        Alignment::Center,
    );
    render_bar_segment(
        f,
        cols[4],
        vec![label_span("elapsed"), Span::raw(format_secs(elapsed_secs))],
        Alignment::Right,
    );
}

fn label_span(label: &'static str) -> Span<'static> {
    Span::styled(format!("{label} "), Style::default().fg(C_FAINT))
}

fn render_bar_segment(
    f: &mut Frame<'_>,
    area: Rect,
    spans: Vec<Span<'static>>,
    alignment: Alignment,
) {
    f.render_widget(
        Paragraph::new(Line::from(spans))
            .style(Style::default().fg(C_DIM))
            .alignment(alignment),
        area,
    );
}

fn delta_span(delta: i64, suffix: &str) -> Span<'static> {
    let (txt, color) = match delta.cmp(&0) {
        std::cmp::Ordering::Greater => (format!(" +{delta}{suffix}"), C_GOOD),
        std::cmp::Ordering::Less => (format!(" {delta}{suffix}"), C_BAD),
        std::cmp::Ordering::Equal => (format!(" +0{suffix}"), C_FAINT),
    };
    Span::styled(txt, Style::default().fg(color))
}

fn format_secs(s: u64) -> String {
    let m = s / 60;
    let r = s % 60;
    if m > 0 {
        format!("{m}m {r:02}s")
    } else {
        format!("{r}s")
    }
}

// ---------------------------------------------------------------------------
// main area
// ---------------------------------------------------------------------------

fn render_main(f: &mut Frame<'_>, area: Rect, state: &mut AppState) {
    let left_width = responsive_left_width(area.width);
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(left_width), Constraint::Min(0)])
        .split(area);

    render_left_column(f, cols[0], state);
    render_output(f, cols[1], state);
}

fn responsive_left_width(total: u16) -> u16 {
    if total < 96 {
        (total / 2).clamp(44, 58)
    } else {
        (total / 3).clamp(52, 72)
    }
}

fn render_left_column(f: &mut Frame<'_>, area: Rect, state: &AppState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        // 13 lines for the step list: 1 top border + 1 title + 9 step rows
        // + 1 spacing + 1 bottom border. The card panel takes everything
        // else (Min(8) lets it shrink only when the terminal is short).
        .constraints([Constraint::Min(8), Constraint::Length(13)])
        .split(area);

    render_card_panel(f, rows[0], state);
    render_steps(f, rows[1], state);
}

fn render_card_panel(f: &mut Frame<'_>, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(C_FAINT))
        .title(copy_title("card", 'c', state.copy_mode))
        .title_style(Style::default().fg(C_DIM).add_modifier(Modifier::BOLD));

    let lines: Vec<Line> = match state.active_iteration().and_then(|i| i.card.as_ref()) {
        None => match &state.session_end {
            Some(SessionEndReason::SurfacedToHuman(reason)) => {
                let mut lines = vec![Line::from(Span::styled(
                    "  startup failed",
                    Style::default().fg(C_BAD).add_modifier(Modifier::BOLD),
                ))];
                lines.push(Line::from(""));
                for line in reason.lines() {
                    lines.push(Line::from(vec![
                        Span::styled("  │ ", Style::default().fg(C_BAD)),
                        Span::styled(line.to_string(), Style::default().fg(C_BAD)),
                    ]));
                }
                lines
            }
            _ => vec![Line::from(Span::styled(
                "  (finding next failing card…)",
                Style::default().fg(C_FAINT),
            ))],
        },
        Some(card) => {
            let iter = state.active_iteration().unwrap();
            let mut v = Vec::new();
            v.push(field("Name", &card.name, C_TITLE));
            v.push(field("Set", &card.set_code, C_DIM));
            v.push(field("Collector #", &card.collector_number, C_DIM));
            v.push(field("Layout", &format!("{:?}", card.layout), C_DIM));
            v.push(field(
                "Mana cost",
                if card.mana_cost.is_empty() {
                    "—"
                } else {
                    card.mana_cost.as_str()
                },
                C_INFO,
            ));
            v.push(Line::from(""));
            v.push(Line::from(Span::styled(
                "Oracle text",
                Style::default().fg(C_FAINT),
            )));
            for line in iter
                .normalized
                .as_deref()
                .unwrap_or(&card.oracle_text)
                .lines()
            {
                v.push(Line::from(vec![
                    Span::styled("  │ ", Style::default().fg(C_FAINT)),
                    Span::raw(line.to_string()),
                ]));
            }
            if let Some(err) = &iter.round_trip_error {
                v.push(Line::from(""));
                v.push(Line::from(Span::styled(
                    "Round-trip",
                    Style::default().fg(C_FAINT),
                )));
                for line in err.lines() {
                    v.push(Line::from(vec![
                        Span::styled("  │ ", Style::default().fg(C_BAD)),
                        Span::styled(line.to_string(), Style::default().fg(C_BAD)),
                    ]));
                }
            }
            v
        }
    };

    let paragraph = Paragraph::new(lines)
        .block(block)
        .wrap(Wrap { trim: false })
        .scroll((state.card_scroll, 0));
    f.render_widget(paragraph, area);
}

fn field(label: &str, value: &str, value_color: Color) -> Line<'static> {
    Line::from(vec![
        Span::styled(
            format!("{label:<11} ", label = label),
            Style::default().fg(C_FAINT),
        ),
        Span::styled(
            value.to_string(),
            Style::default()
                .fg(value_color)
                .add_modifier(Modifier::BOLD),
        ),
    ])
}

fn render_steps(f: &mut Frame<'_>, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(C_FAINT))
        .title(copy_title("steps", 's', state.copy_mode))
        .title_style(Style::default().fg(C_DIM).add_modifier(Modifier::BOLD));

    let mut lines = Vec::new();
    if let Some(iter) = state.active_iteration() {
        for (i, step) in iter.steps.iter().enumerate() {
            let idx = (i + 1) as u8;
            let total = step.total.max(idx);
            lines.push(step_line(idx, total, step));
        }
        if iter.steps.is_empty() {
            lines.push(Line::from(Span::styled(
                "  (no steps yet)",
                Style::default().fg(C_FAINT),
            )));
        }
    }
    let paragraph = Paragraph::new(lines).block(block);
    f.render_widget(paragraph, area);
}

fn step_line(index: u8, total: u8, step: &StepState) -> Line<'static> {
    let (icon, color) = match step.status {
        StepStatus::Pending => ("·", C_FAINT),
        StepStatus::Running => ("⟳", C_WARN),
        StepStatus::Done => ("✓", C_GOOD),
        StepStatus::Failed => ("✗", C_BAD),
    };
    let mut label_style = Style::default().fg(color);
    if step.status == StepStatus::Running {
        label_style = label_style.add_modifier(Modifier::BOLD);
    }
    if step.status == StepStatus::Done {
        // Done steps use the muted "completed" tint from the mockup
        // so the currently-running one stands out.
        label_style = Style::default().fg(Color::Rgb(0xb6, 0xcf, 0xa9));
    }
    let dur = step.duration_secs();
    let extra = match (step.status, dur) {
        (StepStatus::Running, Some(d)) => format!(" +{d}s"),
        (StepStatus::Done | StepStatus::Failed, Some(d)) if d > 0 => format!(" {d}s"),
        _ => String::new(),
    };
    // Pending slots have no label until their StepStarted arrives.
    // Show "(pending)" so the row visually communicates intent.
    let label_text = if step.label.is_empty() {
        "(pending)".to_string()
    } else {
        step.label.clone()
    };
    let resolved_style = if step.label.is_empty() {
        Style::default().fg(C_FAINT)
    } else {
        label_style
    };
    Line::from(vec![
        Span::styled(format!(" {icon} "), Style::default().fg(color)),
        Span::styled(format!("{index}/{total}"), Style::default().fg(C_FAINT)),
        Span::raw("  "),
        Span::styled(label_text, resolved_style),
        Span::styled(extra, Style::default().fg(C_FAINT)),
    ])
}

// ---------------------------------------------------------------------------
// output pane
// ---------------------------------------------------------------------------

fn render_output(f: &mut Frame<'_>, area: Rect, state: &mut AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(C_FAINT))
        .title(copy_title("output", 'o', state.copy_mode))
        .title_style(Style::default().fg(C_DIM).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = render_event_lines(state, inner.width);
    let total_lines = lines.len() as u16;
    let viewport_height = inner.height;
    state.remember_output_view(lines.len(), viewport_height);
    let scroll = if state.autoscroll {
        total_lines.saturating_sub(viewport_height)
    } else {
        state
            .scroll
            .min(total_lines.saturating_sub(viewport_height))
    };

    let paragraph = Paragraph::new(lines)
        .wrap(Wrap { trim: false })
        .scroll((scroll, 0));
    f.render_widget(paragraph, inner);
    set_output_cursor(f, inner, state, total_lines, scroll);
}

fn render_event_lines(state: &AppState, _width: u16) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    let mut current_iter: Option<u32> = None;
    let root = repo_root();
    for row in &state.events {
        if !should_render_row_for_current_iteration(state, row) {
            continue;
        }
        if state.search.filter_mode
            && !state.search.query.is_empty()
            && !row_search_text(row, &root).contains(&state.search.query.to_ascii_lowercase())
        {
            continue;
        }
        if Some(row.iteration_index) != current_iter {
            push_output_line(
                &mut out,
                state,
                iter_separator_line(state, row.iteration_index),
            );
            current_iter = Some(row.iteration_index);
        }
        for line in render_row(row, &root) {
            push_output_line(&mut out, state, line);
        }
    }
    if let Some(end) = &state.session_end {
        push_output_line(&mut out, state, Line::from(""));
        push_output_line(&mut out, state, session_end_line(end));
    }
    out
}

fn should_render_row_for_current_iteration(state: &AppState, row: &TimelineRow) -> bool {
    state
        .active_iteration()
        .map(|iter| row.iteration_index == iter.index)
        .unwrap_or(true)
}

fn set_output_cursor(
    f: &mut Frame<'_>,
    area: Rect,
    state: &AppState,
    total_lines: u16,
    scroll: u16,
) {
    if area.width == 0
        || area.height == 0
        || total_lines == 0
        || state.focus != crate::tui::state::FocusPane::Output
    {
        return;
    }
    let line = if state.visual.active {
        state.visual.cursor
    } else if state.autoscroll {
        total_lines.saturating_sub(1)
    } else {
        scroll
    }
    .min(total_lines.saturating_sub(1));
    if line < scroll || line >= scroll.saturating_add(area.height) {
        return;
    }
    f.set_cursor_position(Position::new(area.x, area.y + line.saturating_sub(scroll)));
}

fn push_output_line(out: &mut Vec<Line<'static>>, state: &AppState, line: Line<'static>) {
    let idx = out.len();
    let line = if state
        .visual
        .range()
        .is_some_and(|(start, end)| (start..=end).contains(&idx))
    {
        line.bg(Color::DarkGray)
    } else {
        line
    };
    out.push(line);
}

fn iter_separator_line(state: &AppState, iter_idx: u32) -> Line<'static> {
    let card_name = state
        .iterations
        .iter()
        .find(|i| i.index == iter_idx)
        .and_then(|i| i.card.as_ref().map(|c| c.name.clone()))
        .unwrap_or_else(|| "(no card)".to_string());
    Line::from(vec![
        Span::styled("── iteration ", Style::default().fg(C_DIM)),
        Span::styled(
            iter_idx.to_string(),
            Style::default().fg(C_TITLE).add_modifier(Modifier::BOLD),
        ),
        Span::styled(" · ", Style::default().fg(C_FAINT)),
        Span::styled(card_name, Style::default().fg(C_TITLE)),
        Span::styled(" ───", Style::default().fg(C_DIM)),
    ])
}

fn session_end_line(reason: &SessionEndReason) -> Line<'static> {
    let (label, color) = match reason {
        SessionEndReason::AllPass => ("session · all cards pass", C_GOOD),
        SessionEndReason::CorpusComplete => {
            ("session · corpus complete (no more paper sets)", C_GOOD)
        }
        SessionEndReason::DryRunStop => ("session · dry-run complete", C_DIM),
        SessionEndReason::MaxIterationsReached(n) => {
            return Line::from(Span::styled(
                format!(
                    "session · reached --max-iterations={}",
                    format_max_iterations(*n)
                ),
                Style::default().fg(C_DIM),
            ));
        }
        SessionEndReason::SurfacedToHuman(_) => ("session · STOPPED, see notes above", C_BAD),
    };
    Line::from(Span::styled(
        label.to_string(),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    ))
}

fn render_row(row: &TimelineRow, repo: &std::path::Path) -> Vec<Line<'static>> {
    match &row.kind {
        TimelineKind::StepHeader { index, label } => vec![Line::from(vec![
            Span::styled("  step ", Style::default().fg(C_DIM)),
            Span::styled(
                format!("{index}"),
                Style::default().fg(C_FAINT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" · "),
            Span::styled(
                label.clone(),
                Style::default().fg(C_TITLE).add_modifier(Modifier::BOLD),
            ),
        ])],
        TimelineKind::StepResult { ok, summary } => {
            let color = if *ok { C_GOOD } else { C_BAD };
            vec![Line::from(vec![
                Span::styled("    ", Style::default()),
                Span::styled(if *ok { "→ " } else { "✗ " }, Style::default().fg(color)),
                Span::styled(summary.clone(), Style::default().fg(color)),
            ])]
        }
        TimelineKind::Note { level, text } => {
            let color = match level {
                NoteLevel::Info => C_DIM,
                NoteLevel::Warn => C_WARN,
                NoteLevel::Error => C_BAD,
            };
            let mut out = Vec::new();
            for (i, line) in text.lines().enumerate() {
                out.push(if i == 0 {
                    Line::from(vec![
                        delta_cell(row.delta),
                        kind_cell("note", color),
                        Span::raw(line.to_string()),
                    ])
                } else {
                    Line::from(vec![
                        Span::raw("                "),
                        Span::styled(line.to_string(), Style::default().fg(color)),
                    ])
                });
            }
            out
        }
        TimelineKind::Agent { provider, parsed } => render_agent_row(row, *provider, parsed, repo),
        TimelineKind::IterationFooter(outcome) => {
            let (text, color) = match outcome {
                IterationOutcomeSummary::Committed {
                    new_passes,
                    corpus_passing,
                    corpus_total,
                    grammar_rules,
                    duration_secs,
                } => (
                    format!(
                        "  iter · committed · +{new_passes} pass · status {corpus_passing}/{corpus_total} · {grammar_rules} rules · {duration_secs}s"
                    ),
                    C_WARN,
                ),
                IterationOutcomeSummary::SurfacedToHuman { reason } => {
                    (format!("  iter · STOPPED · {reason}"), C_BAD)
                }
            };
            vec![Line::from(Span::styled(
                text,
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            ))]
        }
    }
}

fn render_agent_row(
    row: &TimelineRow,
    provider: AgentProvider,
    parsed: &ParsedAgentEvent,
    repo: &std::path::Path,
) -> Vec<Line<'static>> {
    let label = provider.label();
    match parsed {
        ParsedAgentEvent::Init { model } => vec![Line::from(vec![
            delta_cell(row.delta),
            kind_cell(&format!("{label} init"), C_INFO),
            Span::styled(format!("model={model}"), Style::default().fg(C_DIM)),
        ])],
        ParsedAgentEvent::AssistantText { text } => {
            let mut out = Vec::new();
            for (i, line) in text.lines().enumerate() {
                out.push(if i == 0 {
                    Line::from(vec![
                        delta_cell(row.delta),
                        kind_cell(label, C_TEXT),
                        Span::styled(line.to_string(), Style::default().fg(C_TEXT)),
                    ])
                } else {
                    Line::from(vec![
                        Span::raw("                "),
                        Span::styled(line.to_string(), Style::default().fg(C_TEXT)),
                    ])
                });
            }
            out
        }
        ParsedAgentEvent::ToolUse { name, target } => {
            let mut spans = vec![delta_cell(row.delta)];
            match target {
                crate::agent_events::ToolUseTarget::Command(command) => {
                    spans.extend(render_command_spans(command));
                }
                _ => {
                    let target_str = format_tool_target(target, repo);
                    spans.push(kind_cell(name, C_TOOL));
                    if !target_str.is_empty() {
                        spans.push(Span::styled(
                            target_str,
                            Style::default().fg(match target {
                                crate::agent_events::ToolUseTarget::File(_) => C_FILE,
                                _ => C_TOOL,
                            }),
                        ));
                    }
                }
            }
            vec![Line::from(spans)]
        }
        ParsedAgentEvent::ToolResult {
            first_line,
            is_error,
        } => vec![Line::from(vec![
            delta_cell(row.delta),
            kind_cell(
                if *is_error {
                    "tool_error"
                } else {
                    "tool_result"
                },
                if *is_error { C_BAD } else { C_FAINT },
            ),
            Span::styled(
                first_line.clone(),
                Style::default().fg(if *is_error { C_BAD } else { C_FAINT }),
            ),
        ])],
        ParsedAgentEvent::Done {
            subtype,
            num_turns,
            total_cost_usd,
        } => {
            let color = if subtype == "success" { C_GOOD } else { C_BAD };
            vec![Line::from(vec![
                delta_cell(row.delta),
                kind_cell(&format!("{label} done"), color),
                Span::styled(
                    format!("subtype={subtype} turns={num_turns} cost=${total_cost_usd:.4}"),
                    Style::default().fg(color),
                ),
            ])]
        }
        ParsedAgentEvent::Other => Vec::new(),
    }
}

fn delta_cell(delta: u64) -> Span<'static> {
    let (txt, color) = match delta {
        0 => (format!("{delta:>4}s "), C_FAINT),
        1..=2 => (format!("{delta:>4}s "), C_GOOD),
        3..=10 => (format!("{delta:>4}s "), C_WARN),
        _ => (format!("{delta:>4}s "), C_BAD),
    };
    Span::styled(txt, Style::default().fg(color))
}

fn kind_cell(name: &str, color: Color) -> Span<'static> {
    let padded = format!("[{name:<11}] ");
    Span::styled(padded, Style::default().fg(color))
}

fn render_command_spans(command: &str) -> Vec<Span<'static>> {
    let command = display_shell_command(command);
    let mut spans = vec![Span::styled(
        "$ ",
        Style::default().fg(C_BAD).add_modifier(Modifier::BOLD),
    )];
    for (i, token) in shell_tokens(&command).into_iter().enumerate() {
        if i > 0 {
            spans.push(Span::raw(" "));
        }
        let style = shell_token_style(&token, i == 0);
        spans.push(Span::styled(token, style));
    }
    spans
}

fn display_shell_command(command: &str) -> String {
    let trimmed = command.trim();
    for prefix in ["zsh -lc ", "zsh -c ", "bash -lc ", "bash -c ", "sh -c "] {
        if let Some(rest) = trimmed.strip_prefix(prefix) {
            return unquote_shell_arg(rest.trim()).to_string();
        }
    }
    trimmed.to_string()
}

fn unquote_shell_arg(s: &str) -> &str {
    let bytes = s.as_bytes();
    if bytes.len() >= 2
        && ((bytes[0] == b'\'' && bytes[bytes.len() - 1] == b'\'')
            || (bytes[0] == b'"' && bytes[bytes.len() - 1] == b'"'))
    {
        &s[1..s.len() - 1]
    } else {
        s
    }
}

fn shell_tokens(command: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut current = String::new();
    let mut quote: Option<char> = None;
    for ch in command.chars() {
        if let Some(q) = quote {
            current.push(ch);
            if ch == q {
                quote = None;
            }
            continue;
        }
        match ch {
            '\'' | '"' => {
                quote = Some(ch);
                current.push(ch);
            }
            ' ' | '\t' if !current.is_empty() => {
                out.push(std::mem::take(&mut current));
            }
            ' ' | '\t' => {}
            '|' | '&' | ';' | '<' | '>' => {
                if !current.is_empty() {
                    out.push(std::mem::take(&mut current));
                }
                out.push(ch.to_string());
            }
            _ => current.push(ch),
        }
    }
    if !current.is_empty() {
        out.push(current);
    }
    out
}

fn shell_token_style(token: &str, first: bool) -> Style {
    let color = if first || token.contains('/') {
        C_CMD
    } else if token.starts_with('-') {
        C_INFO
    } else if token.starts_with('$') || token.contains('=') {
        C_WARN
    } else if token.starts_with('"') || token.starts_with('\'') {
        C_TEXT
    } else if matches!(token, "|" | "&" | ";" | "<" | ">") {
        C_BAD
    } else {
        C_TITLE
    };
    Style::default().fg(color)
}

fn copy_title(name: &'static str, hotkey: char, active: bool) -> Line<'static> {
    let mut spans = vec![Span::raw(" ")];
    for ch in name.chars() {
        if active && ch == hotkey {
            spans.push(Span::styled(
                ch.to_string(),
                Style::default()
                    .fg(Color::Black)
                    .bg(C_WARN)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(
                ch.to_string(),
                Style::default().fg(C_DIM).add_modifier(Modifier::BOLD),
            ));
        }
    }
    spans.push(Span::raw(" "));
    Line::from(spans)
}

// ---------------------------------------------------------------------------
// status bar
// ---------------------------------------------------------------------------

fn render_status_bar(f: &mut Frame<'_>, area: Rect, state: &AppState) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage(38),
            Constraint::Percentage(38),
            Constraint::Percentage(24),
        ])
        .split(area);
    let nav = vec![
        key_span("q"),
        Span::raw(" quit  "),
        key_span("↑↓"),
        Span::raw(" scroll  "),
        key_span("p"),
        Span::raw(" pause  "),
        key_span("g/G"),
        Span::raw(" top/bottom"),
    ];
    let copy = if state.visual.active {
        vec![
            Span::styled(
                "visual ",
                Style::default().fg(C_WARN).add_modifier(Modifier::BOLD),
            ),
            key_span("j/k"),
            Span::raw(" extend  "),
            hotkey_span("y"),
            Span::raw(" yank  "),
            key_span("Esc"),
            Span::raw(" cancel"),
        ]
    } else if state.copy_mode {
        vec![
            Span::styled(
                "copy ",
                Style::default().fg(C_WARN).add_modifier(Modifier::BOLD),
            ),
            hotkey_span("c"),
            Span::raw(" card  "),
            hotkey_span("s"),
            Span::raw(" steps  "),
            hotkey_span("o"),
            Span::raw(" output  "),
            hotkey_span("a"),
            Span::raw(" all  "),
            key_span("Esc"),
            Span::raw(" cancel"),
        ]
    } else {
        vec![
            key_span("c"),
            Span::raw(" copy  "),
            key_span("/"),
            Span::raw(" search  "),
            key_span("f"),
            Span::raw(" filter  "),
            key_span("H"),
            Span::raw(" history  "),
            key_span("v"),
            Span::raw(" visual"),
        ]
    };
    let right = format!(
        "{} · {} events",
        if state.autoscroll {
            "autoscroll"
        } else {
            "paused"
        },
        state.events.len()
    );
    f.render_widget(Paragraph::new(Line::from(nav)), cols[0]);
    f.render_widget(
        Paragraph::new(Line::from(copy)).alignment(Alignment::Center),
        cols[1],
    );
    f.render_widget(
        Paragraph::new(Line::from(Span::styled(right, Style::default().fg(C_DIM))))
            .alignment(Alignment::Right),
        cols[2],
    );
}

fn key_span(key: &'static str) -> Span<'static> {
    Span::styled(
        format!(" {key} "),
        Style::default().fg(C_DIM).bg(Color::DarkGray),
    )
}

fn hotkey_span(key: &'static str) -> Span<'static> {
    Span::styled(
        format!(" {key} "),
        Style::default()
            .fg(Color::Black)
            .bg(C_WARN)
            .add_modifier(Modifier::BOLD),
    )
}

fn render_search_bar(f: &mut Frame<'_>, area: Rect, state: &AppState) {
    if area.height == 0 {
        return;
    }
    let mode = if state.search.filter_mode {
        "filter"
    } else {
        "search"
    };
    let query = if state.search.editing {
        format!("{}█", state.search.query)
    } else {
        state.search.query.clone()
    };
    let line = Line::from(vec![
        Span::styled(format!(" {mode} /"), Style::default().fg(C_WARN)),
        Span::styled(query, Style::default().fg(C_TITLE)),
        Span::styled(
            " · Enter confirm · n/N next/prev · Esc clear · f filter ",
            Style::default().fg(C_DIM),
        ),
    ]);
    let paragraph = Paragraph::new(line).style(Style::default().fg(C_DIM));
    f.render_widget(paragraph, area);
}

fn format_max_iterations(n: u32) -> String {
    if n == 0 {
        "∞".to_string()
    } else {
        n.to_string()
    }
}

fn render_history_modal(f: &mut Frame<'_>, area: Rect, state: &AppState) {
    if !state.history.open {
        return;
    }
    let width = (area.width.saturating_mul(3) / 5).max(40).min(area.width);
    let height = (area.height.saturating_mul(2) / 3).max(8).min(area.height);
    let x = area.x + area.width.saturating_sub(width) / 2;
    let y = area.y + area.height.saturating_sub(height) / 2;
    let rect = Rect::new(x, y, width, height);
    f.render_widget(Clear, rect);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(C_FAINT))
        .title(" history ")
        .title_style(Style::default().fg(C_DIM).add_modifier(Modifier::BOLD));
    let lines = if state.history.entries.is_empty() {
        vec![Line::from(Span::styled(
            "  no transcript.ndjson files found",
            Style::default().fg(C_FAINT),
        ))]
    } else {
        state
            .history
            .entries
            .iter()
            .enumerate()
            .map(|(i, entry)| {
                let style = if i == state.history.selected {
                    Style::default().fg(C_WARN).add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(C_DIM)
                };
                Line::from(vec![
                    Span::styled(
                        if i == state.history.selected {
                            "› "
                        } else {
                            "  "
                        },
                        style,
                    ),
                    Span::styled(entry.name.clone(), style),
                    Span::styled(
                        format!("  {}", entry.path.display()),
                        Style::default().fg(C_FAINT),
                    ),
                ])
            })
            .collect()
    };
    f.render_widget(Paragraph::new(lines).block(block), rect);
}

fn row_search_text(row: &TimelineRow, repo: &std::path::Path) -> String {
    let mut text = String::new();
    match &row.kind {
        TimelineKind::StepHeader { label, .. } => text.push_str(label),
        TimelineKind::StepResult { summary, .. } => text.push_str(summary),
        TimelineKind::Note { text: t, .. } => text.push_str(t),
        TimelineKind::IterationFooter(outcome) => text.push_str(&format!("{outcome:?}")),
        TimelineKind::Agent { provider, parsed } => {
            text.push_str(provider.label());
            text.push(' ');
            match parsed {
                ParsedAgentEvent::Init { model } => text.push_str(model),
                ParsedAgentEvent::AssistantText { text: t } => text.push_str(t),
                ParsedAgentEvent::ToolUse { name, target } => {
                    text.push_str(name);
                    text.push(' ');
                    text.push_str(&format_tool_target(target, repo));
                }
                ParsedAgentEvent::ToolResult { first_line, .. } => text.push_str(first_line),
                ParsedAgentEvent::Done { subtype, .. } => text.push_str(subtype),
                ParsedAgentEvent::Other => {}
            }
        }
    }
    text.to_ascii_lowercase()
}

// `Iteration` and `SessionMeta` re-exports just so the view's grammar
// doesn't need an explicit import path; rustc still inlines.
#[allow(dead_code)]
type _Iter = Iteration;
#[allow(dead_code)]
type _Meta = SessionMeta;
