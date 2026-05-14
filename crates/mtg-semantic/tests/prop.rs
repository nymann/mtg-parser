// Tier 2 lowering property tests.
//
// Totality (M4 exit criterion): every AST the parser can produce must
// lower without error.
//
// AST strategies are intentionally duplicated from
// crates/mtg-grammar/tests/prop.rs. When the AST grows enough that
// drift between the two copies becomes painful, lift the strategies
// into a feature-gated `mtg_grammar::testing` module and update the
// xtask runner to enable that feature for tier 2.

use mtg_grammar::{ManaCost, ManaSymbol, Statement};
use mtg_semantic::{lower, CardEffect};
use proptest::prelude::*;

fn arb_mana_symbol() -> impl Strategy<Value = ManaSymbol> {
    prop_oneof![
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

    /// Lowering is total over every Statement the parser can produce.
    #[test]
    fn lowering_is_total(stmt in arb_statement()) {
        prop_assert!(lower(&stmt).is_ok());
    }

    /// For ManaCost statements, the IR's total() must equal the sum of
    /// pip weights: each colored pip contributes 1, each generic pip
    /// contributes its number. Cross-checks the lowering against an
    /// independently-computed reference.
    #[test]
    fn mana_value_total_matches_pip_sum(stmt in arb_statement()) {
        let Statement::ManaCost(ref mc) = stmt else { return Ok(()); };
        let expected: u32 = mc.symbols.iter().map(|s| match s {
            ManaSymbol::Generic(n) => *n,
            _ => 1,
        }).sum();
        let CardEffect::ManaCost(mv) = lower(&stmt).unwrap() else {
            return Err(TestCaseError::fail("ManaCost statement lowered to non-ManaCost effect"));
        };
        prop_assert_eq!(mv.total(), expected);
    }
}
