use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use crate::ast::{
    ActionTiming, ActivatedAbility, ActivatedCost, ActivatedEffect, BalanceSameWayAction,
    BasicLandType, CardCount, CastRestriction, Color, Condition, ContinuousEffect, CopyException,
    CreatureStatus, CreatureType, DamageLifeGainCap, DamageRecipient, EachPlayerAction,
    EnchantObject, EnchantedObject, ImperativeAction, InterveningIf, Keyword, ManaCost, ManaSymbol,
    MixedPtModifier, ModalMode, OptionalCost, PermanentType, PhysicalAction, PtModifier, Rounding,
    Sign, SignedNumber, SignedPtComponent, SignedVariable, SourceObject, Statement, StaticAbility,
    Step, TriggerEffect, TriggerEvent, TriggeredAbility, ValueExpression, Variable,
    VariableDefinition, VariablePtModifier, Zone,
};

#[derive(Parser)]
#[grammar = "grammar.pest"]
struct MtgParser;

#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    #[error("parse error: {0}")]
    Pest(#[from] Box<pest::error::Error<Rule>>),
    #[error("internal grammar/AST mismatch: unexpected rule {0}")]
    Internal(&'static str),
}

impl From<pest::error::Error<Rule>> for ParseError {
    fn from(value: pest::error::Error<Rule>) -> Self {
        ParseError::Pest(Box::new(value))
    }
}

pub fn parse(text: &str) -> Result<Statement, ParseError> {
    let mut pairs = MtgParser::parse(Rule::card_text, text)?;
    let card_text = pairs.next().expect("card_text always matches once");
    let mut statements = Vec::new();
    for inner in card_text.into_inner() {
        if inner.as_rule() == Rule::EOI {
            continue;
        }
        statements.push(statement_from_pair(inner)?);
    }
    match statements.len() {
        0 => Err(ParseError::Internal("empty card_text")),
        1 => Ok(statements.into_iter().next().expect("len checked")),
        _ => Ok(Statement::Compound(statements)),
    }
}

fn statement_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    match pair.as_rule() {
        Rule::mana_cost => Ok(Statement::ManaCost(mana_cost_from_pair(pair))),
        Rule::modal_choice => modal_choice_from_pair(pair),
        Rule::modal_mode => Ok(Statement::ModalChoice {
            modes: vec![modal_mode_from_pair(pair)?],
        }),
        Rule::cast_restriction => cast_restriction_from_pair(pair),
        Rule::ante_play_restriction => Ok(Statement::AntePlayRestriction),
        Rule::you_own_target_card_in_zone => you_own_target_card_in_zone_from_pair(pair),
        Rule::exchange_that_card_with_top_card_of_your_library => {
            Ok(Statement::ExchangeThatCardWithTopCardOfYourLibrary)
        }
        Rule::imperative_action_sequence => imperative_action_sequence_from_pair(pair),
        Rule::counter_target_spell => Ok(Statement::CounterTargetSpell),
        Rule::destroy => Ok(Statement::DestroyTargetCreature),
        Rule::regenerate_target_creature => Ok(Statement::RegenerateTargetCreature),
        Rule::named_source_deals_variable_damage_to_any_target => {
            named_source_deals_variable_damage_to_any_target_from_pair(pair)
        }
        Rule::named_source_deals_variable_damage_to_damage_recipients => {
            named_source_deals_variable_damage_to_damage_recipients_from_pair(pair)
        }
        Rule::spend_only_color_mana_on_variable => spend_only_color_mana_on_variable_from_pair(pair),
        Rule::you_gain_life_equal_damage_dealt_capped => {
            you_gain_life_equal_damage_dealt_capped_from_pair(pair)
        }
        Rule::if_its_permanent_cant_be_regenerated_and_would_die_exile_instead_this_turn => {
            if_its_permanent_cant_be_regenerated_and_would_die_exile_instead_this_turn_from_pair(
                pair,
            )
        }
        Rule::destroy_target_permanent_choice => destroy_target_permanent_choice_from_pair(pair),
        Rule::destroy_all => destroy_all_from_pair(pair),
        Rule::target_player_activates_mana_ability_of_each_permanent_they_control => {
            target_player_activates_mana_ability_of_each_permanent_they_control_from_pair(pair)
        }
        Rule::then_that_player_loses_unspent_mana_and_you_add_mana_lost_this_way => {
            Ok(Statement::ThenThatPlayerLosesUnspentManaAndYouAddManaLostThisWay)
        }
        Rule::draw_cards => draw_cards_from_pair(pair),
        Rule::add_mana => add_mana_from_pair(pair),
        Rule::until_eot_you_may_pay_cost_at_timing => {
            until_eot_you_may_pay_cost_at_timing_from_pair(pair)
        }
        Rule::if_you_do_add_mana => if_you_do_add_mana_from_pair(pair),
        Rule::if_you_do_gain_life => if_you_do_gain_life_from_pair(pair),
        Rule::target_spell_or_permanent_becomes_color => {
            target_spell_or_permanent_becomes_color_from_pair(pair)
        }
        Rule::target_permanent_gains_keyword_and_gets_eot => {
            target_permanent_gains_keyword_and_gets_eot_from_pair(pair)
        }
        Rule::each_player_performs_action => each_player_performs_action_from_pair(pair),
        Rule::each_player_equalizes_controlled_permanents => {
            each_player_equalizes_controlled_permanents_from_pair(pair)
        }
        Rule::players_do_actions_the_same_way => players_do_actions_the_same_way_from_pair(pair),
        Rule::as_this_permanent_enters_choose_opponent => {
            as_this_permanent_enters_choose_opponent_from_pair(pair)
        }
        Rule::source_enters_with_pt_counters => source_enters_with_pt_counters_from_pair(pair),
        Rule::this_ability_cant_cause_total_pt_counters_greater_than => {
            this_ability_cant_cause_total_pt_counters_greater_than_from_pair(pair)
        }
        Rule::if_this_ability_activated_at_least_times_this_turn_sacrifice_source_at_next_end_step => {
            if_this_ability_activated_at_least_times_this_turn_sacrifice_source_at_next_end_step_from_pair(pair)
        }
        Rule::activate_only_during_your_upkeep => Ok(Statement::ActivateOnlyDuringYourUpkeep),
        Rule::activate_only_during_your_turn => Ok(Statement::ActivateOnlyDuringYourTurn),
        Rule::keyword_ability => Ok(Statement::Keyword(keyword_from_pair(pair)?)),
        Rule::static_as_long_as
        | Rule::static_colored_permanents_get
        | Rule::static_status_creatures_you_control_get
        | Rule::static_enchanted_gets_with_definitions
        | Rule::static_enchanted_gets
        | Rule::static_enchanted_has_keyword_and_cant_be_enchanted_by_other_auras
        | Rule::static_enchanted_has_keyword
        | Rule::static_enchanted_loses_keyword
        | Rule::static_enchanted_loses_keyword_fragment
        | Rule::static_enchanted_is_basic_land_type
        | Rule::static_enchanted_can_attack_as_though
        | Rule::static_you_control_enchanted
        | Rule::static_you_may_have_source_enter_as_copy
        | Rule::static_source_doesnt_untap_during_your_untap_step
        | Rule::static_basic_lands_are_basic_lands
        | Rule::static_that_permanent_is_basic_land_type_while_has_named_counter
        | Rule::target_creature_defending_player_controls_can_block_any_number
        | Rule::it_blocks_each_attacking_creature_if_able
        | Rule::this_turn_defending_players_make_random_blocking_piles
        | Rule::additional_blockers_may_be_put_into_additional_piles
        | Rule::assign_each_pile_to_attacking_creature_at_random
        | Rule::creatures_in_assigned_pile_block_if_able
        | Rule::static_effect_doesnt_remove_this_aura => {
            Ok(Statement::StaticAbility(static_ability_from_pair(pair)?))
        }
        Rule::activated_ability => Ok(Statement::ActivatedAbility(activated_ability_from_pair(
            pair,
        )?)),
        Rule::triggered_ability => Ok(Statement::TriggeredAbility(triggered_ability_from_pair(
            pair,
        )?)),
        Rule::if_source_on_battlefield_flip_onto_battlefield_from_height
        | Rule::if_source_turns_over_destroy_touched_nontoken_permanents
        | Rule::then_destroy_source => {
            Ok(Statement::PhysicalAction(physical_action_from_pair(pair)?))
        }
        _ => Err(ParseError::Internal("statement")),
    }
}

fn modal_choice_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let modes = pair
        .into_inner()
        .filter(|child| child.as_rule() == Rule::modal_mode)
        .map(modal_mode_from_pair)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Statement::ModalChoice { modes })
}

fn modal_mode_from_pair(pair: Pair<Rule>) -> Result<ModalMode, ParseError> {
    let effect = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("modal_mode missing effect"))?;
    match effect.as_rule() {
        Rule::counter_target_colored_spell => {
            let color = effect
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("counter mode missing color"))?;
            Ok(ModalMode::CounterTargetColoredSpell {
                color: color_from_pair(color)?,
            })
        }
        Rule::destroy_target_colored_permanent => {
            let color = effect
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("destroy mode missing color"))?;
            Ok(ModalMode::DestroyTargetColoredPermanent {
                color: color_from_pair(color)?,
            })
        }
        _ => Err(ParseError::Internal("modal_effect")),
    }
}

fn cast_restriction_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let timing = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("cast_restriction missing timing"))?;
    let restriction = match timing.as_rule() {
        Rule::before_step => {
            let step_pair = timing
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("before_step missing step"))?;
            CastRestriction::BeforeStep {
                step: step_from_pair(step_pair)?,
            }
        }
        Rule::during_your_step => {
            let step_pair = timing
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("during_your_step missing step"))?;
            CastRestriction::DuringYourStep {
                step: step_from_pair(step_pair)?,
            }
        }
        Rule::during_combat_before_blockers_declared => {
            CastRestriction::DuringCombatBeforeBlockersAreDeclared
        }
        _ => return Err(ParseError::Internal("cast_timing")),
    };
    Ok(Statement::CastRestriction(restriction))
}

