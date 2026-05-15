//! TUI state model. The orchestrator's `FlowEvent`s feed [`AppState::apply`].
//! Rendering reads `&AppState`; input handling mutates it.
//!
//! Intentionally decoupled from `grammar_fix.rs` — this module only
//! knows about `FlowEvent` (a value type in `crate::flow`). Reordering
//! steps or adding new ones in `grammar_fix.rs` doesn't require any
//! change here unless the event vocabulary itself grows.

use std::time::{Duration, Instant};

use mtg_scryfall::Card;
use serde_json::json;

use crate::agent_events::{self, ParsedAgentEvent, ToolUseTarget};
use crate::flow::{AgentProvider, FlowEvent, IterationOutcomeSummary, NoteLevel, SessionEndReason};

#[derive(Default)]
pub struct AppState {
    pub session: Option<SessionMeta>,
    pub iterations: Vec<Iteration>,
    pub events: Vec<TimelineRow>,
    pub session_end: Option<SessionEndReason>,
    pub orchestrator_done: bool,

    // view state
    pub scroll: u16, // first row visible in the output pane
    pub card_scroll: u16,
    pub output_line_count: u16,
    pub output_viewport_height: u16,
    pub autoscroll: bool,
    pub focus: FocusPane,
    pub search: SearchState,
    pub history: HistoryState,
    pub copy_mode: bool,
    pub visual: VisualState,
}

impl AppState {
    pub fn new() -> Self {
        Self {
            autoscroll: true,
            ..Default::default()
        }
    }

    pub fn active_iteration(&self) -> Option<&Iteration> {
        self.iterations.last()
    }

    /// Total wall time since the session started. Used in the session bar.
    pub fn elapsed(&self) -> Duration {
        self.session
            .as_ref()
            .map(|s| s.started_at.elapsed())
            .unwrap_or_default()
    }

    /// Mean iteration duration over completed (committed) iterations.
    pub fn avg_iteration_secs(&self) -> Option<u64> {
        let durations: Vec<u64> = self
            .iterations
            .iter()
            .filter_map(|i| i.committed_duration_secs)
            .collect();
        if durations.is_empty() {
            None
        } else {
            Some(durations.iter().sum::<u64>() / durations.len() as u64)
        }
    }

    /// Plain-text content for the output pane, used by the copy shortcut.
    pub fn output_text(&self) -> String {
        self.visible_output_text()
    }

    fn visible_output_text(&self) -> String {
        let repo = crate::paths::repo_root();
        let mut lines = Vec::new();
        let mut current_iter = None;
        for row in &self.events {
            if !self.row_is_visible_iteration(row) {
                continue;
            }
            if Some(row.iteration_index) != current_iter {
                lines.push(output_iteration_separator(self, row.iteration_index));
                current_iter = Some(row.iteration_index);
            }
            lines.extend(output_row_lines(row, &repo));
        }
        if let Some(end) = &self.session_end {
            lines.push(String::new());
            lines.push(output_session_end(end));
        }
        lines.join("\n")
    }

    fn row_is_visible_iteration(&self, row: &TimelineRow) -> bool {
        self.active_iteration()
            .map(|iter| row.iteration_index == iter.index)
            .unwrap_or(true)
    }

    pub fn remember_output_view(&mut self, line_count: usize, viewport_height: u16) {
        self.output_line_count = line_count.min(u16::MAX as usize) as u16;
        self.output_viewport_height = viewport_height;
    }

    pub fn output_bottom_scroll(&self) -> u16 {
        self.output_line_count
            .saturating_sub(self.output_viewport_height)
    }

    pub fn pause_output(&mut self) {
        self.scroll = self.output_bottom_scroll();
        self.autoscroll = false;
    }

