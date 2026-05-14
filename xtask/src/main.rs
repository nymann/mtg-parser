// Developer workflow CLI. Subcommands are added per milestone:
//   M1: `test`
//   M2: `next-card`, `corpus`, `refresh-corpus`
//   later: `grammar-fix`, `bench`, `diff-lark`

use std::process::ExitCode;

mod claude_events;
mod console_sink;
mod corpus_cmd;
mod flow;
mod grammar_fix;
mod next_card;
mod paths;
mod testrun;
mod tui;

const HELP: &str = "\
cargo xtask <command> [args]

Commands:
  test                        Run Tier 1 tests (hand-written unit tests, <1s budget).
  test --tier 2               Run Tier 1 + Tier 2 (round-trip property tests, <10s budget).
  next-card [--set CODE]      Walk a Scryfall set, generate a failing test for the first
                              card the parser can't round-trip. Defaults to --set lea.
  corpus [--update]           Parse every card in the tracked sets and diff against the
                              committed corpus_status.json. --update overwrites it even
                              if there are regressions (use with care).
  refresh-corpus [--set CODE] Force re-fetch a set from Scryfall, bypassing the cache.
                              Without --set, refreshes every set tracked by `corpus`.
  grammar-fix [--set CODE]    Orchestrated loop: next-card → claude -p → tier-1/2 →
              [--max-iterations N]  corpus diff → commit. Defaults: --set lea,
              [--dry-run] [--allow-dirty]  --max-iterations 1. --dry-run builds the
              [--ui console|tui]          prompt and stops; --allow-dirty skips the
                                          clean-tree precondition; --ui tui opens a
                                          full-screen interactive view.

Flags:
  -h, --help        Show this message.
";

enum Ui {
    Console,
    Tui,
}

fn parse_ui(args: &[String]) -> Result<Ui, String> {
    let mut iter = args.iter();
    while let Some(a) = iter.next() {
        match a.as_str() {
            "--ui" => match iter.next().map(String::as_str) {
                Some("console") => return Ok(Ui::Console),
                Some("tui") => return Ok(Ui::Tui),
                Some(other) => {
                    return Err(format!("--ui must be 'console' or 'tui', got {other:?}"))
                }
                None => return Err("--ui requires a value".into()),
            },
            s if s.starts_with("--ui=") => match &s["--ui=".len()..] {
                "console" => return Ok(Ui::Console),
                "tui" => return Ok(Ui::Tui),
                other => return Err(format!("--ui must be 'console' or 'tui', got {other:?}")),
            },
            _ => {}
        }
    }
    Ok(Ui::Console)
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        None | Some("-h") | Some("--help") => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some("test") => testrun::run(&args[1..]),
        Some("next-card") => next_card::run(&args[1..]),
        Some("corpus") => corpus_cmd::run(&args[1..]),
        Some("refresh-corpus") => corpus_cmd::refresh(&args[1..]),
        Some("grammar-fix") => match parse_ui(&args[1..]) {
            Ok(Ui::Console) => grammar_fix::run(&args[1..]),
            Ok(Ui::Tui) => match grammar_fix::Options::parse(&args[1..]) {
                Ok(opts) => match tui::run(opts) {
                    Ok(code) => code,
                    Err(e) => {
                        eprintln!("tui error: {e:#}");
                        ExitCode::FAILURE
                    }
                },
                Err(e) => {
                    eprintln!("{e}");
                    ExitCode::from(2)
                }
            },
            Err(e) => {
                eprintln!("{e}");
                ExitCode::from(2)
            }
        },
        Some(cmd) => {
            eprintln!("unknown command: {cmd}\n\n{HELP}");
            ExitCode::from(2)
        }
    }
}
