use mtg_grammar::{ManaCost, ManaSymbol, Statement};

use crate::error::SemanticError;
use crate::ir::{CardEffect, ManaValue};

/// Lower a syntactic statement into the semantic IR.
///
/// Returns `Result` for forward-compatibility — [`SemanticError`] is
/// uninhabited today, so the call cannot actually fail. The signature
/// will stay stable once real error variants arrive.
pub fn lower(ast: &Statement) -> Result<CardEffect, SemanticError> {
    Ok(match ast {
        Statement::ManaCost(mc) => CardEffect::ManaCost(lower_mana_cost(mc)),
        Statement::DestroyTargetCreature => CardEffect::DestroyTargetCreature,
        Statement::Keyword(kw) => CardEffect::Keyword(*kw),
        Statement::TargetPlayerDrawsCards { count } => {
            CardEffect::TargetPlayerDrawsCards { count: *count }
        }
        Statement::StaticAbility(sa) => CardEffect::StaticAbility(sa.clone()),
        Statement::Compound(stmts) => {
            let lowered = stmts
                .iter()
                .map(lower)
                .collect::<Result<Vec<_>, _>>()?;
            CardEffect::Compound(lowered)
        }
    })
}

fn lower_mana_cost(mc: &ManaCost) -> ManaValue {
    let mut v = ManaValue::default();
    for sym in &mc.symbols {
        match sym {
            // saturating_add is defensive — generic pip values are u32
            // and could conceivably overflow if a malformed test ever
            // produced a vast number of huge generic symbols. Real
            // costs are tiny.
            ManaSymbol::Generic(n) => v.generic = v.generic.saturating_add(*n),
            ManaSymbol::White => v.white += 1,
            ManaSymbol::Blue => v.blue += 1,
            ManaSymbol::Black => v.black += 1,
            ManaSymbol::Red => v.red += 1,
            ManaSymbol::Green => v.green += 1,
            ManaSymbol::Colorless => v.colorless += 1,
        }
    }
    v
}
