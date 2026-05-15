// Tier 1 lowering unit tests. Hand-written AST → expected IR.

use mtg_grammar::{ManaCost, ManaSymbol, Statement};
use mtg_semantic::{lower, CardEffect, ManaValue};

fn mc(symbols: Vec<ManaSymbol>) -> Statement {
    Statement::ManaCost(ManaCost { symbols })
}

#[test]
fn lowers_destroy_target_creature() {
    assert_eq!(
        lower(&Statement::DestroyTargetCreature).unwrap(),
        CardEffect::DestroyTargetCreature,
    );
}

#[test]
fn lowers_counter_target_spell() {
    assert_eq!(
        lower(&Statement::CounterTargetSpell).unwrap(),
        CardEffect::CounterTargetSpell,
    );
}

#[test]
fn lowers_pure_generic_mana_cost() {
    assert_eq!(
        lower(&mc(vec![ManaSymbol::Generic(2)])).unwrap(),
        CardEffect::ManaCost(ManaValue {
            generic: 2,
            ..Default::default()
        }),
    );
}

#[test]
fn aggregates_generic_pips() {
    // {1}{1}{1} and {3} share an IR — that's the point of the lower.
    let one_one_one = lower(&mc(vec![
        ManaSymbol::Generic(1),
        ManaSymbol::Generic(1),
        ManaSymbol::Generic(1),
    ]))
    .unwrap();
    let three = lower(&mc(vec![ManaSymbol::Generic(3)])).unwrap();
    assert_eq!(one_one_one, three);
}

#[test]
fn separates_colors() {
    assert_eq!(
        lower(&mc(vec![
            ManaSymbol::White,
            ManaSymbol::Blue,
            ManaSymbol::Black,
            ManaSymbol::Red,
            ManaSymbol::Green,
            ManaSymbol::Colorless,
        ]))
        .unwrap(),
        CardEffect::ManaCost(ManaValue {
            white: 1,
            blue: 1,
            black: 1,
            red: 1,
            green: 1,
            colorless: 1,
            ..Default::default()
        }),
    );
}

#[test]
fn pip_order_does_not_matter() {
    let a = lower(&mc(vec![ManaSymbol::Red, ManaSymbol::Generic(2)])).unwrap();
    let b = lower(&mc(vec![ManaSymbol::Generic(2), ManaSymbol::Red])).unwrap();
    assert_eq!(a, b);
}

#[test]
fn mixed_cost_total_is_correct() {
    let CardEffect::ManaCost(m) = lower(&mc(vec![
        ManaSymbol::Generic(2),
        ManaSymbol::Red,
        ManaSymbol::Red,
    ]))
    .unwrap() else {
        panic!("expected ManaCost");
    };
    assert_eq!(m.total(), 4);
    assert_eq!(m.generic, 2);
    assert_eq!(m.red, 2);
}

#[test]
fn mana_value_total_is_sum_of_counters() {
    let m = ManaValue {
        generic: 5,
        red: 2,
        ..Default::default()
    };
    assert_eq!(m.total(), 7);
}
