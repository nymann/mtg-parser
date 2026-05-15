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
        Statement::CastRestriction(restriction) => CardEffect::CastRestriction(*restriction),
        Statement::CounterTargetSpell => CardEffect::CounterTargetSpell,
        Statement::DestroyTargetCreature => CardEffect::DestroyTargetCreature,
        Statement::RegenerateTargetCreature => CardEffect::RegenerateTargetCreature,
        Statement::NamedSourceDealsVariableDamageToAnyTarget {
            source_name,
            amount,
        } => CardEffect::NamedSourceDealsVariableDamageToAnyTarget {
            source_name: source_name.clone(),
            amount: *amount,
        },
        Statement::SpendOnlyColorManaOnVariable { color, variable } => {
            CardEffect::SpendOnlyColorManaOnVariable {
                color: *color,
                variable: *variable,
            }
        }
        Statement::YouGainLifeEqualToDamageDealtCapped { caps } => {
            CardEffect::YouGainLifeEqualToDamageDealtCapped { caps: caps.clone() }
        }
        Statement::IfItsPermanentCantBeRegeneratedAndWouldDieExileInsteadThisTurn {
            permanent_type,
        } => CardEffect::IfItsPermanentCantBeRegeneratedAndWouldDieExileInsteadThisTurn {
            permanent_type: *permanent_type,
        },
        Statement::DestroyTargetPermanentChoice { permanent_types } => {
            CardEffect::DestroyTargetPermanentChoice {
                permanent_types: permanent_types.clone(),
            }
        }
        Statement::DestroyAll { permanent_type } => CardEffect::DestroyAll {
            permanent_type: *permanent_type,
        },
        Statement::Keyword(kw) => CardEffect::Keyword(*kw),
        Statement::TargetPlayerDrawsCards { count } => {
            CardEffect::TargetPlayerDrawsCards { count: *count }
        }
        Statement::AddMana { mana } => CardEffect::AddMana { mana: mana.clone() },
        Statement::AntePlayRestriction => CardEffect::AntePlayRestriction,
        Statement::YouOwnTargetCardInZone { zone } => {
            CardEffect::YouOwnTargetCardInZone { zone: *zone }
        }
        Statement::ExchangeThatCardWithTopCardOfYourLibrary => {
            CardEffect::ExchangeThatCardWithTopCardOfYourLibrary
        }
        Statement::ImperativeActionSequence { actions } => CardEffect::ImperativeActionSequence {
            actions: actions.clone(),
        },
        Statement::EachPlayerPerformsAction { action } => {
            CardEffect::EachPlayerPerformsAction { action: *action }
        }
        Statement::UntilEndOfTurnYouMayPayCostAtTiming { timing, cost } => {
            CardEffect::UntilEndOfTurnYouMayPayCostAtTiming {
                timing: *timing,
                cost: *cost,
            }
        }
        Statement::IfYouDoAddMana { mana } => CardEffect::IfYouDoAddMana { mana: mana.clone() },
        Statement::IfYouDoGainLife { amount } => CardEffect::IfYouDoGainLife { amount: *amount },
        Statement::TargetSpellOrPermanentBecomesColor { color } => {
            CardEffect::TargetSpellOrPermanentBecomesColor { color: *color }
        }
        Statement::TargetPermanentGainsKeywordAndGetsUntilEndOfTurn {
            permanent_type,
            keyword,
            modifier,
            definitions,
        } => CardEffect::TargetPermanentGainsKeywordAndGetsUntilEndOfTurn {
            permanent_type: *permanent_type,
            keyword: *keyword,
            modifier: *modifier,
            definitions: definitions.clone(),
        },
        Statement::EachPlayerEqualizesControlledPermanents { permanent_type } => {
            CardEffect::EachPlayerEqualizesControlledPermanents {
                permanent_type: *permanent_type,
            }
        }
        Statement::PlayersDoActionsTheSameWay { actions } => {
            CardEffect::PlayersDoActionsTheSameWay {
                actions: actions.clone(),
            }
        }
        Statement::AsThisPermanentEntersChooseOpponent { permanent_type } => {
            CardEffect::AsThisPermanentEntersChooseOpponent {
                permanent_type: *permanent_type,
            }
        }
        Statement::ThisPermanentEntersWithCounters {
            source,
            amount,
            counter,
        } => CardEffect::ThisPermanentEntersWithCounters {
            source: *source,
            amount: *amount,
            counter: *counter,
        },
        Statement::ThisAbilityCantCauseTotalCountersGreaterThan {
            counter,
            source,
            maximum,
        } => CardEffect::ThisAbilityCantCauseTotalCountersGreaterThan {
            counter: *counter,
            source: *source,
            maximum: *maximum,
        },
        Statement::IfThisAbilityActivatedAtLeastTimesThisTurnSacrificeSourceAtNextEndStep {
            threshold,
            source,
        } => CardEffect::IfThisAbilityActivatedAtLeastTimesThisTurnSacrificeSourceAtNextEndStep {
            threshold: *threshold,
            source: *source,
        },
        Statement::ActivateOnlyDuringYourUpkeep => CardEffect::ActivateOnlyDuringYourUpkeep,
        Statement::ActivateOnlyDuringYourTurn => CardEffect::ActivateOnlyDuringYourTurn,
        Statement::ModalChoice { modes } => CardEffect::ModalChoice {
            modes: modes.clone(),
        },
        Statement::StaticAbility(sa) => CardEffect::StaticAbility(sa.clone()),
        Statement::ActivatedAbility(aa) => CardEffect::ActivatedAbility(aa.clone()),
        Statement::TriggeredAbility(ta) => CardEffect::TriggeredAbility(ta.clone()),
        Statement::PhysicalAction(pa) => CardEffect::PhysicalAction(*pa),
        Statement::Compound(stmts) => {
            let lowered = stmts.iter().map(lower).collect::<Result<Vec<_>, _>>()?;
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
