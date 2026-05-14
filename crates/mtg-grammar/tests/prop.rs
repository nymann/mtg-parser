// Tier 2 property tests. The core invariant: every AST the parser can
// produce must survive `parse(unparse(ast)) == ast`. A failure here
// signals either grammar ambiguity or unparser/grammar drift.

use mtg_grammar::{parse, unparse, ManaCost, ManaSymbol, Statement};
use proptest::prelude::*;

fn arb_mana_symbol() -> impl Strategy<Value = ManaSymbol> {
    prop_oneof![
        (0u32..=20).prop_map(ManaSymbol::Generic),
        Just(ManaSymbol::White),
        Just(ManaSymbol::Blue),
        Just(ManaSymbol::Black),
        Just(ManaSymbol::Red),
        Just(ManaSymbol::Green),
        Just(ManaSymbol::Colorless),
    ]
}

fn arb_mana_cost() -> impl Strategy<Value = ManaCost> {
    prop::collection::vec(arb_mana_symbol(), 1..6).prop_map(|symbols| ManaCost { symbols })
}

fn arb_statement() -> impl Strategy<Value = Statement> {
    prop_oneof![
        arb_mana_cost().prop_map(Statement::ManaCost),
        Just(Statement::DestroyTargetCreature),
    ]
}

proptest! {
    #[test]
    fn round_trip(stmt in arb_statement()) {
        let text = unparse(&stmt);
        let reparsed = parse(&text)
            .map_err(|e| TestCaseError::fail(format!("parse failed on {text:?}: {e}")))?;
        prop_assert_eq!(stmt, reparsed);
    }
}