    pub fn visual_text(&self) -> String {
        let Some((start, end)) = self.visual.range() else {
            return String::new();
        };
        self.visible_output_text()
            .lines()
            .skip(start)
            .take(end.saturating_sub(start) + 1)
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn card_text(&self) -> String {
        let Some(iter) = self.active_iteration() else {
            return match &self.session_end {
                Some(SessionEndReason::SurfacedToHuman(reason)) => {
                    format!("startup failed\n\n{reason}")
                }
                _ => "finding next failing card".to_string(),
            };
        };
        let Some(card) = &iter.card else {
            return "finding next failing card".to_string();
        };
        let mut lines = vec![
            format!("Name: {}", card.name),
            format!("Set: {}", card.set_code),
            format!("Collector #: {}", card.collector_number),
            format!("Layout: {:?}", card.layout),
            format!(
                "Mana cost: {}",
                if card.mana_cost.is_empty() {
                    "-"
                } else {
                    card.mana_cost.as_str()
                }
            ),
            String::new(),
            "Oracle text:".to_string(),
        ];
        for line in iter
            .normalized
            .as_deref()
            .unwrap_or(&card.oracle_text)
            .lines()
        {
            lines.push(format!("  {line}"));
        }
        if let Some(err) = &iter.round_trip_error {
            lines.push(String::new());
            lines.push("Round-trip:".to_string());
            lines.extend(err.lines().map(|line| format!("  {line}")));
        }
        lines.join("\n")
    }

    pub fn steps_text(&self) -> String {
        let Some(iter) = self.active_iteration() else {
            return String::new();
        };
        iter.steps
            .iter()
            .enumerate()
            .map(|(i, step)| {
                let idx = i + 1;
                let label = if step.label.is_empty() {
                    "(pending)"
                } else {
                    step.label.as_str()
                };
                let status = match step.status {
                    StepStatus::Pending => "pending",
                    StepStatus::Running => "running",
                    StepStatus::Done => "done",
                    StepStatus::Failed => "failed",
                };
                match &step.summary {
                    Some(summary) => format!("{idx}. {label} [{status}] -> {summary}"),
                    None => format!("{idx}. {label} [{status}]"),
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    pub fn all_json_text(&self) -> String {
        let iter = self.active_iteration();
        let steps = iter
            .map(|iter| {
                iter.steps
                    .iter()
                    .enumerate()
                    .map(|(i, step)| {
                        json!({
                            "index": i + 1,
                            "label": step.label,
                            "status": match step.status {
                                StepStatus::Pending => "pending",
                                StepStatus::Running => "running",
                                StepStatus::Done => "done",
                                StepStatus::Failed => "failed",
                            },
                            "summary": step.summary,
                            "duration_secs": step.duration_secs(),
                        })
                    })
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        let value = json!({
            "card": iter.and_then(|iter| iter.card.as_ref()),
            "steps": steps,
            "output": self.output_text(),
            "session_end": self.session_end.as_ref().map(output_session_end),
        });
        serde_json::to_string_pretty(&value).expect("TUI copy JSON should serialize")
    }

    pub fn push_ui_note(&mut self, text: impl Into<String>) {
        self.events.push(TimelineRow {
            iteration_index: self.iterations.len() as u32,
            delta: 0,
            kind: TimelineKind::Note {
                level: NoteLevel::Info,
                text: text.into(),
            },
        });
    }

    /// Apply one event from the orchestrator. Pure state transition.
    pub fn apply(&mut self, event: FlowEvent) {
        match event {
            FlowEvent::SessionStarted {
                set,
                max_iterations,
                baseline_corpus_passing,
                baseline_corpus_total,
                baseline_grammar_rules,
            } => {
                self.session = Some(SessionMeta {
                    set,
                    max_iterations,
                    baseline_corpus_passing,
                    baseline_corpus_total,
                    baseline_grammar_rules,
                    current_corpus_passing: baseline_corpus_passing,
                    current_corpus_total: baseline_corpus_total,
                    current_grammar_rules: baseline_grammar_rules,
                    started_at: Instant::now(),
                });
            }

            FlowEvent::StepStarted {
                index,
                total,
                label,
            } => {
                if let Some(iter) = self.iterations.last_mut() {
                    iter.set_step(
                        index,
                        StepState {
                            label: label.clone(),
                            status: StepStatus::Running,
                            started_at: Some(Instant::now()),
                            finished_at: None,
                            summary: None,
                            total,
                        },
                    );
                    iter.current_step = Some(index);
                    iter.last_event_at = Some(Instant::now());
                } else {
                    // Pre-iteration steps (step 1 fires before the
                    // iteration is created). Buffer them in a synthetic
                    // "pending iteration" so they still show up.
                    self.iterations.push(Iteration::pending());
                    self.iterations.last_mut().unwrap().set_step(
                        index,
                        StepState {
                            label: label.clone(),
                            status: StepStatus::Running,
                            started_at: Some(Instant::now()),
                            finished_at: None,
                            summary: None,
                            total,
                        },
                    );
                    self.iterations.last_mut().unwrap().current_step = Some(index);
                    self.iterations.last_mut().unwrap().last_event_at = Some(Instant::now());
                }
                self.events.push(TimelineRow::step_started(
                    self.iterations.len() as u32,
                    index,
                    label,
                ));
            }

            FlowEvent::StepFinished { index, ok, summary } => {
                if let Some(iter) = self.iterations.last_mut() {
                    if let Some(step) = iter.step_mut(index) {
                        step.status = if ok {
                            StepStatus::Done
                        } else {
                            StepStatus::Failed
                        };
                        step.finished_at = Some(Instant::now());
                        step.summary = summary.clone();
                    }
                    if iter.current_step == Some(index) {
                        iter.current_step = None;
                    }
                }
                if let Some(s) = summary {
                    self.events.push(TimelineRow::step_finished(
                        self.iterations.len() as u32,
                        index,
                        ok,
                        s,
                    ));
                }
            }

            FlowEvent::IterationStarted {
                index,
                max_iterations,
                card,
                normalized,
                round_trip_error,
            } => {
                let started_at = Instant::now();
                // If we have a pending iteration (created by an early
                // StepStarted), upgrade it. Otherwise push a fresh one.
                if let Some(last) = self.iterations.last_mut() {
                    if last.is_pending() {
                        last.index = index;
                        last.max_iterations = max_iterations;
                        last.card = Some(card);
                        last.normalized = Some(normalized);
                        last.round_trip_error = Some(round_trip_error);
                        last.started_at = started_at;
                        return;
                    }
                }
                self.iterations.push(Iteration {
                    index,
                    max_iterations,
                    card: Some(card),
                    normalized: Some(normalized),
                    round_trip_error: Some(round_trip_error),
                    started_at,
                    ..Iteration::default()
                });
            }

            FlowEvent::Note { level, text } => {
                self.events.push(TimelineRow {
                    iteration_index: self.iterations.len() as u32,
                    delta: self.delta_since_last(),
                    kind: TimelineKind::Note { level, text },
                });
                self.bump_last_event();
            }

            FlowEvent::AgentEvent {
                provider,
                raw,
                elapsed_secs,
            } => {
                let _ = elapsed_secs; // computed locally via delta
                let delta = self.delta_since_last();
                for parsed in agent_events::parse(provider, &raw) {
                    if self.is_duplicate_agent_done(provider, &parsed) {
                        continue;
                    }
                    self.events.push(TimelineRow {
                        iteration_index: self.iterations.len() as u32,
                        delta,
                        kind: TimelineKind::Agent { provider, parsed },
                    });
                }
                self.bump_last_event();
            }

            FlowEvent::IterationFinished { index, outcome } => {
                if let Some(iter) = self.iterations.last_mut() {
                    iter.outcome = Some(outcome.clone());
                    if let IterationOutcomeSummary::Committed { duration_secs, .. } = &outcome {
                        iter.committed_duration_secs = Some(*duration_secs);
                    }
                }
                if let IterationOutcomeSummary::Committed {
                    new_passes,
                    corpus_passing,
                    corpus_total,
                    grammar_rules,
                    ..
                } = &outcome
                {
                    if let Some(s) = self.session.as_mut() {
                        s.current_corpus_passing = *corpus_passing;
                        s.current_corpus_total = *corpus_total;
                        s.current_grammar_rules = *grammar_rules;
                    }
                    let _ = new_passes;
                }
                self.events.push(TimelineRow {
                    iteration_index: index,
                    delta: 0,
                    kind: TimelineKind::IterationFooter(outcome),
                });
            }

            FlowEvent::SessionFinished { reason } => {
                self.session_end = Some(reason);
                self.orchestrator_done = true;
            }
        }
    }

    fn delta_since_last(&self) -> u64 {
        let iter = match self.iterations.last() {
            Some(i) => i,
            None => return 0,
        };
        match iter.last_event_at {
            Some(t) => t.elapsed().as_secs(),
            None => 0,
        }
    }

    fn bump_last_event(&mut self) {
        if let Some(iter) = self.iterations.last_mut() {
            iter.last_event_at = Some(Instant::now());
        }
    }

    fn is_duplicate_agent_done(&self, provider: AgentProvider, parsed: &ParsedAgentEvent) -> bool {
        let ParsedAgentEvent::Done {
            subtype,
            num_turns,
            total_cost_usd,
        } = parsed
        else {
            return false;
        };
        matches!(
            self.events.last().map(|row| &row.kind),
            Some(TimelineKind::Agent {
                provider: last_provider,
                parsed: ParsedAgentEvent::Done {
                    subtype: last_subtype,
                    num_turns: last_num_turns,
                    total_cost_usd: last_total_cost_usd,
                },
            }) if *last_provider == provider
                && last_subtype == subtype
                && last_num_turns == num_turns
                && (*last_total_cost_usd - *total_cost_usd).abs() < f64::EPSILON
        )
    }
}

#[derive(Debug, Default, Clone)]
pub struct VisualState {
    pub active: bool,
    pub anchor: u16,
    pub cursor: u16,
}

impl VisualState {
    pub fn start(&mut self, line: u16) {
        self.active = true;
        self.anchor = line;
        self.cursor = line;
    }

    pub fn cancel(&mut self) {
        self.active = false;
    }

    pub fn range(&self) -> Option<(usize, usize)> {
        if !self.active {
            return None;
        }
        let start = self.anchor.min(self.cursor) as usize;
        let end = self.anchor.max(self.cursor) as usize;
        Some((start, end))
    }
}

#[derive(Debug, Clone)]
pub struct SessionMeta {
    pub set: String,
    pub max_iterations: u32,
    pub baseline_corpus_passing: usize,
    pub baseline_corpus_total: usize,
    pub baseline_grammar_rules: usize,
    pub current_corpus_passing: usize,
    pub current_corpus_total: usize,
    pub current_grammar_rules: usize,
    pub started_at: Instant,
}

impl SessionMeta {
    pub fn corpus_delta(&self) -> i64 {
        self.current_corpus_passing as i64 - self.baseline_corpus_passing as i64
    }
    pub fn grammar_delta(&self) -> i64 {
        self.current_grammar_rules as i64 - self.baseline_grammar_rules as i64
    }
}

#[derive(Debug, Clone)]
pub struct Iteration {
    pub index: u32,
    pub max_iterations: u32,
    pub card: Option<Card>,
    pub normalized: Option<String>,
    pub round_trip_error: Option<String>,
    pub steps: Vec<StepState>,
    pub current_step: Option<u8>,
    pub started_at: Instant,
    pub last_event_at: Option<Instant>,
    pub outcome: Option<IterationOutcomeSummary>,
    pub committed_duration_secs: Option<u64>,
}

impl Default for Iteration {
    fn default() -> Self {
        Iteration {
            index: 0,
            max_iterations: 0,
            card: None,
            normalized: None,
            round_trip_error: None,
            steps: Vec::new(),
            current_step: None,
            started_at: Instant::now(),
            last_event_at: None,
            outcome: None,
            committed_duration_secs: None,
        }
    }
}

impl Iteration {
    fn pending() -> Self {
        Iteration {
            started_at: Instant::now(),
            ..Default::default()
        }
    }

    pub fn is_pending(&self) -> bool {
        self.card.is_none() && self.outcome.is_none()
    }

    fn set_step(&mut self, index: u8, state: StepState) {
        // Use the StepState's `total` field as the target Vec length —
        // this lets the timeline render all 9 step slots as Pending
        // before they start, instead of only the ones that have fired.
        let i = index.saturating_sub(1) as usize;
        let needed = state.total.max(index) as usize;
        let target = self.steps.len().max(needed).max(i + 1);
        if self.steps.len() < target {
            // Preserve any totals so pending slots also know "/9".
            let total = state.total;
            self.steps
                .resize_with(target, || StepState::pending_with_total(total));
        }
        self.steps[i] = state;
    }

    fn step_mut(&mut self, index: u8) -> Option<&mut StepState> {
        let i = index.saturating_sub(1) as usize;
        self.steps.get_mut(i)
    }

    pub fn step(&self, index: u8) -> Option<&StepState> {
        let i = index.saturating_sub(1) as usize;
        self.steps.get(i)
    }
}

#[derive(Debug, Clone, Default)]
pub struct StepState {
    pub label: String,
    pub status: StepStatus,
    pub started_at: Option<Instant>,
    pub finished_at: Option<Instant>,
    pub summary: Option<String>,
    pub total: u8,
}

impl StepState {
    pub fn pending_with_total(total: u8) -> Self {
        StepState {
            label: String::new(),
            status: StepStatus::Pending,
            started_at: None,
            finished_at: None,
            summary: None,
            total,
        }
    }

    pub fn duration_secs(&self) -> Option<u64> {
        match (self.started_at, self.finished_at) {
            (Some(s), Some(f)) => Some(f.duration_since(s).as_secs()),
            (Some(s), None) => Some(s.elapsed().as_secs()),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum StepStatus {
    #[default]
    Pending,
    Running,
    Done,
    Failed,
}

#[derive(Debug, Clone)]
pub struct TimelineRow {
    pub iteration_index: u32,
    /// Seconds since the previous row in this iteration. Used by the
    /// view for the delta column + color bucketing.
    pub delta: u64,
    pub kind: TimelineKind,
}

impl TimelineRow {
    fn step_started(iter: u32, index: u8, label: String) -> Self {
        TimelineRow {
            iteration_index: iter,
            delta: 0,
            kind: TimelineKind::StepHeader { index, label },
        }
    }
    fn step_finished(iter: u32, _index: u8, ok: bool, summary: String) -> Self {
        TimelineRow {
            iteration_index: iter,
            delta: 0,
            kind: TimelineKind::StepResult { ok, summary },
        }
    }
}

#[derive(Debug, Clone)]
pub enum TimelineKind {
    StepHeader {
        index: u8,
        label: String,
    },
    StepResult {
        ok: bool,
        summary: String,
    },
    Agent {
        provider: AgentProvider,
        parsed: ParsedAgentEvent,
    },
    Note {
        level: NoteLevel,
        text: String,
    },
    IterationFooter(IterationOutcomeSummary),
}

fn output_iteration_separator(state: &AppState, iter_idx: u32) -> String {
    let card_name = state
        .iterations
        .iter()
        .find(|i| i.index == iter_idx)
        .and_then(|i| i.card.as_ref().map(|c| c.name.clone()))
        .unwrap_or_else(|| "(no card)".to_string());
    format!("-- iteration {iter_idx} · {card_name} --")
}

fn output_row_lines(row: &TimelineRow, repo: &std::path::Path) -> Vec<String> {
    match &row.kind {
        TimelineKind::StepHeader { index, label } => vec![format!("step {index} · {label}")],
        TimelineKind::StepResult { ok, summary } => {
            vec![format!("{} {summary}", if *ok { "->" } else { "x" })]
        }
        TimelineKind::Note { level, text } => {
            let prefix = match level {
                NoteLevel::Info => "note",
                NoteLevel::Warn => "warn",
                NoteLevel::Error => "error",
            };
            text.lines()
                .enumerate()
                .map(|(i, line)| {
                    if i == 0 {
                        format_timed_row(row.delta, prefix, line)
                    } else {
                        format!("                {line}")
                    }
                })
                .collect()
        }
        TimelineKind::Agent { provider, parsed } => output_agent_lines(row, *provider, parsed, repo),
        TimelineKind::IterationFooter(outcome) => vec![match outcome {
            IterationOutcomeSummary::Committed {
                new_passes,
                corpus_passing,
                corpus_total,
                grammar_rules,
                duration_secs,
            } => format!(
                "iter · committed · +{new_passes} pass · status {corpus_passing}/{corpus_total} · {grammar_rules} rules · {duration_secs}s"
            ),
            IterationOutcomeSummary::SurfacedToHuman { reason } => {
                format!("iter · STOPPED · {reason}")
            }
        }],
    }
}

fn output_agent_lines(
    row: &TimelineRow,
    provider: AgentProvider,
    parsed: &ParsedAgentEvent,
    repo: &std::path::Path,
) -> Vec<String> {
    let label = provider.label();
    match parsed {
        ParsedAgentEvent::Init { model } => {
            vec![format_timed_row(
                row.delta,
                &format!("{label} init"),
                &format!("model={model}"),
            )]
        }
        ParsedAgentEvent::AssistantText { text } => text
            .lines()
            .enumerate()
            .map(|(i, line)| {
                if i == 0 {
                    format_timed_row(row.delta, label, line)
                } else {
                    format!("                {line}")
                }
            })
            .collect(),
        ParsedAgentEvent::ToolUse { name, target } => {
            let target = format_tool_target(target, repo);
            let detail = if target.is_empty() {
                name.clone()
            } else {
                format!("{name} {target}")
            };
            vec![format_timed_row(row.delta, "tool_use", &detail)]
        }
        ParsedAgentEvent::ToolResult {
            first_line,
            is_error,
        } => vec![format_timed_row(
            row.delta,
            if *is_error {
                "tool_error"
            } else {
                "tool_result"
            },
            first_line,
        )],
        ParsedAgentEvent::Done {
            subtype,
            num_turns,
            total_cost_usd,
        } => vec![format_timed_row(
            row.delta,
            &format!("{label} done"),
            &format!("subtype={subtype} turns={num_turns} cost=${total_cost_usd:.4}"),
        )],
        ParsedAgentEvent::Other => Vec::new(),
    }
}

fn format_timed_row(delta: u64, label: &str, text: &str) -> String {
    format!("{delta:>4}s [{label:<11}] {text}")
}

fn output_session_end(reason: &SessionEndReason) -> String {
    match reason {
        SessionEndReason::AllPass => "session · all cards pass".to_string(),
        SessionEndReason::DryRunStop => "session · dry-run complete".to_string(),
        SessionEndReason::MaxIterationsReached(n) => {
            format!(
                "session · reached --max-iterations={}",
                format_max_iterations(*n)
            )
        }
        SessionEndReason::SurfacedToHuman(_) => "session · STOPPED, see notes above".to_string(),
    }
}

fn format_max_iterations(n: u32) -> String {
    if n == 0 {
        "∞".to_string()
    } else {
        n.to_string()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum FocusPane {
    #[default]
    Output,
    Card,
    Modal,
}

#[derive(Debug, Clone, Default)]
pub struct SearchState {
    pub query: String,
    pub editing: bool,
    pub filter_mode: bool,
    pub current_match: usize,
}

#[derive(Debug, Clone, Default)]
pub struct HistoryState {
    pub open: bool,
    pub entries: Vec<HistoryEntry>,
    pub selected: usize,
}

#[derive(Debug, Clone)]
pub struct HistoryEntry {
    pub name: String,
    pub iteration_index: u32,
    pub path: std::path::PathBuf,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flow::FlowEvent;

    #[test]
    fn step_started_prepopulates_all_pending_slots() {
        let mut state = AppState::new();
        state.apply(FlowEvent::StepStarted {
            index: 1,
            total: 9,
            label: "find next failing card".into(),
        });
        let iter = state.active_iteration().expect("iteration");
        assert_eq!(iter.steps.len(), 9);
        assert_eq!(iter.steps[0].status, StepStatus::Running);
        assert_eq!(iter.steps[0].label, "find next failing card");
        for s in &iter.steps[1..] {
            assert_eq!(s.status, StepStatus::Pending);
            assert!(s.label.is_empty());
            assert_eq!(s.total, 9);
        }
    }

    #[test]
    fn later_step_started_fills_the_correct_slot() {
        let mut state = AppState::new();
        state.apply(FlowEvent::StepStarted {
            index: 1,
            total: 9,
            label: "step 1".into(),
        });
        state.apply(FlowEvent::StepFinished {
            index: 1,
            ok: true,
            summary: None,
        });
        state.apply(FlowEvent::StepStarted {
            index: 5,
            total: 9,
            label: "step 5".into(),
        });
        let iter = state.active_iteration().expect("iteration");
        assert_eq!(iter.steps.len(), 9);
        assert_eq!(iter.steps[0].status, StepStatus::Done);
        assert_eq!(iter.steps[1].status, StepStatus::Pending);
        assert_eq!(iter.steps[4].status, StepStatus::Running);
        assert_eq!(iter.steps[4].label, "step 5");
        assert_eq!(iter.steps[8].status, StepStatus::Pending);
    }

    #[test]
    fn duplicate_adjacent_agent_done_events_are_suppressed() {
        let mut state = AppState::new();
        let raw: serde_json::Value =
            serde_json::from_str(r#"{"type":"turn.completed","status":"success"}"#).unwrap();
        state.apply(FlowEvent::AgentEvent {
            provider: crate::flow::AgentProvider::Codex,
            raw: raw.clone(),
            elapsed_secs: 1,
        });
        state.apply(FlowEvent::AgentEvent {
            provider: crate::flow::AgentProvider::Codex,
            raw,
            elapsed_secs: 2,
        });

        assert_eq!(
            state
                .events
                .iter()
                .filter(|row| matches!(
                    row.kind,
                    TimelineKind::Agent {
                        parsed: ParsedAgentEvent::Done { .. },
                        ..
                    }
                ))
                .count(),
            1
        );
    }
}

/// Helper used by the view to format a tool_use one-liner with file
/// paths shortened to repo-relative form.
pub fn format_tool_target(target: &ToolUseTarget, repo_root: &std::path::Path) -> String {
    match target {
        ToolUseTarget::File(p) => agent_events::relativize(p, repo_root),
        ToolUseTarget::Command(c) => format!("$ {}", agent_events::trim_to(c, 160)),
        ToolUseTarget::Pattern(p) => format!("/{p}/"),
        ToolUseTarget::Description(d) => agent_events::trim_to(d, 160),
        ToolUseTarget::None => String::new(),
    }
}