fn step_from_pair(pair: Pair<Rule>) -> Result<Step, ParseError> {
    if pair.as_rule() != Rule::step {
        return Err(ParseError::Internal("step"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "combat damage" => Ok(Step::CombatDamage),
        "declare attackers" => Ok(Step::DeclareAttackers),
        _ => Err(ParseError::Internal("step variant")),
    }
}

fn you_own_target_card_in_zone_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let zone_pair = pair.into_inner().next().ok_or(ParseError::Internal(
        "you_own_target_card_in_zone missing zone",
    ))?;
    Ok(Statement::YouOwnTargetCardInZone {
        zone: zone_from_pair(zone_pair)?,
    })
}

fn imperative_action_sequence_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let actions = pair
        .into_inner()
        .map(imperative_action_from_pair)
        .collect::<Result<Vec<_>, _>>()?;
    if actions.len() < 2 {
        return Err(ParseError::Internal("imperative action sequence"));
    }
    Ok(Statement::ImperativeActionSequence { actions })
}

fn imperative_action_from_pair(pair: Pair<Rule>) -> Result<ImperativeAction, ParseError> {
    match pair.as_rule() {
        Rule::discard_your_hand_action => Ok(ImperativeAction::DiscardYourHand),
        Rule::ante_top_card_of_your_library_action => {
            Ok(ImperativeAction::AnteTopCardOfYourLibrary)
        }
        Rule::search_your_library_for_a_card_action => {
            Ok(ImperativeAction::SearchYourLibraryForACard)
        }
        Rule::put_that_card_into_your_hand_action => Ok(ImperativeAction::PutThatCardIntoYourHand),
        Rule::shuffle_action => Ok(ImperativeAction::Shuffle),
        Rule::draw_cards_action => {
            let count_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("draw action missing count"))?;
            Ok(ImperativeAction::DrawCards {
                count: card_count_from_pair(count_pair)?,
            })
        }
        Rule::tap_source_action => {
            let source_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("tap action missing source"))?;
            Ok(ImperativeAction::TapSource {
                source: source_object_from_pair(source_pair)?,
            })
        }
        Rule::sacrifice_permanent_of_opponents_choice_action => {
            let permanent_type_pair = pair
                .into_inner()
                .find(|child| child.as_rule() == Rule::permanent_type)
                .ok_or(ParseError::Internal(
                    "sacrifice opponent choice action missing permanent_type",
                ))?;
            Ok(ImperativeAction::SacrificePermanentOfOpponentsChoice {
                permanent_type: permanent_type_from_pair(permanent_type_pair)?,
            })
        }
        _ => Err(ParseError::Internal("imperative action")),
    }
}

fn each_player_performs_action_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let action_pair = pair.into_inner().next().ok_or(ParseError::Internal(
        "each_player_performs_action missing action",
    ))?;
    Ok(Statement::EachPlayerPerformsAction {
        action: each_player_action_from_pair(action_pair)?,
    })
}

fn each_player_action_from_pair(pair: Pair<Rule>) -> Result<EachPlayerAction, ParseError> {
    match pair.as_rule() {
        Rule::each_player_antes_top_card_of_their_library_action => {
            Ok(EachPlayerAction::AnteTopCardOfTheirLibrary)
        }
        _ => Err(ParseError::Internal("each player action")),
    }
}

fn target_permanent_gains_keyword_and_gets_eot_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let pt_pair = inner
        .next()
        .ok_or(ParseError::Internal("target gains missing permanent_type"))?;
    let keyword_pair = inner
        .next()
        .ok_or(ParseError::Internal("target gains missing keyword"))?;
    let modifier_pair = inner
        .next()
        .ok_or(ParseError::Internal("target gains missing modifier"))?;
    let where_pair = inner
        .next()
        .ok_or(ParseError::Internal("target gains missing where_clause"))?;
    Ok(
        Statement::TargetPermanentGainsKeywordAndGetsUntilEndOfTurn {
            permanent_type: permanent_type_from_pair(pt_pair)?,
            keyword: keyword_from_inner_pair(keyword_pair)?,
            modifier: mixed_pt_modifier_from_pair(modifier_pair)?,
            definitions: where_clause_from_pair(where_pair)?,
        },
    )
}

fn target_spell_or_permanent_becomes_color_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let color_pair = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("target becomes color missing color"))?;
    Ok(Statement::TargetSpellOrPermanentBecomesColor {
        color: color_from_pair(color_pair)?,
    })
}

fn each_player_equalizes_controlled_permanents_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mut types = pair.into_inner();
    let chosen_type = types.next().ok_or(ParseError::Internal(
        "equalize permanents missing chosen permanent_type_plural",
    ))?;
    let comparison_type = types.next().ok_or(ParseError::Internal(
        "equalize permanents missing comparison permanent_type_plural",
    ))?;
    let permanent_type = permanent_type_from_plural_pair(chosen_type)?;
    if permanent_type != permanent_type_from_plural_pair(comparison_type)? {
        return Err(ParseError::Internal("equalize permanents type mismatch"));
    }
    Ok(Statement::EachPlayerEqualizesControlledPermanents { permanent_type })
}

fn players_do_actions_the_same_way_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let actions = pair
        .into_inner()
        .map(balance_same_way_action_from_pair)
        .collect::<Result<Vec<_>, _>>()?;
    if actions.is_empty() {
        return Err(ParseError::Internal("same-way action list"));
    }
    Ok(Statement::PlayersDoActionsTheSameWay { actions })
}

fn as_this_permanent_enters_choose_opponent_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let source_pair = pair.into_inner().next().ok_or(ParseError::Internal(
        "as enters choose opponent missing source",
    ))?;
    let permanent_type = match source_object_from_pair(source_pair)? {
        SourceObject::This(permanent_type) => permanent_type,
        SourceObject::ThisAura => return Err(ParseError::Internal("as enters source is aura")),
    };
    Ok(Statement::AsThisPermanentEntersChooseOpponent { permanent_type })
}

fn source_enters_with_pt_counters_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = inner
        .next()
        .ok_or(ParseError::Internal("enters counters missing source"))?;
    let amount_pair = inner
        .next()
        .ok_or(ParseError::Internal("enters counters missing amount"))?;
    let counter_pair = inner
        .next()
        .ok_or(ParseError::Internal("enters counters missing counter"))?;
    let amount =
        number_word_to_u32(amount_pair.as_str()).ok_or(ParseError::Internal("number_word"))?;
    Ok(Statement::ThisPermanentEntersWithCounters {
        source: source_object_from_pair(source_pair)?,
        amount,
        counter: pt_modifier_from_counter_pair(counter_pair)?,
    })
}

fn this_ability_cant_cause_total_pt_counters_greater_than_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let counter_pair = inner
        .next()
        .ok_or(ParseError::Internal("counter cap missing counter"))?;
    let source_pair = inner
        .next()
        .ok_or(ParseError::Internal("counter cap missing source"))?;
    let maximum_pair = inner
        .next()
        .ok_or(ParseError::Internal("counter cap missing maximum"))?;
    let maximum =
        number_word_to_u32(maximum_pair.as_str()).ok_or(ParseError::Internal("number_word"))?;
    Ok(Statement::ThisAbilityCantCauseTotalCountersGreaterThan {
        counter: pt_modifier_from_counter_pair(counter_pair)?,
        source: source_object_from_pair(source_pair)?,
        maximum,
    })
}

fn if_this_ability_activated_at_least_times_this_turn_sacrifice_source_at_next_end_step_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let threshold_pair = inner
        .next()
        .ok_or(ParseError::Internal("activation threshold missing count"))?;
    let source_pair = inner
        .next()
        .ok_or(ParseError::Internal("activation threshold missing source"))?;
    let threshold =
        number_word_to_u32(threshold_pair.as_str()).ok_or(ParseError::Internal("number_word"))?;
    Ok(
        Statement::IfThisAbilityActivatedAtLeastTimesThisTurnSacrificeSourceAtNextEndStep {
            threshold,
            source: source_object_from_pair(source_pair)?,
        },
    )
}

fn balance_same_way_action_from_pair(pair: Pair<Rule>) -> Result<BalanceSameWayAction, ParseError> {
    match pair.as_rule() {
        Rule::discard_cards_action => Ok(BalanceSameWayAction::DiscardCards),
        Rule::sacrifice_permanents_action => {
            let pt = pair.into_inner().next().ok_or(ParseError::Internal(
                "sacrifice action missing permanent_type_plural",
            ))?;
            Ok(BalanceSameWayAction::SacrificePermanents {
                permanent_type: permanent_type_from_plural_pair(pt)?,
            })
        }
        _ => Err(ParseError::Internal("same-way action")),
    }
}

fn destroy_all_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let pt = pair
        .into_inner()
        .next()
        .expect("destroy_all always contains a permanent_type_plural");
    Ok(Statement::DestroyAll {
        permanent_type: permanent_type_from_plural_pair(pt)?,
    })
}

fn destroy_target_permanent_choice_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let permanent_types = pair
        .into_inner()
        .map(permanent_type_from_pair)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Statement::DestroyTargetPermanentChoice { permanent_types })
}

fn named_source_deals_variable_damage_to_any_target_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = inner
        .next()
        .ok_or(ParseError::Internal("named damage missing source name"))?;
    let amount_pair = inner
        .next()
        .ok_or(ParseError::Internal("named damage missing amount"))?;
    Ok(Statement::NamedSourceDealsVariableDamageToAnyTarget {
        source_name: source_pair.as_str().to_string(),
        amount: variable_from_str(amount_pair.as_str())?,
    })
}

fn named_source_deals_variable_damage_to_damage_recipients_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = inner
        .next()
        .ok_or(ParseError::Internal("named damage missing source name"))?;
    let amount_pair = inner
        .next()
        .ok_or(ParseError::Internal("named damage missing amount"))?;
    let recipients = inner
        .map(damage_recipient_from_pair)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        Statement::NamedSourceDealsVariableDamageToDamageRecipients {
            source_name: source_pair.as_str().to_string(),
            amount: variable_from_str(amount_pair.as_str())?,
            recipients,
        },
    )
}

fn damage_recipient_from_pair(pair: Pair<Rule>) -> Result<DamageRecipient, ParseError> {
    let inner = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("damage recipient missing inner rule"))?;
    match inner.as_rule() {
        Rule::each_creature_without_keyword => {
            let keyword_pair = inner.into_inner().next().ok_or(ParseError::Internal(
                "creature damage recipient missing keyword",
            ))?;
            Ok(DamageRecipient::EachCreatureWithoutKeyword {
                keyword: keyword_from_inner_pair(keyword_pair)?,
            })
        }
        Rule::each_player_damage_recipient => Ok(DamageRecipient::EachPlayer),
        _ => Err(ParseError::Internal("damage recipient")),
    }
}

