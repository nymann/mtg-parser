//! ratatui rendering. Pure function of `&AppState` → frame.

use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::claude_events::ParsedClaudeEvent;
use crate::flow::{IterationOutcomeSummary, NoteLevel, SessionEndReason};
use crate::paths::repo_root;
use crate::tui::state::{
    format_tool_target, AppState, Iteration, SessionMeta, StepState, StepStatus, TimelineKind,
    TimelineRow,
};

// Color palette mirrors design/grammar-fix-tui.html (mockup D).
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

pub fn render(f: &mut Frame<'_>, state: &AppState) {
    let area = f.area();
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // title bar
            Constraint::Length(1), // session bar
            Constraint::Min(0),    // main (left + output)
            Constraint::Length(1), // status bar
        ])
        .split(area);

    render_title_bar(f, chunks[0], state);
    render_session_bar(f, chunks[1], state);
    render_main(f, chunks[2], state);
    render_status_bar(f, chunks[3], state);
}

// ---------------------------------------------------------------------------
// title bar
// ---------------------------------------------------------------------------

fn render_title_bar(f: &mut Frame<'_>, area: Rect, state: &AppState) {
    let mut spans = vec![Span::styled(
        "grammar-fix",
        Style::default().fg(C_TITLE).add_modifier(Modifier::BOLD),
    )];
    if let Some(iter) = state.active_iteration() {
        if let Some(card) = &iter.card {
            spans.push(Span::raw(" · "));
            spans.push(Span::styled(
                card.name.clone(),
                Style::default().fg(C_TITLE).add_modifier(Modifier::BOLD),
            ));
            spans.push(Span::raw(format!(
                " ({}/{})",
                card.set_code, card.collector_number
            )));
        }
        spans.push(Span::raw("   "));
        spans.push(Span::styled(
            format!("iter {}/{}", iter.index, iter.max_iterations),
            Style::default().fg(C_DIM),
        ));
        if let Some(step_idx) = iter.current_step {
            if let Some(step) = iter.step(step_idx) {
                let dur = step.duration_secs().unwrap_or(0);
                spans.push(Span::raw("  "));
                spans.push(Span::styled(
                    format!("step {step_idx}/{} · {}", step.total, step.label),
                    Style::default().fg(C_WARN).add_modifier(Modifier::BOLD),
                ));
                spans.push(Span::styled(
                    format!(" (+{dur}s)"),
                    Style::default().fg(C_WARN),
                ));
            }
        }
    } else if let Some(s) = &state.session {
        spans.push(Span::raw("  "));
        spans.push(Span::styled(
            format!("set={} max-iter={}", s.set, s.max_iterations),
            Style::default().fg(C_DIM),
        ));
    }
    let paragraph = Paragraph::new(Line::from(spans)).style(Style::default().bg(Color::Reset));
    f.render_widget(paragraph, area);
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

    let spans: Vec<Span> = vec![
        Span::styled("cards", Style::default().fg(C_FAINT)),
        Span::raw(" "),
        Span::raw(format!("{cards_done} done · {cards_active} active")),
        sep(),
        Span::styled("avg/card", Style::default().fg(C_FAINT)),
        Span::raw(" "),
        Span::raw(avg.map(|s| format!("{s}s")).unwrap_or_else(|| "—".into())),
        sep(),
        Span::styled("grammar", Style::default().fg(C_FAINT)),
        Span::raw(" "),
        Span::raw(format!(
            "{}→{}",
            s.baseline_grammar_rules, s.current_grammar_rules
        )),
        delta_span(s.grammar_delta(), " rules"),
        sep(),
        Span::styled("corpus", Style::default().fg(C_FAINT)),
        Span::raw(" "),
        Span::raw(format!(
            "{}/{}→{}/{}",
            s.baseline_corpus_passing,
            s.baseline_corpus_total,
            s.current_corpus_passing,
            s.current_corpus_total
        )),
        delta_span(s.corpus_delta(), " passes"),
        sep(),
        Span::styled("elapsed", Style::default().fg(C_FAINT)),
        Span::raw(" "),
        Span::raw(format_secs(elapsed_secs)),
    ];

    let paragraph = Paragraph::new(Line::from(spans)).style(Style::default().fg(C_DIM));
    f.render_widget(paragraph, area);
}

