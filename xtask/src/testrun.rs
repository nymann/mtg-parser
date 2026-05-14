use std::process::{Command, ExitCode};

pub fn run(args: &[String]) -> ExitCode {
    let tier = match parse_tier(args) {
        Ok(t) => t,
        Err(msg) => {
            eprintln!("{msg}");
            return ExitCode::from(2);
        }
    };

    let invocations: &[&[&str]] = match tier {
        1 => &[&[
            "test",
            "-p",
            "mtg-grammar",
            "--test",
            "unit",
            "--test",
            "generated",
        ]],
        2 => &[
            &[
                "test",
                "-p",
                "mtg-grammar",
                "--test",
                "unit",
                "--test",
                "generated",
            ],
            &["test", "-p", "mtg-grammar", "--test", "prop"],
        ],
        _ => {
            eprintln!("unsupported tier: {tier} (only 1 and 2 implemented so far)");
            return ExitCode::from(2);
        }
    };

    for argv in invocations {
        let status = Command::new("cargo").args(*argv).status();
        match status {
            Ok(s) if s.success() => {}
            Ok(s) => return ExitCode::from(s.code().unwrap_or(1).clamp(0, 255) as u8),
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