fn spend_only_color_mana_on_variable_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let color_pair = inner
        .next()
        .ok_or(ParseError::Internal("spend-only restriction missing color"))?;
    let variable_pair = inner.next().ok_or(ParseError::Internal(
        "spend-only restriction missing variable",
    ))?;
    Ok(Statement::SpendOnlyColorManaOnVariable {
        color: color_from_pair(color_pair)?,
        variable: variable_from_str(variable_pair.as_str())?,
    })
}

fn you_gain_life_equal_damage_dealt_capped_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let caps = pair
        .into_inner()
        .map(damage_life_gain_cap_from_pair)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(Statement::YouGainLifeEqualToDamageDealtCapped { caps })
}

fn damage_life_gain_cap_from_pair(pair: Pair<Rule>) -> Result<DamageLifeGainCap, ParseError> {
    match pair.as_rule() {
        Rule::player_life_total_before_damage_dealt => {
            Ok(DamageLifeGainCap::PlayerLifeTotalBeforeDamageDealt)
        }
        Rule::planeswalker_loyalty_before_damage_dealt => {
            Ok(DamageLifeGainCap::PlaneswalkerLoyaltyBeforeDamageDealt)
        }
        Rule::creature_toughness => Ok(DamageLifeGainCap::CreatureToughness),
        _ => Err(ParseError::Internal("damage_life_gain_cap")),
    }
}

fn if_its_permanent_cant_be_regenerated_and_would_die_exile_instead_this_turn_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let permanent_type_pair = pair.into_inner().next().ok_or(ParseError::Internal(
        "conditional exile missing permanent type",
    ))?;
    Ok(
        Statement::IfItsPermanentCantBeRegeneratedAndWouldDieExileInsteadThisTurn {
            permanent_type: permanent_type_from_pair(permanent_type_pair)?,
        },
    )
}

fn draw_cards_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let count_pair = pair
        .into_inner()
        .next()
        .expect("draw_cards always contains a draw_count");
    Ok(Statement::TargetPlayerDrawsCards {
        count: card_count_from_pair(count_pair)?,
    })
}

fn target_player_activates_mana_ability_of_each_permanent_they_control_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let permanent_type_pair = pair.into_inner().next().ok_or(ParseError::Internal(
        "target player activates mana ability missing permanent_type",
    ))?;
    Ok(
        Statement::TargetPlayerActivatesManaAbilityOfEachPermanentTheyControl {
            permanent_type: permanent_type_from_pair(permanent_type_pair)?,
        },
    )
}

fn card_count_from_pair(count_pair: Pair<Rule>) -> Result<CardCount, ParseError> {
    let count = match count_pair.as_rule() {
        Rule::number_word => {
            let count = number_word_to_u32(count_pair.as_str())
                .ok_or(ParseError::Internal("number_word"))?;
            CardCount::Number(count)
        }
        Rule::variable_name => CardCount::Variable(variable_from_str(count_pair.as_str())?),
        _ => return Err(ParseError::Internal("draw_count")),
    };
    Ok(count)
}

fn until_eot_you_may_pay_cost_at_timing_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let timing_pair = inner
        .next()
        .ok_or(ParseError::Internal("until_eot missing action timing"))?;
    let cost_pair = inner
        .next()
        .ok_or(ParseError::Internal("until_eot missing optional cost"))?;
    Ok(Statement::UntilEndOfTurnYouMayPayCostAtTiming {
        timing: action_timing_from_pair(timing_pair)?,
        cost: optional_cost_from_pair(cost_pair)?,
    })
}

fn if_you_do_add_mana_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let mana_pair = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("if_you_do_add_mana missing add_mana"))?
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("add_mana missing mana_cost"))?;
    Ok(Statement::IfYouDoAddMana {
        mana: mana_cost_from_pair(mana_pair),
    })
}

fn add_mana_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let mana_pair = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("add_mana missing mana_cost"))?;
    Ok(Statement::AddMana {
        mana: mana_cost_from_pair(mana_pair),
    })
}

fn if_you_do_gain_life_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let amount_pair = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("if_you_do_gain_life missing amount"))?;
    let amount = amount_pair
        .as_str()
        .parse::<u32>()
        .map_err(|_| ParseError::Internal("if_you_do_gain_life amount"))?;
    Ok(Statement::IfYouDoGainLife { amount })
}

fn action_timing_from_pair(pair: Pair<Rule>) -> Result<ActionTiming, ParseError> {
    match pair.as_rule() {
        Rule::any_time_you_could_activate_a_mana_ability => {
            Ok(ActionTiming::AnyTimeYouCouldActivateAManaAbility)
        }
        _ => Err(ParseError::Internal("action_timing")),
    }
}

fn optional_cost_from_pair(pair: Pair<Rule>) -> Result<OptionalCost, ParseError> {
    match pair.as_rule() {
        Rule::pay_life_cost => {
            let amount_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("pay_life_cost missing amount"))?;
            let amount = amount_pair
                .as_str()
                .parse::<u32>()
                .map_err(|_| ParseError::Internal("pay_life_cost amount"))?;
            Ok(OptionalCost::PayLife { amount })
        }
        _ => Err(ParseError::Internal("optional_cost")),
    }
}

fn number_word_to_u32(word: &str) -> Option<u32> {
    match word.to_ascii_lowercase().as_str() {
        "one" => Some(1),
        "two" => Some(2),
        "three" => Some(3),
        "four" => Some(4),
        "five" => Some(5),
        "six" => Some(6),
        "seven" => Some(7),
        "eight" => Some(8),
        "nine" => Some(9),
        "ten" => Some(10),
        _ => None,
    }
}

fn keyword_from_pair(pair: Pair<Rule>) -> Result<Keyword, ParseError> {
    let inner = pair
        .into_inner()
        .next()
        .expect("keyword_ability always contains a keyword");
    keyword_from_inner_pair(inner)
}

fn triggered_ability_from_pair(pair: Pair<Rule>) -> Result<TriggeredAbility, ParseError> {
    let mut event: Option<TriggerEvent> = None;
    let mut intervening_if: Option<InterveningIf> = None;
    let mut effects: Vec<TriggerEffect> = Vec::new();
    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::aura_enters => event = Some(TriggerEvent::ThisAuraEnters),
            Rule::aura_leaves_battlefield => {
                event = Some(TriggerEvent::ThisAuraLeavesTheBattlefield);
            }
            Rule::permanent_enters => {
                event = Some(permanent_enters_from_pair(child)?);
            }
            Rule::player_casts_colored_spell => {
                event = Some(player_casts_colored_spell_from_pair(child)?);
            }
            Rule::enchanted_permanent_dies => {
                event = Some(enchanted_permanent_dies_from_pair(child)?);
            }
            Rule::beginning_of_the_next_end_step => {
                event = Some(TriggerEvent::BeginningOfTheNextEndStep);
            }
            Rule::beginning_of_chosen_players_upkeep => {
                event = Some(TriggerEvent::BeginningOfChosenPlayersUpkeep);
            }
            Rule::beginning_of_each_players_upkeep => {
                event = Some(TriggerEvent::BeginningOfEachPlayersUpkeep);
            }
            Rule::beginning_of_your_upkeep => {
                event = Some(TriggerEvent::BeginningOfYourUpkeep);
            }
            Rule::source_put_into_graveyard_from_battlefield => {
                event = Some(source_put_into_graveyard_from_battlefield_from_pair(child)?);
            }
            Rule::permanent_put_into_graveyard_from_battlefield => {
                event = Some(permanent_put_into_graveyard_from_battlefield_from_pair(
                    child,
                )?);
            }
            Rule::beginning_of_upkeep_of_enchanted_permanent_controller => {
                event = Some(beginning_of_upkeep_of_enchanted_permanent_controller_from_pair(
                    child,
                )?);
            }
            Rule::end_of_combat => {
                event = Some(TriggerEvent::EndOfCombat);
            }
            Rule::source_blocks_or_becomes_blocked_by_non_creature_type_creature => {
                event = Some(
                    source_blocks_or_becomes_blocked_by_non_creature_type_creature_from_pair(
                        child,
                    )?,
                );
            }
            Rule::its_on_the_battlefield => {
                intervening_if = Some(InterveningIf::ItsOnTheBattlefield);
            }
            Rule::source_attacked_or_blocked_this_combat => {
                let source_pair = child.into_inner().next().ok_or(ParseError::Internal(
                    "attacked-or-blocked condition missing source",
                ))?;
                intervening_if = Some(InterveningIf::SourceAttackedOrBlockedThisCombat {
                    source: source_object_from_pair(source_pair)?,
                });
            }
            Rule::enchanted_has_keyword => {
                let mut inner = child.into_inner();
                let object_pair = inner.next().ok_or(ParseError::Internal(
                    "enchanted-has condition missing object",
                ))?;
                let keyword_pair = inner.next().ok_or(ParseError::Internal(
                    "enchanted-has condition missing keyword",
                ))?;
                intervening_if = Some(InterveningIf::EnchantedHasKeyword {
                    object: enchanted_object_from_pair(object_pair)?,
                    keyword: keyword_from_inner_pair(keyword_pair)?,
                });
            }
            Rule::destroy_that_creature_if_it_attacked_this_turn => {
                effects.push(TriggerEffect::DestroyThatCreatureIfItAttackedThisTurn);
            }
            Rule::destroy_that_creature_at_end_of_combat => {
                effects.push(TriggerEffect::DestroyThatCreatureAtEndOfCombat);
            }
            Rule::that_creatures_controller_sacrifices_it => {
                effects.push(TriggerEffect::ThatCreaturesControllerSacrificesIt);
            }
            Rule::loses_and_gains_keyword => {
                effects.push(loses_and_gains_keyword_from_pair(child)?);
            }
            Rule::return_enchanted_card_and_attach => {
                effects.push(return_enchanted_card_and_attach_from_pair(child)?);
            }
            Rule::sacrifice_source_unless_you_pay => {
                effects.push(sacrifice_source_unless_you_pay_from_pair(child)?);
            }
            Rule::you_may_pay_mana => {
                effects.push(you_may_pay_mana_from_pair(child)?);
            }
            Rule::unless_you_pay_mana_do_actions => {
                effects.push(unless_you_pay_mana_do_actions_from_pair(child)?);
            }
            Rule::delayed_remove_all_named_counters_from_linked_land => {
                effects.push(delayed_remove_all_named_counters_from_linked_land_from_pair(
                    child,
                )?);
            }
            Rule::source_deals_damage_to_that_permanents_controller => {
                effects.push(source_deals_damage_to_that_permanents_controller_from_pair(
                    child,
                )?);
            }
            Rule::source_deals_damage_equal_to_that_permanents_toughness_to_the_permanents_controller => {
                effects.push(source_deals_damage_equal_to_that_permanents_toughness_to_the_permanents_controller_from_pair(child)?);
            }
            Rule::source_deals_damage_to_that_player => {
                effects.push(source_deals_damage_to_that_player_from_pair(child)?);
            }
            Rule::source_deals_damage_to_that_permanent => {
                effects.push(source_deals_damage_to_that_permanent_from_pair(child)?);
            }
            Rule::source_deals_variable_damage_to_that_player => {
                effects.push(source_deals_variable_damage_to_that_player_from_pair(
                    child,
                )?);
            }
            Rule::source_gains_static_ability => {
                effects.push(source_gains_static_ability_from_pair(child)?);
            }
            Rule::remove_pt_counter_from_it => {
                effects.push(remove_pt_counter_from_it_from_pair(child)?);
            }
            _ => return Err(ParseError::Internal("triggered_ability child")),
        }
    }
    let event = event.ok_or(ParseError::Internal("triggered_ability missing event"))?;
    if effects.is_empty() {
        return Err(ParseError::Internal("triggered_ability missing effect"));
    }
    Ok(TriggeredAbility {
        event,
        intervening_if,
        effects,
    })
}

