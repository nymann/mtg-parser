// Tier 1 unit tests. Must collectively finish in well under a second.
// Hand-written input → expected AST and AST → expected text.

use mtg_grammar::{parse, unparse, ManaCost, ManaSymbol, Statement};

fn mc(symbols: Vec<ManaSymbol>) -> Statement {
    Statement::ManaCost(ManaCost { symbols })
}

#[test]
fn parses_single_colored_mana_symbol() {
    assert_eq!(parse("{R}").unwrap(), mc(vec![ManaSymbol::Red]));
}

#[test]
fn parses_generic_mana_symbol() {
    assert_eq!(parse("{2}").unwrap(), mc(vec![ManaSymbol::Generic(2)]));
}

#[test]
fn parses_generic_zero_mana_symbol() {
    // {0} is a real cost (Memnite, Spellbook). The canonical form is "{0}".
    assert_eq!(parse("{0}").unwrap(), mc(vec![ManaSymbol::Generic(0)]));
    assert_eq!(unparse(&mc(vec![ManaSymbol::Generic(0)])), "{0}");
}

#[test]
fn parses_compound_mana_cost() {
    assert_eq!(
        parse("{2}{R}{R}").unwrap(),
        mc(vec![
            ManaSymbol::Generic(2),
            ManaSymbol::Red,
            ManaSymbol::Red,
        ]),
    );
}

#[test]
fn parses_all_five_colors() {
    assert_eq!(
        parse("{W}{U}{B}{R}{G}").unwrap(),
        mc(vec![
            ManaSymbol::White,
            ManaSymbol::Blue,
            ManaSymbol::Black,
            ManaSymbol::Red,
            ManaSymbol::Green,
        ]),
    );
}

#[test]
fn parses_colorless_mana_symbol() {
    assert_eq!(parse("{C}").unwrap(), mc(vec![ManaSymbol::Colorless]));
}

#[test]
fn rejects_internal_whitespace_in_mana_cost() {
    assert!(parse("{2} {R}").is_err());
}

#[test]
fn parses_destroy_target_creature() {
    assert_eq!(
        parse("Destroy target creature.").unwrap(),
        Statement::DestroyTargetCreature,
    );
}

#[test]
fn destroy_is_case_insensitive() {
    assert_eq!(
        parse("destroy target creature.").unwrap(),
        Statement::DestroyTargetCreature,
    );
    assert_eq!(
        parse("DESTROY TARGET CREATURE.").unwrap(),
        Statement::DestroyTargetCreature,
    );
}

#[test]
fn destroy_requires_terminating_period() {
    assert!(parse("Destroy target creature").is_err());
}

#[test]
fn unparses_mana_cost_in_scryfall_form() {
    assert_eq!(
        unparse(&mc(vec![
            ManaSymbol::Generic(2),
            ManaSymbol::Red,
            ManaSymbol::Red,
        ])),
        "{2}{R}{R}",
    );
}

#[test]
fn unparses_destroy_in_canonical_form() {
    assert_eq!(
        unparse(&Statement::DestroyTargetCreature),
        "Destroy target creature.",
    );
}
