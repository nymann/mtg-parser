//! ConsoleSink: prints `FlowEvent`s as the human-readable stream
//! developers see today on a plain terminal. No TUI, no colors beyond
//! what the underlying terminal interprets.

use mtg_scryfall::Card;

use crate::agent_events::{self, ParsedAgentEvent, ToolUseTarget};
use crate::flow::{FlowEvent, FlowSink, IterationOutcomeSummary, NoteLevel, SessionEndReason};
use crate::paths::repo_root;

#[derive(Default)]
pub struct ConsoleSink;

impl ConsoleSink {
    pub fn new() -> Self {
        Self
    }
}

impl FlowSink for ConsoleSink {
    fn emit(&mut self, event: FlowEvent) {
        match event {
            FlowEvent::SessionStarted {
                set,
                max_iterations,
                baseline_corpus_passing,
                baseline_corpus_total,
                baseline_grammar_rules,
            } => {
                println!(
                    "add-card  set={set}  max-iter={max_iterations}  \
                     corpus={baseline_corpus_passing}/{baseline_corpus_total}  \
                     grammar={baseline_grammar_rules} rules",
                    max_iterations = format_max_iterations(max_iterations),
                );
            }

            FlowEvent::StepStarted {
                index,
                total,
                label,
            } => {
                println!();
                println!("── step {index}/{total} ── {label}");
            }

            FlowEvent::IterationStarted {
                index,
                max_iterations,
                card,
                normalized,
                round_trip_error,
            } => {
                println!();
                println!(
                    "== iteration {index} / {} ==",
                    format_max_iterations(max_iterations)
                );
                print_card_overview(&card, &normalized, &round_trip_error);
            }

            FlowEvent::StepFinished { index, ok, summary } => match (ok, summary) {
                (true, Some(s)) => println!("    {s}"),
                (false, Some(s)) => println!("    step {index} [FAILED]: {s}"),
                (false, None) => println!("    step {index} [FAILED]"),
                (true, None) => {}
            },

            FlowEvent::Note { level, text } => {
                let prefix = match level {
                    NoteLevel::Info => "    ",
                    NoteLevel::Warn => "    [warn] ",
                    NoteLevel::Error => "    [error] ",
                };
                println!("{prefix}{text}");
            }

            FlowEvent::AgentEvent {
                provider,
                raw,
                elapsed_secs,
            } => {
                let ts = format!("[+{elapsed_secs:>3}s]");
                for parsed in agent_events::parse(provider, &raw) {
                    for line in render_agent_for_console(provider, &parsed) {
                        println!("    {ts} {line}");
                    }
                }
            }

            FlowEvent::IterationFinished { index, outcome } => match outcome {
                IterationOutcomeSummary::Committed {
                    new_passes,
                    corpus_passing,
                    corpus_total,
                    grammar_rules,
                    duration_secs,
                } => {
                    println!();
                    println!(
                        "Iteration {index} committed. New passes: {new_passes}. \
                         Status: {corpus_passing}/{corpus_total}. \
                         Grammar rules: {grammar_rules}. Duration: {duration_secs}s."
                    );
                }
                IterationOutcomeSummary::SurfacedToHuman { reason } => {
                    eprintln!();
                    eprintln!("STOP at iteration {index}: {reason}");
                    eprintln!(
                        "Working tree left as-is; inspect .add-card/<latest>/ for context."
                    );
                }
            },

            FlowEvent::SessionFinished { reason } => match reason {
                SessionEndReason::MaxIterationsReached(n) => {
                    println!();
                    println!("Reached --max-iterations={}.", format_max_iterations(n));
                }
                SessionEndReason::AllPass => {
                    println!();
                    println!("All cards pass; nothing to do.");
                }
                SessionEndReason::CorpusComplete => {
                    println!();
                    println!("All tracked sets fully covered. No more paper sets to advance to.");
                }
                SessionEndReason::DryRunStop => {
                    println!();
                    println!(
                        "--dry-run: not invoking an agent, not writing tests, not committing."
                    );
                }
                // The IterationFinished message already covered this case.
                SessionEndReason::SurfacedToHuman(_) => {}
            },
        }
    }
}

fn format_max_iterations(n: u32) -> String {
    if n == 0 {
        "∞".to_string()
    } else {
        n.to_string()
    }
}

fn print_card_overview(card: &Card, normalized: &str, error: &str) {
    println!("    Name        : {}", card.name);
    println!("    Set         : {}", card.set_code);
    println!("    Collector # : {}", card.collector_number);
    println!("    Layout      : {:?}", card.layout);
    println!(
        "    Mana cost   : {}",
        if card.mana_cost.is_empty() {
            "—"
        } else {
            card.mana_cost.as_str()
        }
    );
    println!("    Oracle text :");
    print_indented(&card.oracle_text, "      | ");
    if normalized != card.oracle_text {
        println!("    Normalized  :");
        print_indented(normalized, "      | ");
    }
    println!("    Round-trip  :");
    print_indented(error, "      | ");
}

fn print_indented(text: &str, prefix: &str) {
    if text.is_empty() {
        println!("{prefix}(empty)");
        return;
    }
    for line in text.lines() {
        println!("{prefix}{line}");
    }
}

fn render_agent_for_console(
    provider: crate::flow::AgentProvider,
    ev: &ParsedAgentEvent,
) -> Vec<String> {
    let label = provider.label();
    match ev {
        ParsedAgentEvent::Init { model } => {
            vec![format!("[{label} init] model={model}")]
        }
        ParsedAgentEvent::AssistantText { text } => {
            // Multi-line prose: prefix the first line, indent the rest
            // so the prefix lines up.
            let mut out = Vec::new();
            for (i, line) in text.lines().enumerate() {
                out.push(if i == 0 {
                    format!("[{label}] {line}")
                } else {
                    format!("            {line}")
                });
            }
            if out.is_empty() {
                out.push(format!("[{label}]"));
            }
            out
        }
        ParsedAgentEvent::ToolUse { name, target } => {
            let summary = format_tool_target(target);
            vec![format!("[tool_use] {name}{summary}")]
        }
        ParsedAgentEvent::ToolResult {
            first_line,
            is_error,
        } => {
            let tag = if *is_error {
                "tool_error"
            } else {
                "tool_result"
            };
            vec![format!(
                "[{tag}] {}",
                agent_events::trim_to(first_line, 200)
            )]
        }
        ParsedAgentEvent::Done {
            subtype,
            num_turns,
            total_cost_usd,
        } => {
            vec![format!(
                "[{label} done] subtype={subtype} turns={num_turns} cost=${total_cost_usd:.4}"
            )]
        }
        ParsedAgentEvent::Other => Vec::new(),
    }
}

fn format_tool_target(target: &ToolUseTarget) -> String {
    match target {
        ToolUseTarget::File(p) => format!(" {}", agent_events::relativize(p, &repo_root())),
        ToolUseTarget::Command(c) => {
            format!(" $ {}", agent_events::trim_to(c, 160))
        }
        ToolUseTarget::Pattern(p) => format!(" /{p}/"),
        ToolUseTarget::Description(d) => format!(" {}", agent_events::trim_to(d, 160)),
        ToolUseTarget::None => String::new(),
    }
}