fn sep() -> Span<'static> {
    Span::styled(" · ", Style::default().fg(C_FAINT))
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

fn render_main(f: &mut Frame<'_>, area: Rect, state: &AppState) {
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Length(38), Constraint::Min(0)])
        .split(area);

    render_left_column(f, cols[0], state);
    render_output(f, cols[1], state);
}

fn render_left_column(f: &mut Frame<'_>, area: Rect, state: &AppState) {
    let rows = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(8), Constraint::Length(12)])
        .split(area);

    render_card_panel(f, rows[0], state);
    render_steps(f, rows[1], state);
}

fn render_card_panel(f: &mut Frame<'_>, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(C_FAINT))
        .title(" card ")
        .title_style(Style::default().fg(C_DIM).add_modifier(Modifier::BOLD));

    let lines: Vec<Line> = match state.active_iteration().and_then(|i| i.card.as_ref()) {
        None => vec![Line::from(Span::styled(
            "  (finding next failing card…)",
            Style::default().fg(C_FAINT),
        ))],
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
        .wrap(Wrap { trim: false });
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
        .title(" steps ")
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
    let label_style = Style::default().fg(color);
    let label_style = if step.status == StepStatus::Running {
        label_style.add_modifier(Modifier::BOLD)
    } else {
        label_style
    };
    let dur = step.duration_secs();
    let extra = match (step.status, dur) {
        (StepStatus::Running, Some(d)) => format!(" +{d}s"),
        (StepStatus::Done | StepStatus::Failed, Some(d)) if d > 0 => format!(" {d}s"),
        _ => String::new(),
    };
    Line::from(vec![
        Span::styled(format!(" {icon} "), Style::default().fg(color)),
        Span::styled(format!("{index}/{total}"), Style::default().fg(C_FAINT)),
        Span::raw("  "),
        Span::styled(step.label.clone(), label_style),
        Span::styled(extra, Style::default().fg(C_FAINT)),
    ])
}

// ---------------------------------------------------------------------------
// output pane
// ---------------------------------------------------------------------------

fn render_output(f: &mut Frame<'_>, area: Rect, state: &AppState) {
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(C_FAINT))
        .title(" output ")
        .title_style(Style::default().fg(C_DIM).add_modifier(Modifier::BOLD));

    let inner = block.inner(area);
    f.render_widget(block, area);

    let lines = render_event_lines(state, inner.width);
    let total_lines = lines.len() as u16;
    let viewport_height = inner.height;
    let scroll = if state.autoscroll {
        total_lines.saturating_sub(viewport_height)
    } else {
        state
            .scroll
            .min(total_lines.saturating_sub(viewport_height))
    };

    let paragraph = Paragraph::new(lines).scroll((scroll, 0));
    f.render_widget(paragraph, inner);
}

