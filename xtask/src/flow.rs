//! Event-driven decoupling between the add-card orchestrator and
//! its output surface (console / TUI).
//!
//! The orchestrator emits `FlowEvent`s through any [`FlowSink`]; the
//! sink decides how to display each event. Adding a new sink does not
//! require touching `add_card.rs`. Reordering or renaming steps in
//! `add_card.rs` does not require touching the sinks (unless the
//! shape of the data attached to an event changes).

use mtg_scryfall::Card;

#[derive(Debug, Clone)]
pub enum FlowEvent {
    /// Emitted once at the start of a run with configuration + baseline
    /// state.
    SessionStarted {
        workflow: String,
        set: String,
        max_iterations: u32,
        baseline_corpus_passing: usize,
        baseline_corpus_total: usize,
        baseline_grammar_rules: usize,
    },
    /// A pipeline step begins.
    StepStarted { index: u8, total: u8, label: String },
    /// A pipeline step finishes. `summary` is optional terse info
    /// associated with this step (a path, a byte count, …).
    StepFinished {
        index: u8,
        ok: bool,
        summary: Option<String>,
    },
    /// One iteration begins. Emitted *after* step 1 has found the card
    /// so the payload is complete.
    IterationStarted {
        index: u32,
        max_iterations: u32,
        card: Card,
        normalized: String,
        round_trip_error: String,
    },
    /// One non-card workflow iteration begins.
    WorkflowIterationStarted {
        index: u32,
        max_iterations: u32,
        title: String,
        detail: String,
    },
    /// Generic mid-step log line. Used sparingly — most output should
    /// be carried by `StepFinished.summary` or `AgentEvent`.
    Note { level: NoteLevel, text: String },
    /// One agent JSONL event (already parsed from NDJSON/JSONL).
    /// The orchestrator mirrors the raw line into `transcript.ndjson`
    /// on disk separately; sinks see only the parsed `serde_json::Value`.
    AgentEvent {
        provider: AgentProvider,
        raw: serde_json::Value,
        /// Seconds since the agent began for this iteration. Computed by
        /// the orchestrator so all sinks share the same clock.
        elapsed_secs: u64,
    },
    /// One iteration finishes.
    IterationFinished {
        index: u32,
        outcome: IterationOutcomeSummary,
    },
    /// The whole session ends.
    SessionFinished { reason: SessionEndReason },
}

#[derive(Debug, Clone)]
pub enum IterationOutcomeSummary {
    Committed {
        new_passes: usize,
        corpus_passing: usize,
        corpus_total: usize,
        grammar_rules: usize,
        duration_secs: u64,
    },
    SurfacedToHuman {
        reason: String,
    },
    // Session-level end states (AllPass, DryRunStop) live on
    // SessionEndReason instead — they're not per-iteration.
}

#[derive(Debug, Clone)]
pub enum SessionEndReason {
    AllPass,
    DryRunStop,
    MaxIterationsReached(u32),
    /// Auto-advance walked off the end of Scryfall's paper expansion
    /// list — every tracked set is fully covered, no more sets to add.
    CorpusComplete,
    // The reason string is consumed only by sinks that surface to the
    // user (TUI today, console keeps it brief).
    SurfacedToHuman(#[allow(dead_code)] String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[allow(dead_code)] // Info / Error are used by future sinks (TUI status line).
pub enum NoteLevel {
    Info,
    Warn,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentProvider {
    Codex,
    Claude,
}

impl AgentProvider {
    pub fn label(self) -> &'static str {
        match self {
            AgentProvider::Codex => "codex",
            AgentProvider::Claude => "claude",
        }
    }
}

/// Consumer of orchestrator events. `Send` lets the orchestrator run
/// on a background thread (e.g. when the main thread is busy rendering
/// a TUI).
pub trait FlowSink: Send {
    fn emit(&mut self, event: FlowEvent);
}