fn permanent_enters_from_pair(pair: Pair<Rule>) -> Result<TriggerEvent, ParseError> {
    let pt = pair.into_inner().next().ok_or(ParseError::Internal(
        "permanent_enters missing permanent_type",
    ))?;
    Ok(TriggerEvent::PermanentEnters {
        permanent_type: permanent_type_from_pair(pt)?,
    })
}

fn player_casts_colored_spell_from_pair(pair: Pair<Rule>) -> Result<TriggerEvent, ParseError> {
    let color = pair.into_inner().next().ok_or(ParseError::Internal(
        "player_casts_colored_spell missing color",
    ))?;
    Ok(TriggerEvent::PlayerCastsColoredSpell {
        color: color_from_pair(color)?,
    })
}

fn enchanted_permanent_dies_from_pair(pair: Pair<Rule>) -> Result<TriggerEvent, ParseError> {
    let pt = pair.into_inner().next().ok_or(ParseError::Internal(
        "enchanted_permanent_dies missing permanent_type",
    ))?;
    Ok(TriggerEvent::EnchantedPermanentDies {
        permanent_type: permanent_type_from_pair(pt)?,
    })
}

fn beginning_of_upkeep_of_enchanted_permanent_controller_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEvent, ParseError> {
    let pt = pair.into_inner().next().ok_or(ParseError::Internal(
        "beginning_of_upkeep_of_enchanted_permanent_controller missing permanent_type",
    ))?;
    Ok(
        TriggerEvent::BeginningOfUpkeepOfEnchantedPermanentController {
            permanent_type: permanent_type_from_pair(pt)?,
        },
    )
}

fn source_put_into_graveyard_from_battlefield_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEvent, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = inner.next().ok_or(ParseError::Internal(
        "put-into-graveyard event missing source",
    ))?;
    let zone_pair = inner.next().ok_or(ParseError::Internal(
        "put-into-graveyard event missing zone",
    ))?;
    if zone_from_pair(zone_pair)? != Zone::Graveyard {
        return Err(ParseError::Internal("put-into-graveyard event zone"));
    }
    Ok(TriggerEvent::SourcePutIntoGraveyardFromBattlefield {
        source: source_object_from_pair(source_pair)?,
    })
}

fn permanent_put_into_graveyard_from_battlefield_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEvent, ParseError> {
    let mut inner = pair.into_inner();
    let permanent_type_pair = inner.next().ok_or(ParseError::Internal(
        "put-into-graveyard event missing permanent_type",
    ))?;
    let zone_pair = inner.next().ok_or(ParseError::Internal(
        "put-into-graveyard event missing zone",
    ))?;
    if zone_from_pair(zone_pair)? != Zone::Graveyard {
        return Err(ParseError::Internal("put-into-graveyard event zone"));
    }
    Ok(TriggerEvent::PermanentPutIntoGraveyardFromBattlefield {
        permanent_type: permanent_type_from_pair(permanent_type_pair)?,
    })
}

fn source_blocks_or_becomes_blocked_by_non_creature_type_creature_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEvent, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = inner.next().ok_or(ParseError::Internal(
        "blocks-or-blocked event missing source",
    ))?;
    let excluded_type_pair = inner.next().ok_or(ParseError::Internal(
        "blocks-or-blocked event missing excluded creature type",
    ))?;
    Ok(
        TriggerEvent::SourceBlocksOrBecomesBlockedByNonCreatureTypeCreature {
            source: source_object_from_pair(source_pair)?,
            excluded_type: creature_type_from_pair(excluded_type_pair)?,
        },
    )
}

fn loses_and_gains_keyword_from_pair(pair: Pair<Rule>) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let loses_pair = inner.next().ok_or(ParseError::Internal(
        "loses_and_gains missing loses keyword",
    ))?;
    let gains_pair = inner.next().ok_or(ParseError::Internal(
        "loses_and_gains missing gains keyword",
    ))?;
    Ok(TriggerEffect::LosesAndGainsKeyword {
        loses: keyword_from_inner_pair(loses_pair)?,
        gains: keyword_from_inner_pair(gains_pair)?,
    })
}

fn keyword_from_inner_pair(pair: Pair<Rule>) -> Result<Keyword, ParseError> {
    match pair.as_rule() {
        Rule::flying => Ok(Keyword::Flying),
        Rule::first_strike => Ok(Keyword::FirstStrike),
        Rule::defender => Ok(Keyword::Defender),
        Rule::banding => Ok(Keyword::Banding),
        Rule::trample => Ok(Keyword::Trample),
        Rule::mountainwalk => Ok(Keyword::Mountainwalk),
        Rule::swampwalk => Ok(Keyword::Swampwalk),
        Rule::indestructible => Ok(Keyword::Indestructible),
        Rule::protection => {
            let color = pair
                .into_inner()
                .next()
                .expect("protection always names a color");
            Ok(Keyword::Protection(color_from_pair(color)?))
        }
        Rule::enchant => {
            let object = pair
                .into_inner()
                .next()
                .expect("enchant always contains an enchant_object alternative");
            Ok(Keyword::Enchant(enchant_object_from_pair(object)?))
        }
        _ => Err(ParseError::Internal("quoted keyword")),
    }
}

fn return_enchanted_card_and_attach_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEffect, ParseError> {
    let pt = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("return_enchanted missing card_type"))?;
    Ok(TriggerEffect::ReturnEnchantedCardAndAttach {
        card_type: permanent_type_from_pair(pt)?,
    })
}

fn sacrifice_source_unless_you_pay_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = inner
        .next()
        .ok_or(ParseError::Internal("sacrifice unless missing source"))?;
    let cost_pair = inner
        .next()
        .ok_or(ParseError::Internal("sacrifice unless missing cost"))?;
    Ok(TriggerEffect::SacrificeSourceUnlessYouPay {
        source: source_object_from_pair(source_pair)?,
        cost: mana_cost_from_pair(cost_pair),
    })
}

fn you_may_pay_mana_from_pair(pair: Pair<Rule>) -> Result<TriggerEffect, ParseError> {
    let cost_pair = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("you_may_pay_mana missing cost"))?;
    Ok(TriggerEffect::YouMayPayMana {
        cost: mana_cost_from_pair(cost_pair),
    })
}

fn unless_you_pay_mana_do_actions_from_pair(pair: Pair<Rule>) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let cost_pair = inner
        .next()
        .ok_or(ParseError::Internal("unless pay missing mana cost"))?;
    let actions = inner
        .map(imperative_action_from_pair)
        .collect::<Result<Vec<_>, _>>()?;
    if actions.is_empty() {
        return Err(ParseError::Internal("unless pay missing actions"));
    }
    Ok(TriggerEffect::UnlessYouPayManaDoActions {
        cost: mana_cost_from_pair(cost_pair),
        actions,
    })
}

fn source_deals_damage_to_that_permanents_controller_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = inner
        .next()
        .ok_or(ParseError::Internal("damage effect missing source"))?;
    let amount_pair = inner
        .next()
        .ok_or(ParseError::Internal("damage effect missing amount"))?;
    let recipient_pair = inner
        .next()
        .ok_or(ParseError::Internal("damage effect missing recipient"))?;
    let amount = amount_pair
        .as_str()
        .parse::<u32>()
        .map_err(|_| ParseError::Internal("damage amount"))?;
    Ok(TriggerEffect::SourceDealsDamageToThatPermanentController {
        source: source_object_from_pair(source_pair)?,
        amount,
        recipient: that_permanents_controller_from_pair(recipient_pair)?,
    })
}

fn source_deals_damage_equal_to_that_permanents_toughness_to_the_permanents_controller_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = inner.next().ok_or(ParseError::Internal(
        "toughness damage effect missing source",
    ))?;
    let toughness_pair = inner.next().ok_or(ParseError::Internal(
        "toughness damage effect missing toughness reference",
    ))?;
    let controller_pair = inner.next().ok_or(ParseError::Internal(
        "toughness damage effect missing controller reference",
    ))?;
    let toughness_permanent_type = permanent_type_from_inner_pair(toughness_pair)?;
    let controller_permanent_type = permanent_type_from_inner_pair(controller_pair)?;
    if toughness_permanent_type != controller_permanent_type {
        return Err(ParseError::Internal("toughness damage references mismatch"));
    }
    Ok(
        TriggerEffect::SourceDealsDamageEqualToThatPermanentsToughnessToThePermanentsController {
            source: source_object_from_pair(source_pair)?,
            permanent_type: toughness_permanent_type,
        },
    )
}

fn source_deals_damage_to_that_player_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = inner.next().ok_or(ParseError::Internal(
        "damage-to-player effect missing source",
    ))?;
    let amount_pair = inner.next().ok_or(ParseError::Internal(
        "damage-to-player effect missing amount",
    ))?;
    let amount = amount_pair
        .as_str()
        .parse::<u32>()
        .map_err(|_| ParseError::Internal("damage-to-player amount"))?;
    Ok(TriggerEffect::SourceDealsDamageToThatPlayer {
        source: source_object_from_pair(source_pair)?,
        amount,
    })
}