fn render_event_lines(state: &AppState, _width: u16) -> Vec<Line<'static>> {
    let mut out = Vec::new();
    let mut current_iter: Option<u32> = None;
    let root = repo_root();
    for row in &state.events {
        if Some(row.iteration_index) != current_iter {
            out.push(iter_separator_line(state, row.iteration_index));
            current_iter = Some(row.iteration_index);
        }
        for line in render_row(row, &root) {
            out.push(line);
        }
    }
    if let Some(end) = &state.session_end {
        out.push(Line::from(""));
        out.push(session_end_line(end));
    }
    out
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
        SessionEndReason::DryRunStop => ("session · dry-run complete", C_DIM),
        SessionEndReason::MaxIterationsReached(n) => {
            return Line::from(Span::styled(
                format!("session · reached --max-iterations={n}"),
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
        TimelineKind::Claude(parsed) => render_claude_row(row, parsed, repo),
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

fn render_claude_row(
    row: &TimelineRow,
    parsed: &ParsedClaudeEvent,
    repo: &std::path::Path,
) -> Vec<Line<'static>> {
    match parsed {
        ParsedClaudeEvent::Init { model } => vec![Line::from(vec![
            delta_cell(row.delta),
            kind_cell("claude init", C_INFO),
            Span::styled(format!("model={model}"), Style::default().fg(C_DIM)),
        ])],
        ParsedClaudeEvent::AssistantText { text } => {
            let mut out = Vec::new();
            for (i, line) in text.lines().enumerate() {
                out.push(if i == 0 {
                    Line::from(vec![
                        delta_cell(row.delta),
                        kind_cell("claude", C_TEXT),
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
        ParsedClaudeEvent::ToolUse { name, target } => {
            let target_str = format_tool_target(target, repo);
            let mut spans = vec![
                delta_cell(row.delta),
                kind_cell("tool_use", C_TOOL),
                Span::styled(
                    name.clone(),
                    Style::default().fg(C_TOOL).add_modifier(Modifier::BOLD),
                ),
            ];
            if !target_str.is_empty() {
                spans.push(Span::raw(" "));
                let color = match target {
                    crate::claude_events::ToolUseTarget::Command(_) => C_CMD,
                    crate::claude_events::ToolUseTarget::File(_) => C_FILE,
                    _ => Color::Reset,
                };
                spans.push(Span::styled(target_str, Style::default().fg(color)));
            }
            vec![Line::from(spans)]
        }
        ParsedClaudeEvent::ToolResult {
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
        ParsedClaudeEvent::Done {
            subtype,
            num_turns,
            total_cost_usd,
        } => {
            let color = if subtype == "success" { C_GOOD } else { C_BAD };
            vec![Line::from(vec![
                delta_cell(row.delta),
                kind_cell("claude done", color),
                Span::styled(
                    format!("subtype={subtype} turns={num_turns} cost=${total_cost_usd:.4}"),
                    Style::default().fg(color),
                ),
            ])]
        }
        ParsedClaudeEvent::Other => Vec::new(),
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

// ---------------------------------------------------------------------------
// status bar
// ---------------------------------------------------------------------------

fn render_status_bar(f: &mut Frame<'_>, area: Rect, state: &AppState) {
    let left = vec![
        Span::styled(" q ", Style::default().fg(C_DIM).bg(Color::DarkGray)),
        Span::raw(" quit · "),
        Span::styled(" ↑↓ ", Style::default().fg(C_DIM).bg(Color::DarkGray)),
        Span::raw(" scroll · "),
        Span::styled(" p ", Style::default().fg(C_DIM).bg(Color::DarkGray)),
        Span::raw(" pause autoscroll · "),
        Span::styled(" g/G ", Style::default().fg(C_DIM).bg(Color::DarkGray)),
        Span::raw(" top/bottom"),
    ];
    let right = format!(
        "{} · {} events",
        if state.autoscroll {
            "autoscroll"
        } else {
            "paused"
        },
        state.events.len()
    );
    let line = Line::from(
        left.into_iter()
            .chain(std::iter::once(Span::raw("  ")))
            .chain(std::iter::once(Span::styled(
                right,
                Style::default().fg(C_DIM),
            )))
            .collect::<Vec<_>>(),
    );
    let paragraph = Paragraph::new(line).style(Style::default().bg(Color::Reset).fg(C_DIM));
    f.render_widget(paragraph, area);
}

// `Iteration` and `SessionMeta` re-exports just so the view's grammar
// doesn't need an explicit import path; rustc still inlines.
#[allow(dead_code)]
type _Iter = Iteration;
#[allow(dead_code)]
type _Meta = SessionMeta;
