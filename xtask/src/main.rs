// Developer workflow CLI. M1 only implements the `test` subcommand;
// further subcommands (`next-card`, `corpus`, `grammar-fix`, ...) are
// added in later milestones.

use std::process::{Command, ExitCode};

const HELP: &str = "\
cargo xtask <command> [args]

Commands:
  test              Run Tier 1 tests (hand-written unit tests, <1s budget).
  test --tier 2     Run Tier 1 + Tier 2 (round-trip property tests, <10s budget).

Flags:
  -h, --help        Show this message.
";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    match args.first().map(String::as_str) {
        None | Some("-h") | Some("--help") => {
            print!("{HELP}");
            ExitCode::SUCCESS
        }
        Some("test") => run_test(&args[1..]),
        Some(cmd) => {
            eprintln!("unknown command: {cmd}\n\n{HELP}");
            ExitCode::from(2)
        }
    }
}

fn run_test(args: &[String]) -> ExitCode {
    let tier = match parse_tier(args) {
        Ok(t) => t,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };

    let invocations: &[&[&str]] = match tier {
        1 => &[&["test", "-p", "mtg-grammar", "--test", "unit"]],
        2 => &[
            &["test", "-p", "mtg-grammar", "--test", "unit"],
            &["test", "-p", "mtg-grammar", "--test", "prop"],
        ],
        _ => {
            eprintln!("unsupported tier: {tier} (only 1 and 2 implemented in M1)");
            return ExitCode::from(2);
        }
    };

    for argv in invocations {
        let status = Command::new("cargo").args(*argv).status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => return ExitCode::from(s.code().unwrap_or(1) as u8),
            Err(e) => {
                eprintln!("failed to spawn cargo: {e}");
                return ExitCode::from(127);
            }
        }
    }
    ExitCode::SUCCESS
}

fn parse_tier(args: &[String]) -> Result<u8, String> {
    let mut iter = args.iter();
    while let Some(arg) = iter.next() {
        if arg == "--tier" {
            let v = iter
                .next()
                .ok_or_else(|| "--tier requires a value".to_string())?;
            return v
                .parse()
                .map_err(|_| format!("--tier value must be an integer, got {v:?}"));
        }
    }
    Ok(1)
}