fn source_deals_damage_to_that_permanent_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = inner.next().ok_or(ParseError::Internal(
        "damage-to-permanent effect missing source",
    ))?;
    let amount_pair = inner.next().ok_or(ParseError::Internal(
        "damage-to-permanent effect missing amount",
    ))?;
    let recipient_pair = inner.next().ok_or(ParseError::Internal(
        "damage-to-permanent effect missing recipient",
    ))?;
    let amount = amount_pair
        .as_str()
        .parse::<u32>()
        .map_err(|_| ParseError::Internal("damage-to-permanent amount"))?;
    Ok(TriggerEffect::SourceDealsDamageToThatPermanent {
        source: source_object_from_pair(source_pair)?,
        amount,
        recipient: permanent_type_from_pair(recipient_pair)?,
    })
}

fn source_deals_variable_damage_to_that_player_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = inner.next().ok_or(ParseError::Internal(
        "variable damage effect missing source",
    ))?;
    let amount_pair = inner.next().ok_or(ParseError::Internal(
        "variable damage effect missing amount",
    ))?;
    let where_pair = inner.next().ok_or(ParseError::Internal(
        "variable damage effect missing where_clause",
    ))?;
    let amount = variable_from_str(amount_pair.as_str())?;
    let definitions = where_clause_from_pair(where_pair)?;
    if !definitions
        .iter()
        .any(|definition| definition.variable == amount)
    {
        return Err(ParseError::Internal(
            "variable damage missing amount definition",
        ));
    }
    Ok(TriggerEffect::SourceDealsVariableDamageToThatPlayer {
        source: source_object_from_pair(source_pair)?,
        amount,
        definitions,
    })
}

fn source_gains_static_ability_from_pair(pair: Pair<Rule>) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = inner
        .next()
        .ok_or(ParseError::Internal("source gains static missing source"))?;
    let ability_pair = inner
        .next()
        .ok_or(ParseError::Internal("source gains static missing ability"))?;
    Ok(TriggerEffect::SourceGainsStaticAbility {
        source: source_object_from_pair(source_pair)?,
        ability: static_ability_from_pair(ability_pair)?,
    })
}

fn remove_pt_counter_from_it_from_pair(pair: Pair<Rule>) -> Result<TriggerEffect, ParseError> {
    let counter_pair = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("remove counter missing counter"))?;
    Ok(TriggerEffect::RemoveCounterFromIt {
        counter: pt_modifier_from_counter_pair(counter_pair)?,
    })
}

fn delayed_remove_all_named_counters_from_linked_land_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let plural_counter_pair = inner.next().ok_or(ParseError::Internal(
        "delayed remove counters missing plural counter",
    ))?;
    let permanent_type_pair = inner.next().ok_or(ParseError::Internal(
        "delayed remove counters missing permanent type",
    ))?;
    let put_counter_pair = inner.next().ok_or(ParseError::Internal(
        "delayed remove counters missing put counter",
    ))?;
    let put_source_pair = inner.next().ok_or(ParseError::Internal(
        "delayed remove counters missing put source",
    ))?;
    let removed_counter_pair = inner.next().ok_or(ParseError::Internal(
        "delayed remove counters missing removed counter",
    ))?;
    let removed_source_pair = inner.next().ok_or(ParseError::Internal(
        "delayed remove counters missing removed source",
    ))?;

    let counter_name = counter_name_from_counter_pair(plural_counter_pair)?;
    let put_counter_name = counter_name_from_counter_pair(put_counter_pair)?;
    let removed_counter_name = counter_name_from_counter_pair(removed_counter_pair)?;
    let source = source_object_from_pair(put_source_pair)?;
    let removed_source = source_object_from_pair(removed_source_pair)?;
    if counter_name != put_counter_name || counter_name != removed_counter_name {
        return Err(ParseError::Internal(
            "delayed remove counter names mismatch",
        ));
    }
    if source != removed_source {
        return Err(ParseError::Internal(
            "delayed remove counter sources mismatch",
        ));
    }

    Ok(
        TriggerEffect::DelayedRemoveAllNamedCountersFromLinkedPermanent {
            counter_name,
            permanent_type: permanent_type_from_pair(permanent_type_pair)?,
            source,
        },
    )
}

fn source_object_from_pair(pair: Pair<Rule>) -> Result<SourceObject, ParseError> {
    if pair.as_rule() != Rule::source_object {
        return Err(ParseError::Internal("source_object"));
    }
    let kind = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("source_object missing kind"))?;
    match kind.as_rule() {
        Rule::permanent_type => Ok(SourceObject::This(permanent_type_from_pair(kind)?)),
        Rule::aura_source_object => Ok(SourceObject::ThisAura),
        _ => Err(ParseError::Internal("source_object kind")),
    }
}

fn that_permanents_controller_from_pair(pair: Pair<Rule>) -> Result<PermanentType, ParseError> {
    if pair.as_rule() != Rule::that_permanents_controller {
        return Err(ParseError::Internal("that_permanents_controller"));
    }
    let pt = pair.into_inner().next().ok_or(ParseError::Internal(
        "that_permanents_controller missing permanent_type",
    ))?;
    permanent_type_from_pair(pt)
}

fn permanent_type_from_inner_pair(pair: Pair<Rule>) -> Result<PermanentType, ParseError> {
    let pt = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("wrapper missing permanent_type"))?;
    permanent_type_from_pair(pt)
}

fn enchant_object_from_pair(pair: Pair<Rule>) -> Result<EnchantObject, ParseError> {
    match pair.as_rule() {
        Rule::enchant_permanent => {
            let pt = pair
                .into_inner()
                .next()
                .expect("enchant_permanent wraps a permanent_type");
            Ok(EnchantObject::Permanent(permanent_type_from_pair(pt)?))
        }
        Rule::enchant_creature_type => {
            let ct = pair
                .into_inner()
                .next()
                .expect("enchant_creature_type wraps a creature_type");
            Ok(EnchantObject::CreatureType(creature_type_from_pair(ct)?))
        }
        Rule::enchant_card_in_zone => {
            let mut inner = pair.into_inner();
            let card_type = inner
                .next()
                .expect("enchant_card_in_zone names the card type first");
            let zone = inner
                .next()
                .expect("enchant_card_in_zone names the zone after the article");
            Ok(EnchantObject::CardInZone {
                card_type: permanent_type_from_pair(card_type)?,
                zone: zone_from_pair(zone)?,
            })
        }
        Rule::enchant_put_onto_battlefield => {
            let pt = pair
                .into_inner()
                .next()
                .expect("enchant_put_onto_battlefield names the card type first");
            Ok(EnchantObject::PutOntoBattlefieldByThisAura {
                card_type: permanent_type_from_pair(pt)?,
            })
        }
        _ => Err(ParseError::Internal("enchant_object")),
    }
}

