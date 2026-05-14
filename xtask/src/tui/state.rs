//! TUI state model. The orchestrator's `FlowEvent`s feed [`AppState::apply`].
//! Rendering reads `&AppState`; input handling mutates it.
//!
//! Intentionally decoupled from `grammar_fix.rs` — this module only
//! knows about `FlowEvent` (a value type in `crate::flow`). Reordering
//! steps or adding new ones in `grammar_fix.rs` doesn't require any
//! change here unless the event vocabulary itself grows.

use std::time::{Duration, Instant};

use mtg_scryfall::Card;

use crate::claude_events::{self, ParsedClaudeEvent, ToolUseTarget};
use crate::flow::{FlowEvent, IterationOutcomeSummary, NoteLevel, SessionEndReason};

#[derive(Default)]
pub struct AppState {
    pub session: Option<SessionMeta>,
    pub iterations: Vec<Iteration>,
    pub events: Vec<TimelineRow>,
    pub session_end: Option<SessionEndReason>,
    pub orchestrator_done: bool,

    // view state
    pub scroll: u16, // first row visible in the output pane
    pub autoscroll: bool,
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

            FlowEvent::ClaudeEvent { raw, elapsed_secs } => {
                let _ = elapsed_secs; // computed locally via delta
                let delta = self.delta_since_last();
                for parsed in claude_events::parse(&raw) {
                    self.events.push(TimelineRow {
                        iteration_index: self.iterations.len() as u32,
                        delta,
                        kind: TimelineKind::Claude(parsed),
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
        let i = index.saturating_sub(1) as usize;
        if i >= self.steps.len() {
            self.steps.resize_with(i + 1, StepState::default);
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
    StepHeader { index: u8, label: String },
    StepResult { ok: bool, summary: String },
    Claude(ParsedClaudeEvent),
    Note { level: NoteLevel, text: String },
    IterationFooter(IterationOutcomeSummary),
}

/// Helper used by the view to format a tool_use one-liner with file
/// paths shortened to repo-relative form.
pub fn format_tool_target(target: &ToolUseTarget, repo_root: &std::path::Path) -> String {
    match target {
        ToolUseTarget::File(p) => claude_events::relativize(p, repo_root),
        ToolUseTarget::Command(c) => format!("$ {}", claude_events::trim_to(c, 160)),
        ToolUseTarget::Pattern(p) => format!("/{p}/"),
        ToolUseTarget::Description(d) => claude_events::trim_to(d, 160),
        ToolUseTarget::None => String::new(),
    }
}
