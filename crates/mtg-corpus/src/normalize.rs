/// Normalize Scryfall Oracle text for parsing.
///
/// For the M2 walking skeleton: strip reminder text (parenthesized
/// phrases) and collapse the runs of whitespace that leaves behind.
/// More aggressive normalization (Unicode em-dashes, split-card joins,
/// etc.) is added when the grammar reaches the cards that need it.
pub fn normalize_oracle_text(text: &str) -> String {
    let no_reminders = strip_reminders(text);
    let collapsed = collapse_whitespace(&no_reminders);
    // Stripping a trailing reminder ("Flying (...)") leaves a trailing
    // space on its line. Trim each line so per-line round-tripping is
    // tractable later.
    collapsed
        .lines()
        .map(str::trim)
        .collect::<Vec<_>>()
        .join("\n")
        .trim()
        .to_string()
}

fn strip_reminders(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut depth: i32 = 0;
    for c in text.chars() {
        match c {
            '(' => depth += 1,
            ')' if depth > 0 => depth -= 1,
            _ if depth == 0 => out.push(c),
            _ => {}
        }
    }
    out
}

fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = false;
    for c in text.chars() {
        if c == '\n' {
            out.push('\n');
            last_space = false;
        } else if c.is_whitespace() {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else {
            out.push(c);
            last_space = false;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_single_reminder() {
        assert_eq!(
            normalize_oracle_text(
                "Flying (This creature can't be blocked except by creatures with flying or reach.)"
            ),
            "Flying",
        );
    }

    #[test]
    fn strips_multiple_reminders() {
        assert_eq!(
            normalize_oracle_text("Trample (some reminder)\nVigilance (more reminder)"),
            "Trample\nVigilance",
        );
    }

    #[test]
    fn preserves_internal_punctuation() {
        assert_eq!(
            normalize_oracle_text("Destroy target creature."),
            "Destroy target creature.",
        );
    }

    #[test]
    fn handles_unmatched_close_paren_gracefully() {
        // Doesn't panic on malformed input; preserves the stray paren.
        assert_eq!(normalize_oracle_text("a) b"), "a) b");
    }
}