fn zone_from_pair(pair: Pair<Rule>) -> Result<Zone, ParseError> {
    if pair.as_rule() != Rule::zone {
        return Err(ParseError::Internal("zone"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "graveyard" => Ok(Zone::Graveyard),
        "ante" => Ok(Zone::Ante),
        _ => Err(ParseError::Internal("zone variant")),
    }
}

fn static_ability_from_pair(pair: Pair<Rule>) -> Result<StaticAbility, ParseError> {
    match pair.as_rule() {
        Rule::static_as_long_as => {
            let mut inner = pair.into_inner();
            let cond_pair = inner
                .next()
                .expect("static_as_long_as begins with a condition");
            let effect_pair = inner
                .next()
                .expect("static_as_long_as has an effect after the condition");
            Ok(StaticAbility::Conditional {
                condition: condition_from_pair(cond_pair)?,
                effect: continuous_effect_from_pair(effect_pair)?,
            })
        }
        Rule::static_colored_permanents_get => {
            let mut inner = pair.into_inner();
            let color_pair = inner
                .next()
                .expect("static_colored_permanents_get begins with a color");
            let pt_pair = inner
                .next()
                .expect("static_colored_permanents_get names the affected permanent type");
            let modifier_pair = inner
                .next()
                .expect("static_colored_permanents_get has a pt_modifier");
            Ok(StaticAbility::ColoredPermanentsGet {
                color: color_from_pair(color_pair)?,
                permanent_type: permanent_type_from_plural_pair(pt_pair)?,
                modifier: pt_modifier_from_pair(modifier_pair)?,
            })
        }
        Rule::static_status_creatures_you_control_get => {
            let mut inner = pair.into_inner();
            let status_pair = inner
                .next()
                .expect("static_status_creatures_you_control_get begins with a status");
            let modifier_pair = inner
                .next()
                .expect("static_status_creatures_you_control_get has a pt_modifier");
            Ok(StaticAbility::StatusCreaturesYouControlGet {
                status: creature_status_from_pair(status_pair)?,
                modifier: pt_modifier_from_pair(modifier_pair)?,
            })
        }
        Rule::static_enchanted_gets => {
            let mut inner = pair.into_inner();
            let pt_pair = inner
                .next()
                .expect("static_enchanted_gets begins with the enchanted permanent type");
            let modifier_pair = inner
                .next()
                .expect("static_enchanted_gets has a pt_modifier");
            Ok(StaticAbility::EnchantedGets {
                permanent_type: permanent_type_from_pair(pt_pair)?,
                modifier: pt_modifier_from_pair(modifier_pair)?,
            })
        }
        Rule::static_enchanted_gets_with_definitions => {
            let mut inner = pair.into_inner();
            let pt_pair = inner.next().expect(
                "static_enchanted_gets_with_definitions begins with the enchanted permanent type",
            );
            let modifier_pair = inner
                .next()
                .expect("static_enchanted_gets_with_definitions has a variable_pt_modifier");
            let where_pair = inner
                .next()
                .expect("static_enchanted_gets_with_definitions has a where_clause");
            Ok(StaticAbility::EnchantedGetsWithDefinitions {
                permanent_type: permanent_type_from_pair(pt_pair)?,
                modifier: variable_pt_modifier_from_pair(modifier_pair)?,
                definitions: where_clause_from_pair(where_pair)?,
            })
        }
        Rule::static_enchanted_has_keyword => {
            let mut inner = pair.into_inner();
            let object_pair = inner
                .next()
                .expect("static_enchanted_has_keyword begins with enchanted object");
            let keyword_pair = inner
                .next()
                .expect("static_enchanted_has_keyword names granted keyword");
            Ok(StaticAbility::EnchantedHasKeyword {
                object: enchanted_object_from_pair(object_pair)?,
                keyword: keyword_from_inner_pair(keyword_pair)?,
            })
        }
        Rule::static_enchanted_loses_keyword | Rule::static_enchanted_loses_keyword_fragment => {
            let mut inner = pair.into_inner();
            let object_pair = inner
                .next()
                .expect("static_enchanted_loses_keyword begins with enchanted object");
            let keyword_pair = inner
                .next()
                .expect("static_enchanted_loses_keyword names removed keyword");
            Ok(StaticAbility::EnchantedLosesKeyword {
                object: enchanted_object_from_pair(object_pair)?,
                keyword: keyword_from_inner_pair(keyword_pair)?,
            })
        }
        Rule::static_enchanted_is_basic_land_type => {
            let mut inner = pair.into_inner();
            let object_pair = inner
                .next()
                .expect("static_enchanted_is_basic_land_type begins with enchanted object");
            let land_type_pair = inner
                .next()
                .expect("static_enchanted_is_basic_land_type names basic land type");
            Ok(StaticAbility::EnchantedIsBasicLandType {
                object: enchanted_object_from_pair(object_pair)?,
                land_type: basic_land_type_from_pair(land_type_pair)?,
            })
        }
        Rule::static_enchanted_has_keyword_and_cant_be_enchanted_by_other_auras => {
            let mut inner = pair.into_inner();
            let object_pair = inner.next().expect(
                "static_enchanted_has_keyword_and_cant_be_enchanted begins with enchanted object",
            );
            let keyword_pair = inner.next().expect(
                "static_enchanted_has_keyword_and_cant_be_enchanted names granted keyword",
            );
            Ok(
                StaticAbility::EnchantedHasKeywordAndCantBeEnchantedByOtherAuras {
                    object: enchanted_object_from_pair(object_pair)?,
                    keyword: keyword_from_inner_pair(keyword_pair)?,
                },
            )
        }
        Rule::static_enchanted_can_attack_as_though => {
            let mut inner = pair.into_inner();
            let object_pair = inner
                .next()
                .expect("static_enchanted_can_attack begins with enchanted object");
            let keyword_pair = inner
                .next()
                .expect("static_enchanted_can_attack names ignored keyword");
            Ok(StaticAbility::EnchantedCanAttackAsThoughItDidntHave {
                object: enchanted_object_from_pair(object_pair)?,
                keyword: keyword_from_inner_pair(keyword_pair)?,
            })
        }
        Rule::static_you_control_enchanted => {
            let object_pair = pair
                .into_inner()
                .next()
                .expect("static_you_control_enchanted names enchanted object");
            Ok(StaticAbility::YouControlEnchanted {
                object: enchanted_object_from_pair(object_pair)?,
            })
        }
        Rule::static_you_may_have_source_enter_as_copy => {
            let mut inner = pair.into_inner();
            let source_pair = inner
                .next()
                .expect("copy replacement begins with source object");
            let permanent_type_pair = inner
                .next()
                .expect("copy replacement names copied permanent type");
            let exception = inner
                .next()
                .map(copy_exception_from_pair)
                .transpose()?;
            Ok(
                StaticAbility::YouMayHaveSourceEnterAsCopyOfAnyPermanentOnBattlefield {
                    source: source_object_from_pair(source_pair)?,
                    permanent_type: permanent_type_from_pair(permanent_type_pair)?,
                    exception,
                },
            )
        }
        Rule::static_effect_doesnt_remove_this_aura => {
            Ok(StaticAbility::EffectDoesntRemoveThisAura)
        }
        Rule::static_source_doesnt_untap_during_your_untap_step => {
            let source_pair = pair
                .into_inner()
                .next()
                .expect("static untap restriction begins with a source object");
            Ok(StaticAbility::SourceDoesntUntapDuringYourUntapStep {
                source: source_object_from_pair(source_pair)?,
            })
        }
        Rule::static_basic_lands_are_basic_lands => {
            let mut inner = pair.into_inner();
            let from_pair = inner
                .next()
                .expect("basic land type-changing effect names source land type");
            let to_pair = inner
                .next()
                .expect("basic land type-changing effect names destination land type");
            Ok(StaticAbility::BasicLandsAreBasicLands {
                from: basic_land_type_from_plural_pair(from_pair)?,
                to: basic_land_type_from_plural_pair(to_pair)?,
            })
        }
        Rule::static_that_permanent_is_basic_land_type_while_has_named_counter => {
            let mut inner = pair.into_inner();
            let permanent_type_pair = inner
                .next()
                .expect("linked land type effect names affected permanent type");
            let land_type_pair = inner
                .next()
                .expect("linked land type effect names basic land type");
            let counter_pair = inner
                .next()
                .expect("linked land type effect names counter");
            Ok(StaticAbility::ThatPermanentIsBasicLandTypeWhileHasNamedCounter {
                permanent_type: permanent_type_from_pair(permanent_type_pair)?,
                land_type: basic_land_type_from_pair(land_type_pair)?,
                counter_name: counter_name_from_counter_pair(counter_pair)?,
            })
        }
        Rule::target_creature_defending_player_controls_can_block_any_number => Ok(
            StaticAbility::TargetCreatureDefendingPlayerControlsCanBlockAnyNumberOfCreaturesThisTurn,
        ),
        Rule::it_blocks_each_attacking_creature_if_able => {
            Ok(StaticAbility::ItBlocksEachAttackingCreatureThisTurnIfAble)
        }
        Rule::this_turn_defending_players_make_random_blocking_piles => {
            Ok(StaticAbility::ThisTurnDefendingPlayersMakeRandomBlockingPiles)
        }
        Rule::additional_blockers_may_be_put_into_additional_piles => {
            Ok(StaticAbility::AdditionalBlockersMayBePutIntoAdditionalPiles)
        }
        Rule::assign_each_pile_to_attacking_creature_at_random => {
            Ok(StaticAbility::AssignEachPileToAttackingCreatureAtRandom)
        }
        Rule::creatures_in_assigned_pile_block_if_able => {
            Ok(StaticAbility::CreaturesInAssignedPileBlockIfAble)
        }
        _ => Err(ParseError::Internal("static_ability variant")),
    }
}

fn copy_exception_from_pair(pair: Pair<Rule>) -> Result<CopyException, ParseError> {
    if pair.as_rule() != Rule::copy_exception {
        return Err(ParseError::Internal("copy_exception"));
    }

    let permanent_type_pair = pair
        .into_inner()
        .next()
        .expect("copy exception names added permanent type");
    Ok(CopyException::PermanentTypeInAdditionToItsOtherTypes {
        permanent_type: permanent_type_from_pair(permanent_type_pair)?,
    })
}

fn activated_ability_from_pair(pair: Pair<Rule>) -> Result<ActivatedAbility, ParseError> {
    let mut inner = pair.into_inner();
    let cost_pair = inner
        .next()
        .ok_or(ParseError::Internal("activated_ability missing cost"))?;
    let effect_pair = inner
        .next()
        .ok_or(ParseError::Internal("activated_ability missing effect"))?;
    Ok(ActivatedAbility {
        costs: activated_cost_from_pair(cost_pair)?,
        effect: activated_effect_from_pair(effect_pair)?,
    })
}

fn activated_cost_from_pair(pair: Pair<Rule>) -> Result<Vec<ActivatedCost>, ParseError> {
    if pair.as_rule() != Rule::activated_cost {
        return Err(ParseError::Internal("activated_cost"));
    }
    let costs = pair
        .into_inner()
        .map(|child| match child.as_rule() {
            Rule::mana_cost => Ok(ActivatedCost::Mana(mana_cost_from_pair(child))),
            Rule::mana_symbol => Ok(ActivatedCost::Mana(ManaCost {
                symbols: vec![mana_symbol_from_pair(child)],
            })),
            Rule::variable_mana_symbol => Ok(ActivatedCost::VariableMana(
                variable_from_mana_symbol_pair(child)?,
            )),
            Rule::tap_symbol => Ok(ActivatedCost::Tap),
            Rule::sacrifice_source => {
                let source_pair = child.into_inner().next().ok_or(ParseError::Internal(
                    "sacrifice_source missing source_object",
                ))?;
                Ok(ActivatedCost::Sacrifice(source_object_from_pair(
                    source_pair,
                )?))
            }
            _ => Err(ParseError::Internal("activated_cost component")),
        })
        .collect::<Result<Vec<_>, _>>()?;
    if costs.is_empty() {
        return Err(ParseError::Internal("activated_cost empty"));
    }
    Ok(costs)
}

fn activated_effect_from_pair(pair: Pair<Rule>) -> Result<ActivatedEffect, ParseError> {
    match pair.as_rule() {
        Rule::add_mana => {
            let mana_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("add_mana missing mana_cost"))?;
            Ok(ActivatedEffect::AddMana(mana_cost_from_pair(mana_pair)))
        }
        Rule::add_one_mana_of_any_color => Ok(ActivatedEffect::AddOneManaOfAnyColor),
        Rule::add_mana_of_any_one_color => {
            let amount_pair = pair.into_inner().next().ok_or(ParseError::Internal(
                "add_mana_of_any_one_color missing number",
            ))?;
            let amount = number_word_to_u32(amount_pair.as_str())
                .ok_or(ParseError::Internal("number_word"))?;
            Ok(ActivatedEffect::AddManaOfAnyOneColor { amount })
        }
        Rule::untap_source => {
            let source_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("untap_source missing source_object"))?;
            Ok(ActivatedEffect::Untap(source_object_from_pair(
                source_pair,
            )?))
        }
        Rule::regenerate_source => {
            let source_pair = pair.into_inner().next().ok_or(ParseError::Internal(
                "regenerate_source missing source_object",
            ))?;
            Ok(ActivatedEffect::Regenerate(source_object_from_pair(
                source_pair,
            )?))
        }
        Rule::counter_target_colored_spell => {
            let color_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("counter colored spell missing color"))?;
            Ok(ActivatedEffect::CounterTargetColoredSpell {
                color: color_from_pair(color_pair)?,
            })
        }
        Rule::destroy_target_permanent => {
            let permanent_type_pair = pair.into_inner().next().ok_or(ParseError::Internal(
                "destroy target missing permanent_type",
            ))?;
            Ok(ActivatedEffect::DestroyTargetPermanent {
                permanent_type: permanent_type_from_pair(permanent_type_pair)?,
            })
        }
        Rule::destroy_target_creature_type => {
            let creature_type_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("destroy target missing creature_type"))?;
            Ok(ActivatedEffect::DestroyTargetCreatureType {
                creature_type: creature_type_from_pair(creature_type_pair)?,
            })
        }
        Rule::target_player_discards_cards => {
            let count_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("target player discards missing count"))?;
            Ok(ActivatedEffect::TargetPlayerDiscardsCards {
                count: discard_count_from_pair(count_pair)?,
            })
        }
        Rule::target_creature_with_power_or_less_cant_be_blocked => {
            let power_pair = pair.into_inner().next().ok_or(ParseError::Internal(
                "target creature unblockable missing power",
            ))?;
            let power = power_pair
                .as_str()
                .parse::<u32>()
                .map_err(|_| ParseError::Internal("target creature unblockable power"))?;
            Ok(ActivatedEffect::TargetCreatureWithPowerOrLessCantBeBlockedThisTurn { power })
        }
        Rule::activated_enchanted_gets_until_eot => {
            let mut inner = pair.into_inner();
            let pt_pair = inner.next().ok_or(ParseError::Internal(
                "activated enchanted gets missing permanent_type",
            ))?;
            let modifier_pair = inner.next().ok_or(ParseError::Internal(
                "activated enchanted gets missing modifier",
            ))?;
            Ok(ActivatedEffect::EnchantedGetsUntilEndOfTurn {
                permanent_type: permanent_type_from_pair(pt_pair)?,
                modifier: pt_modifier_from_pair(modifier_pair)?,
            })
        }
        Rule::activated_source_gets_until_eot => {
            let mut inner = pair.into_inner();
            let source_pair = inner
                .next()
                .ok_or(ParseError::Internal("activated source gets missing source"))?;
            let modifier_pair = inner.next().ok_or(ParseError::Internal(
                "activated source gets missing modifier",
            ))?;
            Ok(ActivatedEffect::SourceGetsUntilEndOfTurn {
                source: source_object_from_pair(source_pair)?,
                modifier: pt_modifier_from_pair(modifier_pair)?,
            })
        }
        Rule::prevent_next_damage_from_colored_source => {
            let color_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("prevent next damage missing color"))?;
            Ok(ActivatedEffect::PreventNextDamageFromColoredSource {
                color: color_from_pair(color_pair)?,
            })
        }
        Rule::prevent_next_damage_to_you_this_turn => {
            let amount_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("prevent next damage missing amount"))?;
            let amount = amount_pair
                .as_str()
                .parse::<u32>()
                .map_err(|_| ParseError::Internal("prevent next damage amount"))?;
            Ok(ActivatedEffect::PreventNextDamageToYouThisTurn { amount })
        }
        Rule::put_up_to_variable_pt_counters_on_source => {
            let mut inner = pair.into_inner();
            let amount_pair = inner
                .next()
                .ok_or(ParseError::Internal("put counters missing variable amount"))?;
            let counter_pair = inner
                .next()
                .ok_or(ParseError::Internal("put counters missing counter"))?;
            let source_pair = inner
                .next()
                .ok_or(ParseError::Internal("put counters missing source"))?;
            Ok(ActivatedEffect::PutUpToVariableCountersOnSource {
                amount: variable_from_str(amount_pair.as_str())?,
                counter: pt_modifier_from_counter_pair(counter_pair)?,
                source: source_object_from_pair(source_pair)?,
            })
        }
        Rule::put_named_counter_on_target_non_basic_land => {
            let mut inner = pair.into_inner();
            let counter_pair = inner
                .next()
                .ok_or(ParseError::Internal("put named counter missing counter"))?;
            let excluded_pair = inner.next().ok_or(ParseError::Internal(
                "put named counter missing excluded land type",
            ))?;
            let permanent_type_pair = inner.next().ok_or(ParseError::Internal(
                "put named counter missing target permanent type",
            ))?;
            if permanent_type_from_pair(permanent_type_pair)? != PermanentType::Land {
                return Err(ParseError::Internal("put named counter target type"));
            }
            let excluded_land_type_pair = excluded_pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("non basic land missing land type"))?;
            Ok(ActivatedEffect::PutNamedCounterOnTargetNonBasicLand {
                counter_name: counter_name_from_counter_pair(counter_pair)?,
                excluded_land_type: basic_land_type_from_pair(excluded_land_type_pair)?,
            })
        }
        Rule::if_source_on_battlefield_flip_onto_battlefield_from_height
        | Rule::if_source_turns_over_destroy_touched_nontoken_permanents
        | Rule::then_destroy_source => Ok(ActivatedEffect::PhysicalAction(
            physical_action_from_pair(pair)?,
        )),
        _ => Err(ParseError::Internal("activated_effect")),
    }
}

