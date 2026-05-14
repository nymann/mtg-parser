// Tier 2 property tests. The core invariant: every AST the parser can
// produce must survive `parse(unparse(ast)) == ast`. A failure here
// signals either grammar ambiguity or unparser/grammar drift.
//
// The M3 exit criterion is 1000 cases; that stays inside the <10s tier-2
// budget thanks to the trivial parser/unparser.

use mtg_grammar::{parse, unparse, ManaCost, ManaSymbol, Statement};
use proptest::prelude::*;

fn arb_mana_symbol() -> impl Strategy<Value = ManaSymbol> {
    prop_oneof![
        // Wider than realistic costs to surface any digit-handling bugs.
        (0u32..=999).prop_map(ManaSymbol::Generic),
        Just(ManaSymbol::White),
        Just(ManaSymbol::Blue),
        Just(ManaSymbol::Black),
        Just(ManaSymbol::Red),
        Just(ManaSymbol::Green),
        Just(ManaSymbol::Colorless),
    ]
}

fn arb_mana_cost() -> impl Strategy<Value = ManaCost> {
    prop::collection::vec(arb_mana_symbol(), 1..16).prop_map(|symbols| ManaCost { symbols })
}

fn arb_statement() -> impl Strategy<Value = Statement> {
    prop_oneof![
        arb_mana_cost().prop_map(Statement::ManaCost),
        Just(Statement::DestroyTargetCreature),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 1000, ..ProptestConfig::default() })]

    #[test]
    fn round_trip(stmt in arb_statement()) {
        let text = unparse(&stmt);
        let reparsed = parse(&text)
            .map_err(|e| TestCaseError::fail(format!("parse failed on {text:?}: {e}")))?;
        prop_assert_eq!(stmt, reparsed);
    }
}
