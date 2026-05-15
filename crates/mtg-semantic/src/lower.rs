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
        Statement::ThisSpellCostsManaMoreToCastForEachTargetBeyondTheFirst { mana } => {
            CardEffect::ThisSpellCostsManaMoreToCastForEachTargetBeyondTheFirst {
                mana: mana.clone(),
            }
        }
        Statement::NamedSourceDealsVariableDamageDividedEvenlyRoundedDownAmongAnyNumberOfTargets {
            source_name,
            amount,
        } => {
            CardEffect::NamedSourceDealsVariableDamageDividedEvenlyRoundedDownAmongAnyNumberOfTargets {
                source_name: source_name.clone(),
                amount: *amount,
            }
        }
        Statement::NamedSourceDealsVariableDamageToAnyTarget {
            source_name,
            amount,
        } => CardEffect::NamedSourceDealsVariableDamageToAnyTarget {
            source_name: source_name.clone(),
            amount: *amount,
        },
        Statement::NamedSourceDealsDamageToAnyTarget {
            source_name,
            amount,
        } => CardEffect::NamedSourceDealsDamageToAnyTarget {
            source_name: source_name.clone(),
            amount: *amount,
        },
        Statement::NamedSourceDealsVariableDamageToDamageRecipients {
            source_name,
            amount,
            recipients,
        } => CardEffect::NamedSourceDealsVariableDamageToDamageRecipients {
            source_name: source_name.clone(),
            amount: *amount,
            recipients: recipients.clone(),
        },
        Statement::PreventAllCombatDamageThisTurn => CardEffect::PreventAllCombatDamageThisTurn,
        Statement::SpendOnlyColorManaOnVariable { color, variable } => {
            CardEffect::SpendOnlyColorManaOnVariable {
                color: *color,
                variable: *variable,
            }
        }
        Statement::AsSourceEntersYouLoseLifeEqualToYourLifeTotal { source } => {
            CardEffect::AsSourceEntersYouLoseLifeEqualToYourLifeTotal { source: *source }
        }
        Statement::YouGainLifeEqualToDamageDealtCapped { caps } => {
            CardEffect::YouGainLifeEqualToDamageDealtCapped { caps: caps.clone() }
        }
        Statement::IfYouCantYouLoseTheGame => CardEffect::IfYouCantYouLoseTheGame,
        Statement::IfYouCantSourceDealsDamageToYou { source, amount } => {
            CardEffect::IfYouCantSourceDealsDamageToYou {
                source: *source,
                amount: *amount,
            }
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
        Statement::DestroyTargetPermanent { permanent_type } => {
            CardEffect::DestroyTargetPermanent {
                permanent_type: *permanent_type,
            }
        }
        Statement::ThatPermanentsControllerMayAttachThisAuraToPermanentOfTheirChoice {
            controller_of,
            attach_to,
        } => CardEffect::ThatPermanentsControllerMayAttachThisAuraToPermanentOfTheirChoice {
            controller_of: *controller_of,
            attach_to: *attach_to,
        },
        Statement::DestroyAll { permanent_type } => CardEffect::DestroyAll {
            permanent_type: *permanent_type,
        },
        Statement::DestroyAllBasicLands { basic_land_type } => CardEffect::DestroyAllBasicLands {
            basic_land_type: *basic_land_type,
        },
        Statement::Keyword(kw) => CardEffect::Keyword(*kw),
        Statement::KeywordList(keywords) => CardEffect::KeywordList(keywords.clone()),
        Statement::SemicolonKeywordList(keywords) => {
            CardEffect::SemicolonKeywordList(keywords.clone())
        }
        Statement::TargetPlayerDrawsCards { count } => {
            CardEffect::TargetPlayerDrawsCards { count: *count }
        }
        Statement::TargetPlayerDiscardsCardsAtRandom { count } => {
            CardEffect::TargetPlayerDiscardsCardsAtRandom { count: *count }
        }
        Statement::IfYouWouldDrawCardDuringYourDrawStepInsteadYouMaySkipThatDraw => {
            CardEffect::IfYouWouldDrawCardDuringYourDrawStepInsteadYouMaySkipThatDraw
        }
        Statement::LookAtTopCardsOfTargetPlayersLibraryThenPutThemBackInAnyOrder { count } => {
            CardEffect::LookAtTopCardsOfTargetPlayersLibraryThenPutThemBackInAnyOrder {
                count: *count,
            }
        }
        Statement::YouMayHaveThatPlayerShuffle => CardEffect::YouMayHaveThatPlayerShuffle,
        Statement::TargetPlayerGainsLife { amount } => {
            CardEffect::TargetPlayerGainsLife { amount: *amount }
        }
        Statement::TapAllPermanentsTargetPlayerControlsAndThatPlayerLosesUnspentMana {
            permanent_type,
        } => CardEffect::TapAllPermanentsTargetPlayerControlsAndThatPlayerLosesUnspentMana {
            permanent_type: *permanent_type,
        },
        Statement::TargetPlayerActivatesManaAbilityOfEachPermanentTheyControl {
            permanent_type,
        } => CardEffect::TargetPlayerActivatesManaAbilityOfEachPermanentTheyControl {
            permanent_type: *permanent_type,
        },
        Statement::ThenThatPlayerLosesUnspentManaAndYouAddManaLostThisWay => {
            CardEffect::ThenThatPlayerLosesUnspentManaAndYouAddManaLostThisWay
        }
        Statement::ChangeTextOfTargetSpellOrPermanentReplacingBasicLandType => {
            CardEffect::ChangeTextOfTargetSpellOrPermanentReplacingBasicLandType
        }
        Statement::AddMana { mana } => CardEffect::AddMana { mana: mana.clone() },
        Statement::AntePlayRestriction => CardEffect::AntePlayRestriction,
        Statement::YouOwnTargetCardInZone { zone } => {
            CardEffect::YouOwnTargetCardInZone { zone: *zone }
        }
        Statement::ExchangeThatCardWithTopCardOfYourLibrary => {
            CardEffect::ExchangeThatCardWithTopCardOfYourLibrary
        }
        Statement::CopyTargetSpellExceptCopyIsColor { spell_types, color } => {
            CardEffect::CopyTargetSpellExceptCopyIsColor {
                spell_types: spell_types.clone(),
                color: *color,
            }
        }
        Statement::YouMayChooseNewTargetsForTheCopy => {
            CardEffect::YouMayChooseNewTargetsForTheCopy
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
                cost: cost.clone(),
            }
        }
        Statement::PreventNextDamageThatWouldBeDealtToRecipientThisTurn { amount, recipient } => {
            CardEffect::PreventNextDamageThatWouldBeDealtToRecipientThisTurn {
                amount: *amount,
                recipient: *recipient,
            }
        }
        Statement::IfYouDoPreventNextDamageThatWouldBeDealtToRecipientThisTurn {
            amount,
            recipient,
        } => CardEffect::IfYouDoPreventNextDamageThatWouldBeDealtToRecipientThisTurn {
            amount: *amount,
            recipient: *recipient,
        },
        Statement::IfYouDoAddMana { mana } => CardEffect::IfYouDoAddMana { mana: mana.clone() },
        Statement::IfYouDoUntap { source } => CardEffect::IfYouDoUntap { source: *source },
        Statement::IfYouDoGainLife { amount } => CardEffect::IfYouDoGainLife { amount: *amount },
        Statement::IfYouDoUntilYourNextTurnYouCantBeAttackedExceptByCreaturesWithKeywords {
            keywords,
        } => CardEffect::IfYouDoUntilYourNextTurnYouCantBeAttackedExceptByCreaturesWithKeywords {
            keywords: keywords.clone(),
        },
        Statement::IfYouDoCastThatCardFaceDownWithoutPayingManaCost { power, toughness } => {
            CardEffect::IfYouDoCastThatCardFaceDownWithoutPayingManaCost {
                power: *power,
                toughness: *toughness,
            }
        }
        Statement::IfFaceDownSpellCreatureWouldAssignOrDealDamageOrTapTurnFaceUpInstead => {
            CardEffect::IfFaceDownSpellCreatureWouldAssignOrDealDamageOrTapTurnFaceUpInstead
        }
        Statement::TargetSpellOrPermanentBecomesColor { color } => {
            CardEffect::TargetSpellOrPermanentBecomesColor { color: *color }
        }
        Statement::TargetPermanentGetsUntilEndOfTurn {
            permanent_type,
            modifier,
        } => CardEffect::TargetPermanentGetsUntilEndOfTurn {
            permanent_type: *permanent_type,
            modifier: *modifier,
        },
        Statement::TargetPermanentGetsMixedUntilEndOfTurn {
            permanent_type,
            modifier,
        } => CardEffect::TargetPermanentGetsMixedUntilEndOfTurn {
            permanent_type: *permanent_type,
            modifier: *modifier,
        },
        Statement::TargetPermanentGainsKeywordUntilEndOfTurn {
            permanent_type,
            keyword,
        } => CardEffect::TargetPermanentGainsKeywordUntilEndOfTurn {
            permanent_type: *permanent_type,
            keyword: *keyword,
        },
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
        Statement::ActivateOnlyDuringCombat => CardEffect::ActivateOnlyDuringCombat,
        Statement::ActivateOnlyDuringYourTurn => CardEffect::ActivateOnlyDuringYourTurn,
        Statement::ActivateOnlyDuringYourTurnAndOnlyOnceEachTurn => {
            CardEffect::ActivateOnlyDuringYourTurnAndOnlyOnceEachTurn
        }
        Statement::ActivateOnlyAsSorcery => CardEffect::ActivateOnlyAsSorcery,
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