fn discard_count_from_pair(pair: Pair<Rule>) -> Result<CardCount, ParseError> {
    match pair.as_rule() {
        Rule::discard_one_card => Ok(CardCount::Number(1)),
        Rule::discard_counted_cards => {
            let count_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("discard counted cards missing count"))?;
            card_count_from_pair(count_pair)
        }
        _ => Err(ParseError::Internal("discard count")),
    }
}

fn physical_action_from_pair(pair: Pair<Rule>) -> Result<PhysicalAction, ParseError> {
    match pair.as_rule() {
        Rule::if_source_on_battlefield_flip_onto_battlefield_from_height => {
            let mut inner = pair.into_inner();
            let source_pair = inner
                .next()
                .ok_or(ParseError::Internal("physical flip missing source_object"))?;
            let height_pair = inner
                .next()
                .ok_or(ParseError::Internal("physical flip missing minimum height"))?;
            let minimum_height_feet = number_word_to_u32(height_pair.as_str())
                .ok_or(ParseError::Internal("physical flip height"))?;
            Ok(
                PhysicalAction::IfSourceOnBattlefieldFlipOntoBattlefieldFromHeight {
                    source: source_object_from_pair(source_pair)?,
                    minimum_height_feet,
                },
            )
        }
        Rule::if_source_turns_over_destroy_touched_nontoken_permanents => {
            let source_pair = pair.into_inner().next().ok_or(ParseError::Internal(
                "physical turns-over missing source_object",
            ))?;
            Ok(PhysicalAction::IfSourceTurnsOverCompletelyAtLeastOnceDuringFlipDestroyAllNontokenPermanentsItTouches {
                source: source_object_from_pair(source_pair)?,
            })
        }
        Rule::then_destroy_source => {
            let source_pair = pair.into_inner().next().ok_or(ParseError::Internal(
                "physical then-destroy missing source_object",
            ))?;
            Ok(PhysicalAction::ThenDestroySource {
                source: source_object_from_pair(source_pair)?,
            })
        }
        _ => Err(ParseError::Internal("physical_action")),
    }
}

fn enchanted_object_from_pair(pair: Pair<Rule>) -> Result<EnchantedObject, ParseError> {
    match pair.as_rule() {
        Rule::permanent_type => Ok(EnchantedObject::Permanent(permanent_type_from_pair(pair)?)),
        Rule::creature_type => Ok(EnchantedObject::CreatureType(creature_type_from_pair(
            pair,
        )?)),
        _ => Err(ParseError::Internal("enchanted_object")),
    }
}

fn mixed_pt_modifier_from_pair(pair: Pair<Rule>) -> Result<MixedPtModifier, ParseError> {
    if pair.as_rule() != Rule::mixed_pt_modifier {
        return Err(ParseError::Internal("mixed_pt_modifier"));
    }
    let mut inner = pair.into_inner();
    let power_pair = inner.next().expect("mixed_pt_modifier has power first");
    let toughness_pair = inner
        .next()
        .expect("mixed_pt_modifier has toughness second");
    Ok(MixedPtModifier {
        power: signed_pt_component_from_pair(power_pair)?,
        toughness: signed_pt_component_from_pair(toughness_pair)?,
    })
}

fn signed_pt_component_from_pair(pair: Pair<Rule>) -> Result<SignedPtComponent, ParseError> {
    match pair.as_rule() {
        Rule::signed_number => Ok(SignedPtComponent::Number(signed_number_from_pair(pair)?)),
        Rule::signed_variable => Ok(SignedPtComponent::Variable(signed_variable_from_pair(
            pair,
        )?)),
        _ => Err(ParseError::Internal("signed_pt_component")),
    }
}

fn pt_modifier_from_pair(pair: Pair<Rule>) -> Result<PtModifier, ParseError> {
    if pair.as_rule() != Rule::pt_modifier {
        return Err(ParseError::Internal("pt_modifier"));
    }
    let mut inner = pair.into_inner();
    let power_pair = inner.next().expect("pt_modifier has power first");
    let toughness_pair = inner.next().expect("pt_modifier has toughness second");
    Ok(PtModifier {
        power: signed_number_from_pair(power_pair)?,
        toughness: signed_number_from_pair(toughness_pair)?,
    })
}

fn pt_modifier_from_counter_pair(pair: Pair<Rule>) -> Result<PtModifier, ParseError> {
    match pair.as_rule() {
        Rule::pt_counter | Rule::pt_counters => {
            let modifier_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("pt counter missing modifier"))?;
            pt_modifier_from_pair(modifier_pair)
        }
        _ => Err(ParseError::Internal("pt counter")),
    }
}

fn signed_number_from_pair(pair: Pair<Rule>) -> Result<SignedNumber, ParseError> {
    if pair.as_rule() != Rule::signed_number {
        return Err(ParseError::Internal("signed_number"));
    }
    let s = pair.as_str();
    let (sign_char, rest) = s.split_at(1);
    let sign = match sign_char {
        "+" => Sign::Plus,
        "-" => Sign::Minus,
        _ => return Err(ParseError::Internal("signed_number sign")),
    };
    let magnitude = rest
        .parse::<u32>()
        .map_err(|_| ParseError::Internal("signed_number magnitude"))?;
    Ok(SignedNumber { sign, magnitude })
}

fn variable_pt_modifier_from_pair(pair: Pair<Rule>) -> Result<VariablePtModifier, ParseError> {
    if pair.as_rule() != Rule::variable_pt_modifier {
        return Err(ParseError::Internal("variable_pt_modifier"));
    }
    let mut inner = pair.into_inner();
    let power_pair = inner.next().expect("variable_pt_modifier has power first");
    let toughness_pair = inner
        .next()
        .expect("variable_pt_modifier has toughness second");
    Ok(VariablePtModifier {
        power: signed_variable_from_pair(power_pair)?,
        toughness: signed_variable_from_pair(toughness_pair)?,
    })
}

fn signed_variable_from_pair(pair: Pair<Rule>) -> Result<SignedVariable, ParseError> {
    if pair.as_rule() != Rule::signed_variable {
        return Err(ParseError::Internal("signed_variable"));
    }
    let s = pair.as_str();
    let (sign_char, rest) = s.split_at(1);
    let sign = match sign_char {
        "+" => Sign::Plus,
        "-" => Sign::Minus,
        _ => return Err(ParseError::Internal("signed_variable sign")),
    };
    Ok(SignedVariable {
        sign,
        variable: variable_from_str(rest)?,
    })
}

fn where_clause_from_pair(pair: Pair<Rule>) -> Result<Vec<VariableDefinition>, ParseError> {
    if pair.as_rule() != Rule::where_clause {
        return Err(ParseError::Internal("where_clause"));
    }
    pair.into_inner()
        .map(variable_definition_from_pair)
        .collect()
}

fn variable_definition_from_pair(pair: Pair<Rule>) -> Result<VariableDefinition, ParseError> {
    if pair.as_rule() != Rule::variable_definition {
        return Err(ParseError::Internal("variable_definition"));
    }
    let mut inner = pair.into_inner();
    let variable_pair = inner
        .next()
        .expect("variable_definition begins with a variable_name");
    let value_pair = inner
        .next()
        .expect("variable_definition has a value_expression");
    Ok(VariableDefinition {
        variable: variable_from_str(variable_pair.as_str())?,
        value: value_expression_from_pair(value_pair)?,
    })
}

fn value_expression_from_pair(pair: Pair<Rule>) -> Result<ValueExpression, ParseError> {
    match pair.as_rule() {
        Rule::half_number_of_basic_lands_you_control => {
            let mut inner = pair.into_inner();
            let land_type_pair = inner
                .next()
                .expect("half-number expression names a basic land type");
            let rounding_pair = inner
                .next()
                .expect("half-number expression names a rounding direction");
            Ok(ValueExpression::HalfNumberOfBasicLandsYouControl {
                basic_land_type: basic_land_type_from_plural_pair(land_type_pair)?,
                rounding: rounding_from_pair(rounding_pair)?,
            })
        }
        Rule::its_power => Ok(ValueExpression::ItsPower),
        Rule::number_of_cards_in_their_hand_minus => {
            let amount_pair = pair.into_inner().next().ok_or(ParseError::Internal(
                "number-of-cards expression missing subtraction amount",
            ))?;
            let amount = amount_pair
                .as_str()
                .parse::<u32>()
                .map_err(|_| ParseError::Internal("number-of-cards subtraction amount"))?;
            Ok(ValueExpression::NumberOfCardsInTheirHandMinus { amount })
        }
        _ => Err(ParseError::Internal("value_expression")),
    }
}

fn variable_from_str(s: &str) -> Result<Variable, ParseError> {
    match s {
        "X" => Ok(Variable::X),
        "Y" => Ok(Variable::Y),
        _ => Err(ParseError::Internal("variable_name")),
    }
}

fn basic_land_type_from_plural_pair(pair: Pair<Rule>) -> Result<BasicLandType, ParseError> {
    if pair.as_rule() != Rule::basic_land_type_plural {
        return Err(ParseError::Internal("basic_land_type_plural"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "plains" => Ok(BasicLandType::Plains),
        "islands" => Ok(BasicLandType::Island),
        "swamps" => Ok(BasicLandType::Swamp),
        "mountains" => Ok(BasicLandType::Mountain),
        "forests" => Ok(BasicLandType::Forest),
        _ => Err(ParseError::Internal("basic_land_type_plural variant")),
    }
}

fn basic_land_type_from_pair(pair: Pair<Rule>) -> Result<BasicLandType, ParseError> {
    if pair.as_rule() != Rule::basic_land_type {
        return Err(ParseError::Internal("basic_land_type"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "plains" => Ok(BasicLandType::Plains),
        "island" => Ok(BasicLandType::Island),
        "swamp" => Ok(BasicLandType::Swamp),
        "mountain" => Ok(BasicLandType::Mountain),
        "forest" => Ok(BasicLandType::Forest),
        _ => Err(ParseError::Internal("basic_land_type variant")),
    }
}

fn counter_name_from_counter_pair(pair: Pair<Rule>) -> Result<String, ParseError> {
    match pair.as_rule() {
        Rule::named_counter | Rule::named_counters => {
            let name_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("named counter missing name"))?;
            Ok(name_pair.as_str().to_ascii_lowercase())
        }
        _ => Err(ParseError::Internal("named counter")),
    }
}

fn rounding_from_pair(pair: Pair<Rule>) -> Result<Rounding, ParseError> {
    if pair.as_rule() != Rule::rounding {
        return Err(ParseError::Internal("rounding"));
    }
    let direction = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("rounding missing direction"))?;
    match direction.as_str().to_ascii_lowercase().as_str() {
        "down" => Ok(Rounding::Down),
        "up" => Ok(Rounding::Up),
        _ => Err(ParseError::Internal("rounding direction")),
    }
}

fn condition_from_pair(pair: Pair<Rule>) -> Result<Condition, ParseError> {
    match pair.as_rule() {
        Rule::enchanted_isnt => {
            let mut types = pair.into_inner();
            let pt = types
                .next()
                .expect("enchanted_isnt names the enchanted type first");
            let neg = types
                .next()
                .expect("enchanted_isnt names the negated type second");
            Ok(Condition::EnchantedIsNot {
                permanent_type: permanent_type_from_pair(pt)?,
                negated_type: permanent_type_from_pair(neg)?,
            })
        }
        _ => Err(ParseError::Internal("condition")),
    }
}

fn continuous_effect_from_pair(pair: Pair<Rule>) -> Result<ContinuousEffect, ParseError> {
    match pair.as_rule() {
        Rule::becomes_pt_from_mv => {
            let types = pair
                .into_inner()
                .map(permanent_type_from_pair)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(ContinuousEffect::BecomesWithPtFromManaValue { types })
        }
        _ => Err(ParseError::Internal("continuous_effect")),
    }
}

fn permanent_type_from_pair(pair: Pair<Rule>) -> Result<PermanentType, ParseError> {
    if pair.as_rule() != Rule::permanent_type {
        return Err(ParseError::Internal("permanent_type"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "artifact" => Ok(PermanentType::Artifact),
        "creature" => Ok(PermanentType::Creature),
        "enchantment" => Ok(PermanentType::Enchantment),
        "land" => Ok(PermanentType::Land),
        "planeswalker" => Ok(PermanentType::Planeswalker),
        _ => Err(ParseError::Internal("permanent_type variant")),
    }
}

fn permanent_type_from_plural_pair(pair: Pair<Rule>) -> Result<PermanentType, ParseError> {
    if pair.as_rule() != Rule::permanent_type_plural {
        return Err(ParseError::Internal("permanent_type_plural"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "artifacts" => Ok(PermanentType::Artifact),
        "creatures" => Ok(PermanentType::Creature),
        "enchantments" => Ok(PermanentType::Enchantment),
        "lands" => Ok(PermanentType::Land),
        "planeswalkers" => Ok(PermanentType::Planeswalker),
        _ => Err(ParseError::Internal("permanent_type_plural variant")),
    }
}

fn color_from_pair(pair: Pair<Rule>) -> Result<Color, ParseError> {
    if pair.as_rule() != Rule::color_word {
        return Err(ParseError::Internal("color_word"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "white" => Ok(Color::White),
        "blue" => Ok(Color::Blue),
        "black" => Ok(Color::Black),
        "red" => Ok(Color::Red),
        "green" => Ok(Color::Green),
        _ => Err(ParseError::Internal("color_word variant")),
    }
}

fn creature_status_from_pair(pair: Pair<Rule>) -> Result<CreatureStatus, ParseError> {
    if pair.as_rule() != Rule::creature_status {
        return Err(ParseError::Internal("creature_status"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "tapped" => Ok(CreatureStatus::Tapped),
        "untapped" => Ok(CreatureStatus::Untapped),
        _ => Err(ParseError::Internal("creature_status variant")),
    }
}

fn creature_type_from_pair(pair: Pair<Rule>) -> Result<CreatureType, ParseError> {
    if pair.as_rule() != Rule::creature_type {
        return Err(ParseError::Internal("creature_type"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "wall" => Ok(CreatureType::Wall),
        _ => Err(ParseError::Internal("creature_type variant")),
    }
}

fn mana_cost_from_pair(pair: Pair<Rule>) -> ManaCost {
    let symbols = pair.into_inner().map(mana_symbol_from_pair).collect();
    ManaCost { symbols }
}

fn mana_symbol_from_pair(pair: Pair<Rule>) -> ManaSymbol {
    let body = pair
        .into_inner()
        .next()
        .expect("mana_symbol always contains generic|color");
    match body.as_rule() {
        Rule::generic => ManaSymbol::Generic(
            body.as_str()
                .parse()
                .expect("generic is ASCII_DIGIT+, fits u32 in practice"),
        ),
        Rule::color => match body.as_str() {
            "W" => ManaSymbol::White,
            "U" => ManaSymbol::Blue,
            "B" => ManaSymbol::Black,
            "R" => ManaSymbol::Red,
            "G" => ManaSymbol::Green,
            "C" => ManaSymbol::Colorless,
            _ => unreachable!("color rule restricts to WUBRGC"),
        },
        _ => unreachable!("mana_body is silent and only contains generic|color"),
    }
}

fn variable_from_mana_symbol_pair(pair: Pair<Rule>) -> Result<Variable, ParseError> {
    if pair.as_rule() != Rule::variable_mana_symbol {
        return Err(ParseError::Internal("variable_mana_symbol"));
    }
    let s = pair.as_str();
    let variable = s
        .strip_prefix('{')
        .and_then(|s| s.strip_suffix('}'))
        .ok_or(ParseError::Internal("variable_mana_symbol braces"))?;
    variable_from_str(variable)
}
