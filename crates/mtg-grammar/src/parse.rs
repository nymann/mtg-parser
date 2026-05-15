use pest::iterators::{Pair, Pairs};
use pest::Parser;
use pest_derive::Parser;

use crate::ast::{
    ActionTiming, ActivatedAbility, ActivatedCost, ActivatedDamageEffect,
    ActivatedDamageEventEffect, ActivatedDamageRecipient, ActivatedDamageSource, ActivatedEffect,
    ActivationPermission, AsEntersChoice, BalanceSameWayAction, BasicLandType,
    BasicLandTypeReference, CardCount, CastRestriction, Color, ColoredTargetEffect, Condition,
    ContinuousEffect, CopyException, CounterUnlessCost, CreatureStatus, CreatureType, DamageAmount,
    DamageAssignment, DamageEvent, DamageEventPattern, DamageKind, DamageLifeGainCap,
    DamagePreventionAmount, DamagePreventionDuration, DamagePreventionEffect,
    DamagePreventionEvent, DamageRecipient, DamageRecipients, DamageRedirectionDestination,
    DestroyTarget, EachPlayerAction, EnchantObject, EnchantedObject, IfYouDoEffect,
    ImperativeAction, InterveningIf, Keyword, LandCountController, LifeLossAmount, LifeLossPlayer,
    ManaCost, ManaSymbol, MixedPtModifier, ModalMode, NamedDamageEvent, NamedKeywordAbility,
    NamedSourcePowerToughnessCount, ObjectStatus, OptionalCost, PayManaAmount, PayManaPlayer,
    PaymentFailureEffect, PermanentController, PermanentType, PhysicalAction, PreventionRecipient,
    PtModifier, Rounding, Sign, SignedNumber, SignedPtComponent, SignedVariable, SourceObject,
    SpellType, Statement, StaticAbility, Step, TapAllPermanentsActor,
    TargetPermanentEndOfTurnEffect, TriggerCondition, TriggerDamageCondition,
    TriggerDamageRecipient, TriggerDamageSource, TriggerEffect, TriggerEvent, TriggeredAbility,
    TriggeredDamage, ValueExpression, Variable, VariableDefinition, VariablePtModifier, Zone,
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

fn next_inner<'i>(
    inner: &mut Pairs<'i, Rule>,
    context: &'static str,
) -> Result<Pair<'i, Rule>, ParseError> {
    inner.next().ok_or(ParseError::Internal(context))
}

fn only_inner<'i>(
    pair: Pair<'i, Rule>,
    context: &'static str,
) -> Result<Pair<'i, Rule>, ParseError> {
    pair.into_inner()
        .next()
        .ok_or(ParseError::Internal(context))
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
        Rule::copy_target_spell_except_copy_is_color => {
            copy_target_spell_except_copy_is_color_from_pair(pair)
        }
        Rule::you_may_choose_new_targets_for_the_copy => {
            Ok(Statement::YouMayChooseNewTargetsForTheCopy)
        }
        Rule::label_phrase => Ok(Statement::Label {
            label: label_from_pair(pair)?,
        }),
        Rule::imperative_action_sequence => imperative_action_sequence_from_pair(pair),
        Rule::counter_target_spell => counter_target_spell_from_pair(pair),
        Rule::this_spell_costs_mana_more_to_cast_for_each_target_beyond_the_first => {
            this_spell_costs_mana_more_to_cast_for_each_target_beyond_the_first_from_pair(pair)
        }
        Rule::destroy => destroy_from_pair(pair),
        Rule::regenerate_target_creature => Ok(Statement::RegenerateTargetCreature),
        Rule::damage_event_statement => damage_event_statement_from_pair(pair),
        Rule::damage_prevention_effect => damage_prevention_effect_statement_from_pair(pair),
        Rule::spend_only_color_mana_on_variable => spend_only_color_mana_on_variable_from_pair(pair),
        Rule::as_source_enters_you_lose_life_equal_to_your_life_total => {
            as_source_enters_you_lose_life_equal_to_your_life_total_from_pair(pair)
        }
        Rule::you_gain_life_equal_damage_dealt_capped => {
            you_gain_life_equal_damage_dealt_capped_from_pair(pair)
        }
        Rule::if_you_cant_you_lose_the_game => Ok(Statement::IfYouCantYouLoseTheGame),
        Rule::if_you_cant_source_deals_damage_to_you => {
            if_you_cant_source_deals_damage_to_you_from_pair(pair)
        }
        Rule::if_its_permanent_cant_be_regenerated_and_would_die_exile_instead_this_turn => {
            if_its_permanent_cant_be_regenerated_and_would_die_exile_instead_this_turn_from_pair(
                pair,
            )
        }
        Rule::that_permanents_controller_may_attach_this_aura_to_permanent_of_their_choice => {
            that_permanents_controller_may_attach_this_aura_to_permanent_of_their_choice_from_pair(
                pair,
            )
        }
        Rule::tap_all_permanents_then_mana_loss => {
            tap_all_permanents_then_mana_loss_from_pair(pair)
        }
        Rule::player_payment_failure => player_payment_failure_from_pair(pair),
        Rule::target_player_activates_mana_ability_of_each_permanent_they_control => {
            target_player_activates_mana_ability_of_each_permanent_they_control_from_pair(pair)
        }
        Rule::then_that_player_loses_unspent_mana_and_you_add_mana_lost_this_way => {
            Ok(Statement::ThenThatPlayerLosesUnspentManaAndYouAddManaLostThisWay)
        }
        Rule::target_player_gains_life => target_player_gains_life_from_pair(pair),
        Rule::if_you_would_draw_card_during_your_draw_step_instead_you_may_skip_that_draw => {
            Ok(Statement::IfYouWouldDrawCardDuringYourDrawStepInsteadYouMaySkipThatDraw)
        }
        Rule::look_at_top_cards_of_target_players_library_then_put_them_back_in_any_order => {
            look_at_top_cards_of_target_players_library_then_put_them_back_in_any_order_from_pair(
                pair,
            )
        }
        Rule::you_may_have_that_player_shuffle => Ok(Statement::YouMayHaveThatPlayerShuffle),
        Rule::draw_cards => draw_cards_from_pair(pair),
        Rule::target_player_discards_cards_at_random => {
            target_player_discards_cards_at_random_from_pair(pair)
        }
        Rule::add_mana => add_mana_from_pair(pair),
        Rule::until_eot_you_may_pay_cost_at_timing => {
            until_eot_you_may_pay_cost_at_timing_from_pair(pair)
        }
        Rule::if_you_do_effect => if_you_do_effect_from_pair(pair),
        Rule::if_you_do_cast_that_card_face_down_without_paying_mana_cost => {
            if_you_do_cast_that_card_face_down_without_paying_mana_cost_from_pair(pair)
        }
        Rule::if_face_down_spell_creature_would_assign_or_deal_damage_or_tap_turn_face_up_instead => {
            Ok(Statement::IfFaceDownSpellCreatureWouldAssignOrDealDamageOrTapTurnFaceUpInstead)
        }
        Rule::if_you_do_until_your_next_turn_you_cant_be_attacked_except_by_creatures_with_keywords => {
            if_you_do_until_your_next_turn_you_cant_be_attacked_except_by_creatures_with_keywords_from_pair(pair)
        }
        Rule::change_text_of_target_spell_or_permanent_replacing_basic_land_type => {
            Ok(Statement::ChangeTextOfTargetSpellOrPermanentReplacingBasicLandType)
        }
        Rule::target_spell_or_permanent_becomes_color => {
            target_spell_or_permanent_becomes_color_from_pair(pair)
        }
        Rule::target_permanent_until_eot => target_permanent_until_eot_from_pair(pair),
        Rule::each_player_performs_action => each_player_performs_action_from_pair(pair),
        Rule::each_player_equalizes_controlled_permanents => {
            each_player_equalizes_controlled_permanents_from_pair(pair)
        }
        Rule::players_do_actions_the_same_way => players_do_actions_the_same_way_from_pair(pair),
        Rule::then_for_each_attacking_creature_choose_label_blocking_restriction => {
            then_for_each_attacking_creature_choose_label_blocking_restriction_from_pair(pair)
        }
        Rule::as_source_enters_choose => as_source_enters_choose_from_pair(pair),
        Rule::activated_ability_with_activation_permission => {
            activated_ability_with_activation_permission_from_pair(pair)
        }
        Rule::source_enters_with_pt_counters => source_enters_with_pt_counters_from_pair(pair),
        Rule::this_ability_cant_cause_total_pt_counters_greater_than => {
            this_ability_cant_cause_total_pt_counters_greater_than_from_pair(pair)
        }
        Rule::if_this_ability_activated_at_least_times_this_turn_sacrifice_source_at_next_end_step => {
            if_this_ability_activated_at_least_times_this_turn_sacrifice_source_at_next_end_step_from_pair(pair)
        }
        Rule::only_sources_owner_may_activate_this_ability => {
            only_sources_owner_may_activate_this_ability_from_pair(pair)
        }
        Rule::activate_only_during_your_upkeep => Ok(Statement::ActivateOnlyDuringYourUpkeep),
        Rule::activate_only_during_combat => Ok(Statement::ActivateOnlyDuringCombat),
        Rule::activate_only_during_your_turn_and_only_once_each_turn => {
            Ok(Statement::ActivateOnlyDuringYourTurnAndOnlyOnceEachTurn)
        }
        Rule::activate_only_during_your_turn => Ok(Statement::ActivateOnlyDuringYourTurn),
        Rule::activate_only_during_opponents_turn_before_attackers_declared => {
            Ok(Statement::ActivateOnlyDuringOpponentsTurnBeforeAttackersDeclared)
        }
        Rule::activate_only_as_sorcery => Ok(Statement::ActivateOnlyAsSorcery),
        Rule::destroy_it_at_beginning_of_next_end_step_if_it_didnt_attack_this_turn => {
            Ok(Statement::DestroyItAtBeginningOfNextEndStepIfItDidntAttackThisTurn)
        }
        Rule::keyword_ability => Ok(Statement::Keyword(keyword_from_pair(pair)?)),
        Rule::keyword_ability_list => keyword_list_from_pair(pair),
        Rule::semicolon_keyword_ability_list => semicolon_keyword_list_from_pair(pair),
        Rule::static_as_long_as
        | Rule::static_colored_spells_cost_mana_more_to_cast
        | Rule::static_activated_abilities_of_colored_permanents_cost_mana_more_to_activate
        | Rule::static_colored_permanents_get
        | Rule::static_other_creature_type_get_and_have_keyword
        | Rule::static_status_creatures_you_control_get
        | Rule::static_enchanted_gets_with_definitions
        | Rule::static_enchanted_gets
        | Rule::static_enchanted_has_triggered_ability
        | Rule::static_enchanted_has_keyword_and_cant_be_enchanted_by_other_auras
        | Rule::static_enchanted_has_keyword
        | Rule::static_enchanted_loses_keyword
        | Rule::static_enchanted_loses_keyword_fragment
        | Rule::static_enchanted_is_basic_land_type
        | Rule::static_enchanted_can_attack_as_though_it_had
        | Rule::static_enchanted_can_attack_as_though_it_didnt_have
        | Rule::static_enchanted_cant_be_blocked_except_by_creature_type
        | Rule::static_all_creatures_able_to_block_enchanted_do_so
        | Rule::static_you_control_enchanted
        | Rule::static_you_have_no_maximum_hand_size
        | Rule::you_dont_lose_game_for_having_zero_or_less_life
        | Rule::if_you_would_gain_life_draw_that_many_cards_instead
        | Rule::static_if_effect_causes_you_to_discard_card_you_may_put_it_on_top_of_library_instead
        | Rule::static_you_may_play_any_number_of_permanents_on_each_of_your_turns
        | Rule::static_you_may_have_source_enter_as_copy
        | Rule::static_source_enters_tapped
        | Rule::static_source_attacks_each_combat_if_able
        | Rule::static_source_cant_attack_unless_defending_player_controls_basic_land
        | Rule::static_source_cant_be_blocked_by_creature_type
        | Rule::static_source_doesnt_untap_during_your_untap_step
        | Rule::static_creatures_with_power_or_greater_dont_untap_during_their_controllers_untap_steps
        | Rule::static_source_cant_block_creatures_with_power_or_greater
        | Rule::static_named_source_pt_equal_to_count
        | Rule::static_basic_lands_are_basic_lands
        | Rule::static_basic_lands_are_pt_colored_creatures_still_lands
        | Rule::static_that_permanent_is_basic_land_type_while_has_named_counter
        | Rule::remove_target_creature_defending_player_controls_from_combat
        | Rule::creatures_it_was_blocking_become_unblocked
        | Rule::you_may_have_it_block_attacking_creature
        | Rule::that_creature_attacks_this_turn_if_able
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
        Rule::triggered_ability | Rule::triggered_ability_fragment => Ok(
            Statement::TriggeredAbility(triggered_ability_from_pair(pair)?),
        ),
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
        Rule::colored_target_effect => match colored_target_effect_from_pair(effect)? {
            ColoredTargetEffect::CounterSpell { color } => {
                Ok(ModalMode::CounterTargetColoredSpell { color })
            }
            ColoredTargetEffect::DestroyPermanent { color } => {
                Ok(ModalMode::DestroyTargetColoredPermanent { color })
            }
        },
        Rule::target_player_gains_life => Ok(ModalMode::TargetPlayerGainsLife {
            amount: target_player_gains_life_amount_from_pair(effect)?,
        }),
        Rule::damage_prevention_effect_this_turn => {
            let effect = damage_prevention_effect_from_this_turn_pair(effect)?;
            Ok(ModalMode::PreventDamageThisTurn { effect })
        }
        _ => Err(ParseError::Internal("modal_effect")),
    }
}

fn colored_target_effect_from_pair(pair: Pair<Rule>) -> Result<ColoredTargetEffect, ParseError> {
    let action_pair = only_inner(pair, "colored_target_effect missing action")?;
    colored_target_action_from_pair(action_pair)
}

fn colored_target_action_from_pair(pair: Pair<Rule>) -> Result<ColoredTargetEffect, ParseError> {
    match pair.as_rule() {
        Rule::counter_target_colored_spell_action => {
            let color_pair = only_inner(pair, "counter colored spell missing color")?;
            let color = color_from_pair(color_pair)?;
            Ok(ColoredTargetEffect::CounterSpell { color })
        }
        Rule::destroy_target_colored_permanent_action => {
            let target_pair = only_inner(pair, "destroy colored permanent missing target")?;
            let color_pair = only_inner(target_pair, "target colored permanent missing color")?;
            let color = color_from_pair(color_pair)?;
            Ok(ColoredTargetEffect::DestroyPermanent { color })
        }
        _ => Err(ParseError::Internal("colored_target_action")),
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
        Rule::during_step => {
            let step_pair = timing
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("during_step missing step"))?;
            CastRestriction::DuringStep {
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
        "declare blockers" => Ok(Step::DeclareBlockers),
        _ => Err(ParseError::Internal("step variant")),
    }
}

fn you_own_target_card_in_zone_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let zone_pair = only_inner(pair, "you_own_target_card_in_zone missing zone")?;
    Ok(Statement::YouOwnTargetCardInZone {
        zone: zone_from_pair(zone_pair)?,
    })
}

fn copy_target_spell_except_copy_is_color_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let spell_types_pair = inner.next().ok_or(ParseError::Internal(
        "copy target spell missing spell_type_choice",
    ))?;
    let color_pair = inner
        .next()
        .ok_or(ParseError::Internal("copy target spell missing color"))?;
    let spell_types = spell_types_pair
        .into_inner()
        .map(spell_type_from_pair)
        .collect::<Result<Vec<_>, _>>()?;
    if spell_types.is_empty() {
        return Err(ParseError::Internal("spell_type_choice empty"));
    }
    Ok(Statement::CopyTargetSpellExceptCopyIsColor {
        spell_types,
        color: color_from_pair(color_pair)?,
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
    let action_pair = only_inner(pair, "each_player_performs_action missing action")?;
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

fn target_permanent_until_eot_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let permanent_type_pair = inner.next().ok_or(ParseError::Internal(
        "target permanent until end of turn missing permanent_type",
    ))?;
    let effect_pair = inner.next().ok_or(ParseError::Internal(
        "target permanent until end of turn missing effect",
    ))?;
    Ok(Statement::target_permanent_until_end_of_turn(
        permanent_type_from_pair(permanent_type_pair)?,
        target_permanent_eot_effect_from_pair(effect_pair)?,
    ))
}

fn target_permanent_eot_effect_from_pair(
    pair: Pair<Rule>,
) -> Result<TargetPermanentEndOfTurnEffect, ParseError> {
    match pair.as_rule() {
        Rule::target_permanent_gets_eot_effect => {
            let modifier_pair = only_inner(pair, "target gets missing modifier")?;
            Ok(TargetPermanentEndOfTurnEffect::Gets(
                mixed_pt_modifier_from_pair(modifier_pair)?,
            ))
        }
        Rule::target_permanent_gains_keyword_eot_effect => {
            let keyword_pair = only_inner(pair, "target gains missing keyword")?;
            Ok(TargetPermanentEndOfTurnEffect::GainsKeyword(
                keyword_from_inner_pair(keyword_pair)?,
            ))
        }
        Rule::target_permanent_gains_keyword_and_gets_eot_effect => {
            let mut inner = pair.into_inner();
            let keyword_pair = inner.next().ok_or(ParseError::Internal(
                "target gains and gets missing keyword",
            ))?;
            let modifier_pair = inner.next().ok_or(ParseError::Internal(
                "target gains and gets missing modifier",
            ))?;
            let where_pair = inner.next().ok_or(ParseError::Internal(
                "target gains and gets missing where_clause",
            ))?;
            Ok(TargetPermanentEndOfTurnEffect::GainsKeywordAndGets {
                keyword: keyword_from_inner_pair(keyword_pair)?,
                modifier: mixed_pt_modifier_from_pair(modifier_pair)?,
                definitions: where_clause_from_pair(where_pair)?,
            })
        }
        _ => Err(ParseError::Internal("target permanent end of turn effect")),
    }
}

fn target_permanent_gains_keyword_until_eot_from_pair(
    pair: Pair<Rule>,
) -> Result<ActivatedEffect, ParseError> {
    let mut inner = pair.into_inner();
    let permanent_type_pair = inner.next().ok_or(ParseError::Internal(
        "target permanent gains keyword missing permanent_type",
    ))?;
    let effect_pair = inner.next().ok_or(ParseError::Internal(
        "target permanent gains keyword missing effect",
    ))?;
    let keyword_pair = only_inner(
        effect_pair,
        "target permanent gains keyword missing keyword",
    )?;
    Ok(ActivatedEffect::TargetPermanentGainsKeywordUntilEndOfTurn {
        permanent_type: permanent_type_from_pair(permanent_type_pair)?,
        keyword: keyword_from_inner_pair(keyword_pair)?,
    })
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

fn as_source_enters_choose_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = next_inner(&mut inner, "as enters choose missing source")?;
    let choice_pair = next_inner(&mut inner, "as enters choose missing choice")?;
    Ok(Statement::AsSourceEntersChoose {
        source: source_object_from_pair(source_pair)?,
        choice: as_enters_choice_from_pair(choice_pair)?,
    })
}

fn as_enters_choice_from_pair(pair: Pair<Rule>) -> Result<AsEntersChoice, ParseError> {
    match pair.as_rule() {
        Rule::opponent_choice => Ok(AsEntersChoice::Opponent),
        Rule::basic_land_type_choice => Ok(AsEntersChoice::BasicLandType),
        _ => Err(ParseError::Internal("as_enters_choice")),
    }
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
            let pt = only_inner(pair, "sacrifice action missing permanent_type_plural")?;
            Ok(BalanceSameWayAction::SacrificePermanents {
                permanent_type: permanent_type_from_plural_pair(pt)?,
            })
        }
        _ => Err(ParseError::Internal("same-way action")),
    }
}

fn permanent_type_choice_from_pair(pair: Pair<Rule>) -> Result<Vec<PermanentType>, ParseError> {
    pair.into_inner().map(permanent_type_from_pair).collect()
}

fn target_permanent_choice_from_pair(pair: Pair<Rule>) -> Result<Vec<PermanentType>, ParseError> {
    let choice_pair = only_inner(
        pair,
        "target_permanent_choice missing permanent_type_choice",
    )?;
    permanent_type_choice_from_pair(choice_pair)
}

fn destroy_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let target_pair = only_inner(pair, "destroy missing target")?;
    Ok(Statement::destroy(destroy_target_from_pair(target_pair)?))
}

fn destroy_target_from_pair(pair: Pair<Rule>) -> Result<DestroyTarget, ParseError> {
    match pair.as_rule() {
        Rule::destroy_target => {
            let target_pair = only_inner(pair, "destroy_target missing target")?;
            destroy_target_from_pair(target_pair)
        }
        Rule::target_permanent_choice => Ok(DestroyTarget::TargetPermanents(
            target_permanent_choice_from_pair(pair)?,
        )),
        Rule::target_colored_permanent => {
            let color_pair = only_inner(pair, "target_colored_permanent missing color")?;
            Ok(DestroyTarget::TargetColoredPermanent(color_from_pair(
                color_pair,
            )?))
        }
        Rule::target_creature_type => {
            let creature_type_pair =
                only_inner(pair, "target_creature_type missing creature_type")?;
            Ok(DestroyTarget::TargetCreatureType(creature_type_from_pair(
                creature_type_pair,
            )?))
        }
        Rule::destroy_all => {
            let target_pair = only_inner(pair, "destroy_all missing target")?;
            destroy_target_from_pair(target_pair)
        }
        Rule::destroy_all_objects => {
            let target_pair = only_inner(pair, "destroy_all_objects missing target")?;
            destroy_target_from_pair(target_pair)
        }
        Rule::permanent_type_plural_list => Ok(DestroyTarget::AllPermanents(
            permanent_type_plural_list_from_pair(pair)?,
        )),
        Rule::basic_land_type_plural => Ok(DestroyTarget::AllBasicLands(
            basic_land_type_from_plural_pair(pair)?,
        )),
        _ => Err(ParseError::Internal("destroy target")),
    }
}

fn that_permanents_controller_may_attach_this_aura_to_permanent_of_their_choice_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let controller_pair = next_inner(
        &mut inner,
        "controller attach effect missing controlled permanent_type",
    )?;
    let controller_type_pair = only_inner(
        controller_pair,
        "controller attach effect missing controller permanent_type",
    )?;
    let attach_to_pair = next_inner(
        &mut inner,
        "controller attach effect missing destination permanent_type",
    )?;
    Ok(
        Statement::ThatPermanentsControllerMayAttachThisAuraToPermanentOfTheirChoice {
            controller_of: permanent_type_from_pair(controller_type_pair)?,
            attach_to: permanent_type_from_pair(attach_to_pair)?,
        },
    )
}

fn this_spell_costs_mana_more_to_cast_for_each_target_beyond_the_first_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mana_pair = only_inner(pair, "additional target cost missing mana")?;
    Ok(
        Statement::ThisSpellCostsManaMoreToCastForEachTargetBeyondTheFirst {
            mana: mana_cost_from_pair(mana_pair),
        },
    )
}

fn damage_event_statement_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    Ok(Statement::NamedSourceDealsDamage {
        event: named_damage_event_from_pair(pair)?,
    })
}

fn only_sources_owner_may_activate_this_ability_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let source_pair = only_inner(pair, "owner activation restriction missing source")?;
    Ok(Statement::OnlySourcesOwnerMayActivateThisAbility {
        source: source_object_from_possessive_pair(source_pair)?,
    })
}

fn named_damage_event_from_pair(pair: Pair<Rule>) -> Result<NamedDamageEvent, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = next_inner(&mut inner, "damage event missing source name")?;
    let next_pair = next_inner(&mut inner, "damage event missing payload")?;
    if next_pair.as_rule() == Rule::damage_event_assignment_list {
        let assignments = damage_event_assignments_from_pair(next_pair)?;
        let amount = assignments
            .first()
            .ok_or(ParseError::Internal("damage event missing assignment"))?
            .amount;
        return Ok(DamageEvent {
            source: source_pair.as_str().to_string(),
            amount,
            recipient: DamageRecipients::Assignments(assignments),
        });
    }
    let recipients_pair = next_inner(&mut inner, "damage event missing recipients")?;
    Ok(DamageEvent {
        source: source_pair.as_str().to_string(),
        amount: damage_event_amount_from_pair(next_pair)?,
        recipient: damage_event_recipients_from_pair(recipients_pair)?,
    })
}

fn damage_event_assignments_from_pair(
    pair: Pair<Rule>,
) -> Result<Vec<DamageAssignment<DamageRecipient>>, ParseError> {
    pair.into_inner()
        .map(|assignment_pair| {
            let mut inner = assignment_pair.into_inner();
            let amount_pair = next_inner(&mut inner, "damage event assignment missing amount")?;
            let recipient_pair =
                next_inner(&mut inner, "damage event assignment missing recipient")?;
            Ok(DamageAssignment {
                amount: damage_event_amount_from_pair(amount_pair)?,
                recipient: damage_recipient_from_pair(recipient_pair)?,
            })
        })
        .collect()
}

fn damage_event_amount_from_pair(pair: Pair<Rule>) -> Result<DamageAmount, ParseError> {
    let inner = only_inner(pair, "damage event amount missing inner rule")?;
    damage_amount_from_pair(inner)
}

fn damage_event_recipients_from_pair(pair: Pair<Rule>) -> Result<DamageRecipients, ParseError> {
    let inner = only_inner(pair, "damage event recipients missing inner rule")?;
    match inner.as_rule() {
        Rule::damage_event_any_target => Ok(DamageRecipients::AnyTarget),
        Rule::damage_event_divided_evenly_rounded_down_among_any_number_of_targets => {
            Ok(DamageRecipients::DividedEvenlyRoundedDownAmongAnyNumberOfTargets)
        }
        Rule::damage_event_recipient_list => {
            let recipients = inner
                .into_inner()
                .map(damage_recipient_from_pair)
                .collect::<Result<Vec<_>, _>>()?;
            Ok(DamageRecipients::List(recipients))
        }
        _ => Err(ParseError::Internal("damage event recipients")),
    }
}

fn damage_recipient_from_pair(pair: Pair<Rule>) -> Result<DamageRecipient, ParseError> {
    let inner = if pair.as_rule() == Rule::damage_recipient {
        only_inner(pair, "damage recipient missing inner rule")?
    } else {
        pair
    };
    match inner.as_rule() {
        Rule::any_target_prevention_recipient => Ok(DamageRecipient::AnyTarget),
        Rule::you_damage_recipient => Ok(DamageRecipient::You),
        Rule::each_creature_damage_recipient => Ok(DamageRecipient::EachCreature),
        Rule::each_creature_with_keyword => {
            let keyword_pair = only_inner(inner, "creature damage recipient missing keyword")?;
            Ok(DamageRecipient::EachCreatureWithKeyword {
                keyword: keyword_from_inner_pair(keyword_pair)?,
            })
        }
        Rule::each_creature_without_keyword => {
            let keyword_pair = only_inner(inner, "creature damage recipient missing keyword")?;
            Ok(DamageRecipient::EachCreatureWithoutKeyword {
                keyword: keyword_from_inner_pair(keyword_pair)?,
            })
        }
        Rule::each_player_damage_recipient => Ok(DamageRecipient::EachPlayer),
        Rule::that_player_damage_recipient => Ok(DamageRecipient::ThatPlayer),
        _ => Err(ParseError::Internal("damage recipient")),
    }
}

fn spend_only_color_mana_on_variable_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let color_pair = next_inner(&mut inner, "spend-only restriction missing color")?;
    let variable_pair = next_inner(&mut inner, "spend-only restriction missing variable")?;
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

fn as_source_enters_you_lose_life_equal_to_your_life_total_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let source_pair = only_inner(pair, "as_source_enters_you_lose_life missing source")?;
    Ok(Statement::AsSourceEntersYouLoseLifeEqualToYourLifeTotal {
        source: source_object_from_pair(source_pair)?,
    })
}

fn if_its_permanent_cant_be_regenerated_and_would_die_exile_instead_this_turn_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let permanent_type_pair = only_inner(pair, "conditional exile missing permanent type")?;
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

fn target_player_discards_cards_at_random_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let count_pair = only_inner(pair, "target player discards at random missing count")?;
    Ok(Statement::TargetPlayerDiscardsCardsAtRandom {
        count: discard_count_from_pair(count_pair)?,
    })
}

fn target_player_gains_life_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    Ok(Statement::TargetPlayerGainsLife {
        amount: target_player_gains_life_amount_from_pair(pair)?,
    })
}

fn target_player_gains_life_amount_from_pair(pair: Pair<Rule>) -> Result<u32, ParseError> {
    let amount_pair = only_inner(pair, "target_player_gains_life missing amount")?;
    amount_pair
        .as_str()
        .parse::<u32>()
        .map_err(|_| ParseError::Internal("target_player_gains_life amount"))
}

fn counter_target_spell_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let unless_cost = pair
        .into_inner()
        .next()
        .map(counter_unless_cost_from_pair)
        .transpose()?;
    Ok(Statement::CounterTargetSpell { unless_cost })
}

fn counter_unless_cost_from_pair(pair: Pair<Rule>) -> Result<CounterUnlessCost, ParseError> {
    let cost_pair = only_inner(pair, "counter unless missing mana cost")?;
    Ok(CounterUnlessCost::ItsControllerPays(mana_cost_from_pair(
        cost_pair,
    )))
}

fn tap_all_permanents_then_mana_loss_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let permanent_type_pair = only_inner(
        pair,
        "tap all permanents target player controls missing permanent_type_plural",
    )?;
    Ok(Statement::TapAllPermanentsAndPlayerLosesUnspentMana {
        actor: TapAllPermanentsActor::TargetPlayer,
        permanent_type: permanent_type_from_plural_pair(permanent_type_pair)?,
        with_mana_abilities: false,
    })
}

fn player_payment_failure_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let effect_pair = only_inner(pair, "player payment failure missing effect")?;
    Ok(Statement::PlayerPaymentFailure {
        effect: payment_failure_effect_from_pair(effect_pair)?,
    })
}

fn payment_failure_effect_from_pair(pair: Pair<Rule>) -> Result<PaymentFailureEffect, ParseError> {
    match pair.as_rule() {
        Rule::payment_failure_tap_mana_sources => {
            let mut permanent_type = None;
            let mut with_mana_abilities = false;
            for child in pair.into_inner() {
                match child.as_rule() {
                    Rule::permanent_type_plural => {
                        permanent_type = Some(permanent_type_from_plural_pair(child)?);
                    }
                    Rule::with_mana_abilities => with_mana_abilities = true,
                    _ => {}
                }
            }
            Ok(PaymentFailureEffect::TapAllPermanentsAndLoseUnspentMana {
                permanent_type: permanent_type.ok_or(ParseError::Internal(
                    "payment failure tap missing permanent type",
                ))?,
                with_mana_abilities,
            })
        }
        _ => Err(ParseError::Internal("payment failure effect")),
    }
}

fn target_player_activates_mana_ability_of_each_permanent_they_control_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let permanent_type_pair = only_inner(
        pair,
        "target player activates mana ability missing permanent_type",
    )?;
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

fn look_at_top_cards_of_target_players_library_then_put_them_back_in_any_order_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let count_pair = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("look at top cards missing card count"))?;
    Ok(
        Statement::LookAtTopCardsOfTargetPlayersLibraryThenPutThemBackInAnyOrder {
            count: card_count_from_pair(count_pair)?,
        },
    )
}

fn until_eot_you_may_pay_cost_at_timing_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mut timing = None;
    let mut cost = None;
    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::any_time_you_could_activate_a_mana_ability
            | Rule::any_time_you_could_cast_an_instant => {
                timing = Some(action_timing_from_pair(child)?);
            }
            Rule::pay_life_cost | Rule::pay_mana_cost => {
                cost = Some(optional_cost_from_pair(child)?);
            }
            _ => return Err(ParseError::Internal("until_eot child")),
        }
    }
    Ok(Statement::UntilEndOfTurnYouMayPayCostAtTiming {
        timing: timing.ok_or(ParseError::Internal("until_eot missing action timing"))?,
        cost: cost.ok_or(ParseError::Internal("until_eot missing optional cost"))?,
    })
}

fn damage_prevention_effect_statement_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let inner = only_inner(pair, "damage prevention effect missing inner rule")?;
    match inner.as_rule() {
        Rule::damage_prevention_effect_sentence => {
            let (effect, definitions) = damage_prevention_effect_sentence_from_pair(inner)?;
            Ok(Statement::PreventDamageThisTurn {
                effect,
                definitions,
            })
        }
        Rule::damage_prevention_effect_this_turn => {
            let effect = damage_prevention_effect_from_this_turn_pair(inner)?;
            Ok(Statement::damage_prevention_effect(effect))
        }
        _ => Err(ParseError::Internal("damage prevention effect")),
    }
}

fn damage_prevention_effect_sentence_from_pair(
    pair: Pair<Rule>,
) -> Result<
    (
        DamagePreventionEffect<PreventionRecipient>,
        Vec<VariableDefinition>,
    ),
    ParseError,
> {
    let mut effect = damage_prevention_effect_from_this_turn_pair_with_recipient(
        pair.clone(),
        Rule::damage_prevention_recipient_clause,
        |recipient_pair| {
            prevention_recipient_from_pair(only_inner(
                recipient_pair,
                "damage prevention missing recipient",
            )?)
        },
        "damage prevention child",
    )?;
    let mut definitions = Vec::new();
    let mut has_duration = false;
    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::damage_prevention_duration_fragment => has_duration = true,
            Rule::damage_prevention_variable_definition => {
                let where_pair = only_inner(child, "damage prevention missing where clause")?;
                definitions = where_clause_from_pair(where_pair)?;
            }
            _ => {}
        }
    }
    effect.duration = has_duration.then_some(DamagePreventionDuration::ThisTurn);
    Ok((effect, definitions))
}

fn if_you_do_cast_that_card_face_down_without_paying_mana_cost_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let power_pair = inner
        .next()
        .ok_or(ParseError::Internal("face-down cast missing power"))?;
    let toughness_pair = inner
        .next()
        .ok_or(ParseError::Internal("face-down cast missing toughness"))?;
    Ok(
        Statement::IfYouDoCastThatCardFaceDownWithoutPayingManaCost {
            power: power_pair
                .as_str()
                .parse::<u32>()
                .map_err(|_| ParseError::Internal("face-down cast power"))?,
            toughness: toughness_pair
                .as_str()
                .parse::<u32>()
                .map_err(|_| ParseError::Internal("face-down cast toughness"))?,
        },
    )
}

fn damage_prevention_effect_from_this_turn_pair(
    pair: Pair<Rule>,
) -> Result<DamagePreventionEffect<PreventionRecipient>, ParseError> {
    damage_prevention_effect_from_this_turn_pair_with_recipient(
        pair,
        Rule::damage_prevention_recipient_clause,
        |recipient_pair| {
            prevention_recipient_from_pair(only_inner(
                recipient_pair,
                "damage prevention missing recipient",
            )?)
        },
        "damage prevention child",
    )
}

fn damage_prevention_effect_from_this_turn_pair_with_recipient<R>(
    pair: Pair<Rule>,
    recipient_rule: Rule,
    mut recipient_from_pair: impl FnMut(Pair<Rule>) -> Result<R, ParseError>,
    unexpected_child_context: &'static str,
) -> Result<DamagePreventionEffect<R>, ParseError> {
    let mut amount = None;
    let mut event = None;
    let mut kind = None;
    let mut recipient = None;
    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::damage_prevention_amount_axis => {
                amount = Some(damage_prevention_amount_axis_from_pair(child)?);
            }
            Rule::damage_prevention_event => {
                let (parsed_event, parsed_kind) = damage_prevention_event_from_pair(child)?;
                event = Some(parsed_event);
                kind = parsed_kind;
            }
            Rule::damage_kind => {
                kind = Some(damage_kind_from_pair(child)?);
            }
            rule if rule == recipient_rule => {
                recipient = Some(recipient_from_pair(child)?);
            }
            Rule::damage_prevention_duration_fragment
            | Rule::damage_prevention_variable_definition => {}
            _ => return Err(ParseError::Internal(unexpected_child_context)),
        }
    }
    let mut effect = DamagePreventionEffect::this_turn(
        amount.ok_or(ParseError::Internal("damage prevention missing amount"))?,
        kind,
        recipient,
    );
    effect.event = event.ok_or(ParseError::Internal("damage prevention missing event"))?;
    Ok(effect)
}

fn damage_prevention_amount_axis_from_pair(
    pair: Pair<Rule>,
) -> Result<DamagePreventionAmount, ParseError> {
    let inner = only_inner(pair, "damage prevention amount missing inner rule")?;
    match inner.as_rule() {
        Rule::damage_prevention_all_amount => Ok(DamagePreventionAmount::All),
        Rule::damage_prevention_next_amount => Ok(DamagePreventionAmount::Next(
            damage_prevention_next_amount_from_pair(inner)?,
        )),
        Rule::damage_prevention_of_amount => Ok(DamagePreventionAmount::Amount(
            damage_prevention_next_amount_from_pair(inner)?,
        )),
        _ => Err(ParseError::Internal("damage prevention amount")),
    }
}

fn damage_prevention_next_amount_from_pair(pair: Pair<Rule>) -> Result<DamageAmount, ParseError> {
    let amount_pair = only_inner(pair, "damage prevention next amount missing amount")?;
    damage_amount_from_pair(amount_pair)
}

fn damage_prevention_event_from_pair(
    pair: Pair<Rule>,
) -> Result<(DamagePreventionEvent, Option<DamageKind>), ParseError> {
    let inner = only_inner(pair, "damage prevention event missing inner rule")?;
    match inner.as_rule() {
        Rule::damage_prevention_that_would_be_dealt => {
            let kind = inner
                .into_inner()
                .next()
                .map(damage_kind_from_pair)
                .transpose()?;
            Ok((DamagePreventionEvent::ThatWouldBeDealt, kind))
        }
        Rule::damage_prevention_of_that_damage => Ok((DamagePreventionEvent::OfThatDamage, None)),
        _ => Err(ParseError::Internal("damage prevention event")),
    }
}

fn damage_kind_from_pair(pair: Pair<Rule>) -> Result<DamageKind, ParseError> {
    match pair.as_rule() {
        Rule::damage_kind => match pair.as_str().to_ascii_lowercase().as_str() {
            "combat" => Ok(DamageKind::CombatDamage),
            _ => Err(ParseError::Internal("damage kind")),
        },
        _ => Err(ParseError::Internal("damage kind")),
    }
}

fn damage_amount_from_pair(pair: Pair<Rule>) -> Result<DamageAmount, ParseError> {
    match pair.as_rule() {
        Rule::unsigned_number | Rule::damage_amount => {
            let amount = pair
                .as_str()
                .parse::<u32>()
                .map_err(|_| ParseError::Internal("damage amount"))?;
            Ok(DamageAmount::Number(amount))
        }
        Rule::variable_name => Ok(DamageAmount::Variable(variable_from_str(pair.as_str())?)),
        _ => Err(ParseError::Internal("damage amount")),
    }
}

fn if_you_cant_source_deals_damage_to_you_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = inner
        .next()
        .ok_or(ParseError::Internal("if-you-cant damage missing source"))?;
    let amount_pair = inner
        .next()
        .ok_or(ParseError::Internal("if-you-cant damage missing amount"))?;
    Ok(Statement::IfYouCantSourceDealsDamageToYou {
        source: source_object_from_pair(source_pair)?,
        amount: damage_amount_from_pair(amount_pair)?,
    })
}

fn prevention_recipient_from_pair(pair: Pair<Rule>) -> Result<PreventionRecipient, ParseError> {
    match pair.as_rule() {
        Rule::any_target_prevention_recipient => Ok(PreventionRecipient::AnyTarget),
        Rule::that_permanent_or_player_prevention_recipient => {
            Ok(PreventionRecipient::ThatPermanentOrPlayer)
        }
        _ => Err(ParseError::Internal("prevention recipient")),
    }
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

fn if_you_do_effect_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let effect_pair = only_inner(pair, "if_you_do missing effect")?;
    Ok(Statement::if_you_do(if_you_do_effect_from_inner_pair(
        effect_pair,
    )?))
}

fn if_you_do_effect_from_inner_pair(pair: Pair<Rule>) -> Result<IfYouDoEffect, ParseError> {
    match pair.as_rule() {
        Rule::add_mana => {
            let mana_pair = only_inner(pair, "if_you_do add_mana missing mana_cost")?;
            Ok(IfYouDoEffect::AddMana {
                mana: mana_cost_from_pair(mana_pair),
            })
        }
        Rule::untap_source => {
            let source_pair = only_inner(pair, "if_you_do untap missing source_object")?;
            Ok(IfYouDoEffect::Untap {
                source: source_object_from_pair(source_pair)?,
            })
        }
        Rule::untap_referenced_permanent => {
            let permanent_type_pair =
                only_inner(pair, "if_you_do untap missing referenced permanent")?;
            Ok(IfYouDoEffect::UntapReferencedPermanent {
                permanent_type: permanent_type_from_pair(permanent_type_pair)?,
            })
        }
        Rule::gain_life_effect => Ok(IfYouDoEffect::GainLife {
            amount: if_you_do_gain_life_amount_from_pair(pair)?,
        }),
        Rule::damage_prevention_effect_this_turn => {
            let effect = damage_prevention_effect_from_this_turn_pair(pair)?;
            Ok(IfYouDoEffect::PreventDamageThisTurn { effect })
        }
        _ => Err(ParseError::Internal("if_you_do effect")),
    }
}

fn if_you_do_until_your_next_turn_you_cant_be_attacked_except_by_creatures_with_keywords_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let list_pair = only_inner(pair, "attack restriction missing keyword list")?;
    let keywords = list_pair
        .into_inner()
        .map(keyword_from_inner_pair)
        .collect::<Result<Vec<_>, _>>()?;
    Ok(
        Statement::IfYouDoUntilYourNextTurnYouCantBeAttackedExceptByCreaturesWithKeywords {
            keywords,
        },
    )
}

fn if_you_do_gain_life_amount_from_pair(pair: Pair<Rule>) -> Result<u32, ParseError> {
    if pair.as_rule() == Rule::unsigned_number {
        return pair
            .as_str()
            .parse::<u32>()
            .map_err(|_| ParseError::Internal("if_you_do_gain_life amount"));
    }

    for child in pair.into_inner() {
        if let Ok(amount) = if_you_do_gain_life_amount_from_pair(child) {
            return Ok(amount);
        }
    }
    Err(ParseError::Internal("if_you_do_gain_life missing amount"))
}

fn you_gain_life_from_pair(pair: Pair<Rule>) -> Result<TriggerEffect, ParseError> {
    let amount_pair = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("you_gain_life missing amount"))?;
    Ok(TriggerEffect::YouGainLife {
        amount: amount_pair
            .as_str()
            .parse::<u32>()
            .map_err(|_| ParseError::Internal("you_gain_life amount"))?,
    })
}

fn player_loses_life_from_pair(pair: Pair<Rule>) -> Result<TriggerEffect, ParseError> {
    let mut player = None;
    let mut amount = None;
    let mut rounding = None;
    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::its_owner_life_loss_player => player = Some(LifeLossPlayer::ItsOwner),
            Rule::half_their_life_loss_amount => {
                amount = Some(LifeLossAmount::HalfTheirLife {
                    rounding: Rounding::Down,
                });
            }
            Rule::unsigned_number => {
                amount = Some(LifeLossAmount::Number(
                    child
                        .as_str()
                        .parse::<u32>()
                        .map_err(|_| ParseError::Internal("life loss amount"))?,
                ));
            }
            Rule::rounding => rounding = Some(rounding_from_pair(child)?),
            _ => return Err(ParseError::Internal("player loses life part")),
        }
    }
    let amount = match (amount, rounding) {
        (Some(LifeLossAmount::HalfTheirLife { .. }), Some(rounding)) => {
            LifeLossAmount::HalfTheirLife { rounding }
        }
        (Some(amount), None) => amount,
        (Some(LifeLossAmount::Number(_)), Some(_)) => {
            return Err(ParseError::Internal("number life loss cannot be rounded"));
        }
        (None, _) => return Err(ParseError::Internal("player loses life missing amount")),
    };
    Ok(TriggerEffect::PlayerLosesLife {
        player: player.ok_or(ParseError::Internal("player loses life missing player"))?,
        amount,
    })
}

fn action_timing_from_pair(pair: Pair<Rule>) -> Result<ActionTiming, ParseError> {
    match pair.as_rule() {
        Rule::any_time_you_could_activate_a_mana_ability => {
            Ok(ActionTiming::AnyTimeYouCouldActivateAManaAbility)
        }
        Rule::any_time_you_could_cast_an_instant => Ok(ActionTiming::AnyTimeYouCouldCastAnInstant),
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
        Rule::pay_mana_cost => {
            let mana_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("pay_mana_cost missing mana"))?;
            Ok(OptionalCost::PayMana {
                mana: mana_cost_from_pair(mana_pair),
            })
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

fn keyword_list_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let keywords = pair
        .into_inner()
        .map(keyword_from_inner_pair)
        .collect::<Result<Vec<_>, _>>()?;
    if keywords.len() < 2 {
        return Err(ParseError::Internal("keyword_ability_list"));
    }
    Ok(Statement::KeywordList(keywords))
}

fn semicolon_keyword_list_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let keywords = pair
        .into_inner()
        .map(keyword_from_inner_pair)
        .collect::<Result<Vec<_>, _>>()?;
    if keywords.len() < 2 {
        return Err(ParseError::Internal("semicolon_keyword_ability_list"));
    }
    Ok(Statement::SemicolonKeywordList(keywords))
}

fn label_from_pair(pair: Pair<Rule>) -> Result<String, ParseError> {
    match pair.as_rule() {
        Rule::label_word => Ok(pair.as_str().to_ascii_lowercase()),
        Rule::label_phrase | Rule::quoted_label | Rule::quoted_label_with_inner_period => {
            let label_pair = only_inner(pair, "label missing word")?;
            label_from_pair(label_pair)
        }
        Rule::quoted_label_pile => {
            let label_pair = only_inner(pair, "quoted label pile missing label")?;
            label_from_pair(label_pair)
        }
        _ => Err(ParseError::Internal("label")),
    }
}

fn then_for_each_attacking_creature_choose_label_blocking_restriction_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mut labels = Vec::new();
    let mut keyword = None;
    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::quoted_label | Rule::quoted_label_with_inner_period => {
                labels.push(label_from_pair(child)?);
            }
            Rule::keyword_ability_name | Rule::landwalk | Rule::protection | Rule::enchant => {
                keyword = Some(keyword_from_inner_pair(child)?);
            }
            _ => return Err(ParseError::Internal("label blocking restriction child")),
        }
    }
    Ok(
        Statement::ForEachAttackingCreatureChooseLabelBlockingRestriction {
            labels,
            keyword: keyword.ok_or(ParseError::Internal(
                "label blocking restriction missing keyword",
            ))?,
        },
    )
}

fn triggered_ability_from_pair(pair: Pair<Rule>) -> Result<TriggeredAbility, ParseError> {
    let mut condition: Option<TriggerCondition> = None;
    let mut effects: Vec<TriggerEffect> = Vec::new();
    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::trigger_condition => condition = Some(trigger_condition_from_pair(child)?),
            Rule::trigger_effect_sequence | Rule::trigger_effect_fragment_sequence => {
                effects.extend(trigger_effect_sequence_from_pair(child)?);
            }
            _ => return Err(ParseError::Internal("triggered_ability child")),
        }
    }
    let condition = condition.ok_or(ParseError::Internal("triggered_ability missing condition"))?;
    if effects.is_empty() {
        return Err(ParseError::Internal("triggered_ability missing effect"));
    }
    Ok(TriggeredAbility::from_parts(condition, effects))
}

fn trigger_condition_from_pair(pair: Pair<Rule>) -> Result<TriggerCondition, ParseError> {
    let mut event: Option<TriggerEvent> = None;
    let mut intervening_if: Option<InterveningIf> = None;
    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::trigger_event_clause => event = Some(trigger_event_from_pair(child)?),
            Rule::trigger_intervening_if_clause => {
                intervening_if = Some(triggered_intervening_if_from_pair(child)?);
            }
            _ => return Err(ParseError::Internal("trigger_condition child")),
        }
    }
    Ok(TriggerCondition {
        event: event.ok_or(ParseError::Internal("trigger_condition missing event"))?,
        intervening_if,
    })
}

fn trigger_event_from_pair(pair: Pair<Rule>) -> Result<TriggerEvent, ParseError> {
    let event_pair = only_inner(pair, "trigger_event_clause missing event")?;
    match event_pair.as_rule() {
        Rule::aura_enters => Ok(TriggerEvent::ThisAuraEnters),
        Rule::aura_leaves_battlefield => Ok(TriggerEvent::ThisAuraLeavesTheBattlefield),
        Rule::permanent_enters => permanent_enters_from_pair(event_pair),
        Rule::player_casts_colored_spell => player_casts_colored_spell_from_pair(event_pair),
        Rule::player_taps_permanent_for_mana => {
            player_taps_permanent_for_mana_from_pair(event_pair)
        }
        Rule::basic_land_type_is_tapped_for_mana => {
            basic_land_type_is_tapped_for_mana_from_pair(event_pair)
        }
        Rule::basic_land_type_controller_becomes_status => {
            basic_land_type_controller_becomes_status_from_pair(event_pair)
        }
        Rule::one_or_more_creatures_you_control_attack => {
            Ok(TriggerEvent::OneOrMoreCreaturesYouControlAttack)
        }
        Rule::you_play_permanent => you_play_permanent_from_pair(event_pair),
        Rule::enchanted_permanent_dies => enchanted_permanent_dies_from_pair(event_pair),
        Rule::source_dies => source_dies_from_pair(event_pair),
        Rule::enchanted_object_becomes_status => {
            enchanted_object_becomes_status_from_pair(event_pair)
        }
        Rule::beginning_of_the_next_end_step => Ok(TriggerEvent::BeginningOfTheNextEndStep),
        Rule::beginning_of_the_end_step => Ok(TriggerEvent::BeginningOfTheEndStep),
        Rule::beginning_of_chosen_players_upkeep => {
            Ok(TriggerEvent::BeginningOfChosenPlayersUpkeep)
        }
        Rule::beginning_of_each_players_upkeep => Ok(TriggerEvent::BeginningOfEachPlayersUpkeep),
        Rule::beginning_of_each_players_draw_step => {
            Ok(TriggerEvent::BeginningOfEachPlayersDrawStep)
        }
        Rule::beginning_of_your_draw_step => Ok(TriggerEvent::BeginningOfYourDrawStep),
        Rule::beginning_of_your_upkeep => Ok(TriggerEvent::BeginningOfYourUpkeep),
        Rule::source_put_into_graveyard_from_battlefield => {
            source_put_into_graveyard_from_battlefield_from_pair(event_pair)
        }
        Rule::source_is_dealt_damage => source_is_dealt_damage_from_pair(event_pair),
        Rule::you_are_dealt_damage => Ok(TriggerEvent::YouAreDealtDamage),
        Rule::source_deals_damage_to_an_opponent => {
            source_deals_damage_to_an_opponent_from_pair(event_pair)
        }
        Rule::you_control_no_basic_lands => you_control_no_basic_lands_from_pair(event_pair),
        Rule::permanent_put_into_graveyard_from_battlefield => {
            permanent_put_into_graveyard_from_battlefield_from_pair(event_pair)
        }
        Rule::beginning_of_upkeep_of_enchanted_permanent_controller => {
            beginning_of_upkeep_of_enchanted_permanent_controller_from_pair(event_pair)
        }
        Rule::end_of_combat => Ok(TriggerEvent::EndOfCombat),
        Rule::source_blocks_or_becomes_blocked_by_non_creature_type_creature => {
            source_blocks_or_becomes_blocked_by_non_creature_type_creature_from_pair(event_pair)
        }
        _ => Err(ParseError::Internal("trigger event")),
    }
}

fn triggered_intervening_if_from_pair(pair: Pair<Rule>) -> Result<InterveningIf, ParseError> {
    let condition_pair = only_inner(pair, "trigger_intervening_if_clause missing condition")?;
    match condition_pair.as_rule() {
        Rule::its_on_the_battlefield => Ok(InterveningIf::ItsOnTheBattlefield),
        Rule::no_permanents_are_on_the_battlefield => {
            let permanent_type_pair =
                only_inner(condition_pair, "no permanents condition missing type")?;
            Ok(InterveningIf::NoPermanentsAreOnTheBattlefield {
                permanent_type: permanent_type_from_plural_pair(permanent_type_pair)?,
            })
        }
        Rule::source_attacked_or_blocked_this_combat => {
            let source_pair = only_inner(
                condition_pair,
                "attacked-or-blocked condition missing source",
            )?;
            Ok(InterveningIf::SourceAttackedOrBlockedThisCombat {
                source: source_object_from_pair(source_pair)?,
            })
        }
        Rule::source_object_is_status => {
            let mut inner = condition_pair.into_inner();
            let source_pair = inner.next().ok_or(ParseError::Internal(
                "source-status condition missing source",
            ))?;
            let status_pair = inner.next().ok_or(ParseError::Internal(
                "source-status condition missing status",
            ))?;
            Ok(InterveningIf::SourceIsStatus {
                source: source_object_from_pair(source_pair)?,
                status: object_status_from_pair(status_pair)?,
            })
        }
        Rule::this_card_in_your_graveyard_with_cards_above_it => {
            this_card_in_your_graveyard_with_cards_above_it_from_pair(condition_pair)
        }
        Rule::enchanted_has_keyword => {
            let mut inner = condition_pair.into_inner();
            let object_pair = inner.next().ok_or(ParseError::Internal(
                "enchanted-has condition missing object",
            ))?;
            let keyword_pair = inner.next().ok_or(ParseError::Internal(
                "enchanted-has condition missing keyword",
            ))?;
            Ok(InterveningIf::EnchantedHasKeyword {
                object: enchanted_object_from_pair(object_pair)?,
                keyword: keyword_from_inner_pair(keyword_pair)?,
            })
        }
        Rule::it_wasnt_first_permanent_you_played_this_turn => {
            let permanent_type_pair =
                condition_pair
                    .into_inner()
                    .next()
                    .ok_or(ParseError::Internal(
                        "first-play condition missing permanent_type",
                    ))?;
            Ok(InterveningIf::ItWasntFirstPermanentYouPlayedThisTurn {
                permanent_type: permanent_type_from_pair(permanent_type_pair)?,
            })
        }
        _ => Err(ParseError::Internal("intervening-if condition")),
    }
}

fn trigger_effect_sequence_from_pair(pair: Pair<Rule>) -> Result<Vec<TriggerEffect>, ParseError> {
    pair.into_inner().map(trigger_effect_from_pair).collect()
}

fn trigger_effect_from_pair(pair: Pair<Rule>) -> Result<TriggerEffect, ParseError> {
    match pair.as_rule() {
        Rule::destroy_that_creature_if_it_attacked_this_turn => {
            Ok(TriggerEffect::DestroyThatCreatureIfItAttackedThisTurn)
        }
        Rule::destroy_it => Ok(TriggerEffect::DestroyIt),
        Rule::destroy_that_creature_at_end_of_combat => {
            Ok(TriggerEffect::DestroyThatCreatureAtEndOfCombat)
        }
        Rule::that_creatures_controller_sacrifices_it => {
            Ok(TriggerEffect::ThatCreaturesControllerSacrificesIt)
        }
        Rule::loses_and_gains_keyword => loses_and_gains_keyword_from_pair(pair),
        Rule::return_enchanted_card_and_attach => return_enchanted_card_and_attach_from_pair(pair),
        Rule::sacrifice_source_effect => sacrifice_source_effect_from_pair(pair),
        Rule::sacrifice_source_unless_you_pay => sacrifice_source_unless_you_pay_from_pair(pair),
        Rule::sacrifice_permanent_other_than_source => {
            sacrifice_permanent_other_than_source_from_pair(pair)
        }
        Rule::sacrifice_that_many_nontoken_permanents => {
            Ok(TriggerEffect::SacrificeThatManyNontokenPermanents)
        }
        Rule::player_loses_life => player_loses_life_from_pair(pair),
        Rule::you_lose_the_game => Ok(TriggerEffect::YouLoseTheGame),
        Rule::you_gain_life => you_gain_life_from_pair(pair),
        Rule::you_may_pay_mana | Rule::player_may_pay_mana => you_may_pay_mana_from_pair(pair),
        Rule::damage_prevention_effect_sentence => {
            let (effect, definitions) = damage_prevention_effect_sentence_from_pair(pair)?;
            Ok(TriggerEffect::PreventDamage {
                effect,
                definitions,
            })
        }
        Rule::tap_enchanted_object => tap_enchanted_object_from_pair(pair),
        Rule::you_may_put_this_card_onto_the_battlefield => {
            Ok(TriggerEffect::YouMayPutThisCardOntoTheBattlefield)
        }
        Rule::if_you_do_gain_life | Rule::if_you_do_gain_life_fragment => {
            Ok(TriggerEffect::IfYouDoGainLife {
                amount: if_you_do_gain_life_amount_from_pair(pair)?,
            })
        }
        Rule::unless_you_pay_mana_do_actions => unless_you_pay_mana_do_actions_from_pair(pair),
        Rule::delayed_remove_all_named_counters_from_linked_land => {
            delayed_remove_all_named_counters_from_linked_land_from_pair(pair)
        }
        Rule::put_that_many_named_counters_on_source => {
            put_that_many_named_counters_on_source_from_pair(pair)
        }
        Rule::you_may_remove_named_counter_from_source => {
            you_may_remove_named_counter_from_source_from_pair(pair)
        }
        Rule::source_deals_damage => source_deals_damage_from_pair(pair),
        Rule::that_player_draws_an_additional_card => {
            Ok(TriggerEffect::ThatPlayerDrawsAnAdditionalCard)
        }
        Rule::that_player_discards_card_at_random => {
            Ok(TriggerEffect::ThatPlayerDiscardsCardAtRandom)
        }
        Rule::that_player_adds_mana_of_any_type_that_permanent_produced => {
            that_player_adds_mana_of_any_type_that_permanent_produced_from_pair(pair)
        }
        Rule::its_controller_adds_an_additional_mana => {
            its_controller_adds_an_additional_mana_from_pair(pair)
        }
        Rule::defending_player_divides_creatures_without_keyword_into_labeled_piles => {
            defending_player_divides_creatures_without_keyword_into_labeled_piles_from_pair(pair)
        }
        Rule::source_gains_static_ability => source_gains_static_ability_from_pair(pair),
        Rule::remove_pt_counter_from_it => remove_pt_counter_from_it_from_pair(pair),
        Rule::put_pt_counter_on_it => put_pt_counter_on_it_from_pair(pair),
        _ => Err(ParseError::Internal("trigger effect")),
    }
}

fn defending_player_divides_creatures_without_keyword_into_labeled_piles_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEffect, ParseError> {
    let mut keyword = None;
    let mut labels = Vec::new();
    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::keyword_ability_name | Rule::landwalk | Rule::protection | Rule::enchant => {
                keyword = Some(keyword_from_inner_pair(child)?);
            }
            Rule::quoted_label_pile => labels.push(label_from_pair(child)?),
            _ => return Err(ParseError::Internal("divide labeled piles child")),
        }
    }
    Ok(
        TriggerEffect::DefendingPlayerDividesCreaturesWithoutKeywordIntoLabeledPiles {
            keyword: keyword.ok_or(ParseError::Internal("divide labeled piles missing keyword"))?,
            labels,
        },
    )
}

fn this_card_in_your_graveyard_with_cards_above_it_from_pair(
    pair: Pair<Rule>,
) -> Result<InterveningIf, ParseError> {
    let mut inner = pair.into_inner();
    let zone_pair = inner.next().ok_or(ParseError::Internal(
        "this-card graveyard condition missing zone",
    ))?;
    let count_pair = inner.next().ok_or(ParseError::Internal(
        "this-card graveyard condition missing count",
    ))?;
    let card_type_pair = inner.next().ok_or(ParseError::Internal(
        "this-card graveyard condition missing card type",
    ))?;
    let count =
        number_word_to_u32(count_pair.as_str()).ok_or(ParseError::Internal("number_word"))?;
    Ok(InterveningIf::ThisCardInYourZoneWithCardsAboveIt {
        zone: zone_from_pair(zone_pair)?,
        count,
        card_type: permanent_type_from_pair(card_type_pair)?,
    })
}

fn permanent_enters_from_pair(pair: Pair<Rule>) -> Result<TriggerEvent, ParseError> {
    let pt = only_inner(pair, "permanent_enters missing permanent_type")?;
    Ok(TriggerEvent::PermanentEnters {
        permanent_type: permanent_type_from_pair(pt)?,
    })
}

fn player_casts_colored_spell_from_pair(pair: Pair<Rule>) -> Result<TriggerEvent, ParseError> {
    let color = only_inner(pair, "player_casts_colored_spell missing color")?;
    Ok(TriggerEvent::PlayerCastsColoredSpell {
        color: color_from_pair(color)?,
    })
}

fn player_taps_permanent_for_mana_from_pair(pair: Pair<Rule>) -> Result<TriggerEvent, ParseError> {
    let permanent_type = only_inner(
        pair,
        "player_taps_permanent_for_mana missing permanent_type",
    )?;
    Ok(TriggerEvent::PlayerTapsPermanentForMana {
        permanent_type: permanent_type_from_pair(permanent_type)?,
    })
}

fn you_control_no_basic_lands_from_pair(pair: Pair<Rule>) -> Result<TriggerEvent, ParseError> {
    let land_type_pair = only_inner(pair, "you_control_no_basic_lands missing land type")?;
    Ok(TriggerEvent::YouControlNoBasicLands {
        land_type: basic_land_type_from_plural_pair(land_type_pair)?,
    })
}

fn basic_land_type_is_tapped_for_mana_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEvent, ParseError> {
    let land_type = only_inner(
        pair,
        "basic_land_type_is_tapped_for_mana missing basic_land_type",
    )?;
    Ok(TriggerEvent::BasicLandTypeIsTappedForMana {
        land_type: basic_land_type_from_pair(land_type)?,
    })
}

fn basic_land_type_controller_becomes_status_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEvent, ParseError> {
    let mut inner = pair.into_inner();
    let land_type_pair = inner.next().ok_or(ParseError::Internal(
        "basic_land_type_controller_becomes_status missing basic_land_type",
    ))?;
    let controller_pair = inner.next().ok_or(ParseError::Internal(
        "basic_land_type_controller_becomes_status missing controller",
    ))?;
    let status_pair = inner.next().ok_or(ParseError::Internal(
        "basic_land_type_controller_becomes_status missing status",
    ))?;
    Ok(TriggerEvent::BasicLandTypeControllerBecomesStatus {
        land_type: basic_land_type_from_pair(land_type_pair)?,
        controller: permanent_controller_from_pair(controller_pair)?,
        status: object_status_from_pair(status_pair)?,
    })
}

fn you_play_permanent_from_pair(pair: Pair<Rule>) -> Result<TriggerEvent, ParseError> {
    let permanent_type = only_inner(pair, "you_play_permanent missing permanent_type")?;
    Ok(TriggerEvent::YouPlayPermanent {
        permanent_type: permanent_type_from_pair(permanent_type)?,
    })
}

fn enchanted_permanent_dies_from_pair(pair: Pair<Rule>) -> Result<TriggerEvent, ParseError> {
    let pt = only_inner(pair, "enchanted_permanent_dies missing permanent_type")?;
    Ok(TriggerEvent::EnchantedPermanentDies {
        permanent_type: permanent_type_from_pair(pt)?,
    })
}

fn source_dies_from_pair(pair: Pair<Rule>) -> Result<TriggerEvent, ParseError> {
    let source_pair = only_inner(pair, "source_dies missing source_object")?;
    Ok(TriggerEvent::SourceDies {
        source: source_object_from_pair(source_pair)?,
    })
}

fn enchanted_object_becomes_status_from_pair(pair: Pair<Rule>) -> Result<TriggerEvent, ParseError> {
    let mut inner = pair.into_inner();
    let object_pair = inner.next().ok_or(ParseError::Internal(
        "enchanted_object_becomes_status missing object",
    ))?;
    let status_pair = inner.next().ok_or(ParseError::Internal(
        "enchanted_object_becomes_status missing status",
    ))?;
    Ok(TriggerEvent::EnchantedObjectBecomesStatus {
        object: enchanted_object_from_pair(object_pair)?,
        status: object_status_from_pair(status_pair)?,
    })
}

fn beginning_of_upkeep_of_enchanted_permanent_controller_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEvent, ParseError> {
    let pt = only_inner(
        pair,
        "beginning_of_upkeep_of_enchanted_permanent_controller missing permanent_type",
    )?;
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

fn source_is_dealt_damage_from_pair(pair: Pair<Rule>) -> Result<TriggerEvent, ParseError> {
    let source_pair = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("source dealt damage missing source"))?;
    Ok(TriggerEvent::SourceIsDealtDamage {
        source: source_object_from_pair(source_pair)?,
    })
}

fn source_deals_damage_to_an_opponent_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEvent, ParseError> {
    let source_pair = only_inner(pair, "source deals damage to opponent missing source")?;
    Ok(TriggerEvent::SourceDealsDamageToAnOpponent {
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

fn that_player_adds_mana_of_any_type_that_permanent_produced_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let amount_pair = inner
        .next()
        .ok_or(ParseError::Internal("mana-produced effect missing amount"))?;
    let permanent_type_pair = inner.next().ok_or(ParseError::Internal(
        "mana-produced effect missing permanent type",
    ))?;
    let amount = number_word_to_u32(amount_pair.as_str())
        .ok_or(ParseError::Internal("mana-produced effect amount"))?;
    Ok(
        TriggerEffect::ThatPlayerAddsManaOfAnyTypeThatPermanentProduced {
            amount,
            permanent_type: permanent_type_from_pair(permanent_type_pair)?,
        },
    )
}

fn its_controller_adds_an_additional_mana_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEffect, ParseError> {
    let mana_pair = only_inner(
        pair,
        "its_controller_adds_an_additional_mana missing mana_symbol",
    )?;
    Ok(TriggerEffect::ItsControllerAddsAdditionalMana {
        mana: mana_symbol_from_pair(mana_pair),
    })
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
        Rule::keyword_ability_name => Ok(Keyword::Named(named_keyword_ability_from_str(
            pair.as_str(),
        )?)),
        Rule::landwalk => {
            let land_type = only_inner(pair, "landwalk missing basic_land_type")?;
            Ok(Keyword::Landwalk(basic_land_type_from_pair(land_type)?))
        }
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

fn named_keyword_ability_from_str(text: &str) -> Result<NamedKeywordAbility, ParseError> {
    match text.to_ascii_lowercase().as_str() {
        "first strike" => Ok(NamedKeywordAbility::FirstStrike),
        "flying" => Ok(NamedKeywordAbility::Flying),
        "reach" => Ok(NamedKeywordAbility::Reach),
        "haste" => Ok(NamedKeywordAbility::Haste),
        "defender" => Ok(NamedKeywordAbility::Defender),
        "banding" => Ok(NamedKeywordAbility::Banding),
        "trample" => Ok(NamedKeywordAbility::Trample),
        "indestructible" => Ok(NamedKeywordAbility::Indestructible),
        "fear" => Ok(NamedKeywordAbility::Fear),
        _ => Err(ParseError::Internal("keyword ability name")),
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

fn sacrifice_source_effect_from_pair(pair: Pair<Rule>) -> Result<TriggerEffect, ParseError> {
    let source_pair = only_inner(pair, "sacrifice source effect missing source")?;
    Ok(TriggerEffect::SacrificeSource {
        source: source_object_from_pair(source_pair)?,
    })
}

fn sacrifice_permanent_other_than_source_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let permanent_type_pair = inner.next().ok_or(ParseError::Internal(
        "sacrifice other permanent missing permanent_type",
    ))?;
    let source_pair = inner.next().ok_or(ParseError::Internal(
        "sacrifice other permanent missing source",
    ))?;
    Ok(TriggerEffect::SacrificePermanentOtherThanSource {
        permanent_type: permanent_type_from_pair(permanent_type_pair)?,
        source: source_object_from_pair(source_pair)?,
    })
}

fn you_may_pay_mana_from_pair(pair: Pair<Rule>) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let player_pair = inner
        .next()
        .ok_or(ParseError::Internal("you_may_pay_mana missing player"))?;
    let amount_pair = inner
        .next()
        .ok_or(ParseError::Internal("you_may_pay_mana missing amount"))?;
    Ok(TriggerEffect::YouMayPayMana {
        player: pay_mana_player_from_pair(player_pair)?,
        amount: pay_mana_amount_from_pair(amount_pair)?,
    })
}

fn pay_mana_player_from_pair(pair: Pair<Rule>) -> Result<PayManaPlayer, ParseError> {
    match pair.as_rule() {
        Rule::you_pay_mana_player => Ok(PayManaPlayer::You),
        Rule::that_player_pay_mana_player => Ok(PayManaPlayer::ThatPlayer),
        _ => Err(ParseError::Internal("pay mana player")),
    }
}

fn pay_mana_amount_from_pair(pair: Pair<Rule>) -> Result<PayManaAmount, ParseError> {
    match pair.as_rule() {
        Rule::mana_cost => Ok(PayManaAmount::Cost(mana_cost_from_pair(pair))),
        Rule::any_amount_of_mana => Ok(PayManaAmount::AnyAmountOfMana),
        _ => Err(ParseError::Internal("pay mana amount")),
    }
}

fn tap_enchanted_object_from_pair(pair: Pair<Rule>) -> Result<TriggerEffect, ParseError> {
    let object_pair = only_inner(pair, "tap_enchanted_object missing object")?;
    Ok(TriggerEffect::TapEnchanted(enchanted_object_from_pair(
        object_pair,
    )?))
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

fn source_deals_damage_from_pair(pair: Pair<Rule>) -> Result<TriggerEffect, ParseError> {
    let mut event = None;
    let mut condition = None;
    let mut definitions = None;

    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::trigger_damage_event => {
                event = Some(trigger_damage_event_from_pair(child)?);
            }
            Rule::trigger_damage_condition => {
                condition = Some(trigger_damage_condition_from_pair(child)?);
            }
            Rule::trigger_damage_variable_definition => {
                let where_pair = only_inner(child, "trigger damage missing where clause")?;
                definitions = Some(where_clause_from_pair(where_pair)?);
            }
            _ => return Err(ParseError::Internal("source deals damage part")),
        }
    }

    let event = event.ok_or(ParseError::Internal("source deals damage missing event"))?;
    validate_trigger_damage_amount_recipient(event.amount, event.recipient)?;

    let definitions = match (event.amount, definitions) {
        (DamageAmount::Number(_), Some(_)) => {
            return Err(ParseError::Internal(
                "number damage cannot have variable definition",
            ));
        }
        (DamageAmount::Number(_), None) => Vec::new(),
        (DamageAmount::Variable(amount), Some(definitions)) => {
            if !definitions
                .iter()
                .any(|definition| definition.variable == amount)
            {
                return Err(ParseError::Internal(
                    "variable damage missing amount definition",
                ));
            }
            definitions
        }
        (DamageAmount::Variable(_), None) => {
            return Err(ParseError::Internal(
                "variable damage missing amount definition",
            ));
        }
        (DamageAmount::ThatPermanentsToughness(_), Some(_))
        | (DamageAmount::NumberOfBasicLandsTheyControl(_), Some(_)) => {
            return Err(ParseError::Internal(
                "equal-to damage cannot have variable definition",
            ));
        }
        (DamageAmount::ThatPermanentsToughness(_), None)
        | (DamageAmount::NumberOfBasicLandsTheyControl(_), None) => Vec::new(),
    };

    Ok(TriggerEffect::SourceDealsDamage(TriggeredDamage {
        event,
        condition,
        definitions,
    }))
}

fn trigger_damage_event_from_pair(
    pair: Pair<Rule>,
) -> Result<DamageEvent<TriggerDamageSource, TriggerDamageRecipient>, ParseError> {
    let mut source = None;
    let mut amount = None;
    let mut recipient = None;

    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::source_object => {
                source = Some(TriggerDamageSource::Source(source_object_from_pair(child)?));
            }
            Rule::it_damage_source => {
                source = Some(TriggerDamageSource::It);
            }
            Rule::trigger_damage_amount => {
                amount = Some(trigger_damage_amount_from_pair(child)?);
            }
            Rule::trigger_damage_equal_to_amount => {
                amount = Some(trigger_damage_equal_to_amount_from_pair(child)?);
            }
            Rule::that_permanents_controller
            | Rule::the_permanents_controller
            | Rule::that_player_damage_recipient
            | Rule::you_damage_recipient
            | Rule::that_permanent_damage_recipient => {
                recipient = Some(trigger_damage_recipient_from_pair(child)?);
            }
            _ => return Err(ParseError::Internal("trigger damage event part")),
        }
    }

    Ok(DamageEvent {
        source: source.ok_or(ParseError::Internal("trigger damage event missing source"))?,
        amount: amount.ok_or(ParseError::Internal("trigger damage event missing amount"))?,
        recipient: recipient.ok_or(ParseError::Internal(
            "trigger damage event missing recipient",
        ))?,
    })
}

fn damage_number_from_pair(pair: Pair<Rule>, context: &'static str) -> Result<u32, ParseError> {
    pair.as_str()
        .parse::<u32>()
        .map_err(|_| ParseError::Internal(context))
}

fn trigger_damage_amount_from_pair(pair: Pair<Rule>) -> Result<DamageAmount, ParseError> {
    let inner = only_inner(pair, "trigger damage amount missing inner rule")?;
    damage_amount_from_pair(inner)
}

fn trigger_damage_equal_to_amount_from_pair(pair: Pair<Rule>) -> Result<DamageAmount, ParseError> {
    let inner = only_inner(pair, "trigger damage equal amount missing inner rule")?;
    match inner.as_rule() {
        Rule::that_permanents_toughness => Ok(DamageAmount::ThatPermanentsToughness(
            permanent_type_from_inner_pair(inner)?,
        )),
        Rule::basic_land_type_plural => Ok(DamageAmount::NumberOfBasicLandsTheyControl(
            basic_land_type_from_plural_pair(inner)?,
        )),
        _ => Err(ParseError::Internal("trigger damage equal amount")),
    }
}

fn validate_trigger_damage_amount_recipient(
    amount: DamageAmount,
    recipient: TriggerDamageRecipient,
) -> Result<(), ParseError> {
    match (amount, recipient) {
        (
            DamageAmount::ThatPermanentsToughness(amount_type),
            TriggerDamageRecipient::ThatPermanentController(recipient_type),
        ) if amount_type == recipient_type => Ok(()),
        (
            DamageAmount::ThatPermanentsToughness(_),
            TriggerDamageRecipient::ThatPermanentController(_),
        ) => Err(ParseError::Internal("toughness damage references mismatch")),
        (DamageAmount::ThatPermanentsToughness(_), _) => Err(ParseError::Internal(
            "toughness damage must be dealt to that permanent's controller",
        )),
        (DamageAmount::NumberOfBasicLandsTheyControl(_), TriggerDamageRecipient::ThatPlayer) => {
            Ok(())
        }
        (DamageAmount::NumberOfBasicLandsTheyControl(_), _) => Err(ParseError::Internal(
            "basic-land-count damage must be dealt to that player",
        )),
        (DamageAmount::Number(_) | DamageAmount::Variable(_), _) => Ok(()),
    }
}

fn trigger_damage_recipient_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerDamageRecipient, ParseError> {
    match pair.as_rule() {
        Rule::that_permanents_controller => Ok(TriggerDamageRecipient::ThatPermanentController(
            that_permanents_controller_from_pair(pair)?,
        )),
        Rule::the_permanents_controller => Ok(TriggerDamageRecipient::ThatPermanentController(
            the_permanents_controller_from_pair(pair)?,
        )),
        Rule::that_player_damage_recipient => Ok(TriggerDamageRecipient::ThatPlayer),
        Rule::you_damage_recipient => Ok(TriggerDamageRecipient::You),
        Rule::that_permanent_damage_recipient => {
            let permanent_type_pair = only_inner(
                pair,
                "that permanent damage recipient missing permanent_type",
            )?;
            Ok(TriggerDamageRecipient::ThatPermanent(
                permanent_type_from_pair(permanent_type_pair)?,
            ))
        }
        _ => Err(ParseError::Internal("trigger damage recipient")),
    }
}

fn trigger_damage_condition_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerDamageCondition, ParseError> {
    let cost_pair = only_inner(pair, "trigger damage condition missing cost")?;
    Ok(TriggerDamageCondition::UnlessYouPay(mana_cost_from_pair(
        cost_pair,
    )))
}

fn activated_damage_effect_from_pair(
    pair: Pair<Rule>,
) -> Result<ActivatedDamageEffect, ParseError> {
    match pair.as_rule() {
        Rule::activated_direct_damage_effect => activated_direct_damage_effect_from_pair(pair),
        Rule::next_damage_event_effect => next_damage_event_effect_from_pair(pair),
        Rule::next_damage_redirection_effect => next_damage_redirection_effect_from_pair(pair),
        Rule::activated_damage_prevention_effect => {
            let effect = activated_damage_prevention_effect_from_pair(pair)?;
            Ok(ActivatedDamageEffect::PreventDamageThisTurn { effect })
        }
        _ => Err(ParseError::Internal("activated damage effect")),
    }
}

fn activated_direct_damage_effect_from_pair(
    pair: Pair<Rule>,
) -> Result<ActivatedDamageEffect, ParseError> {
    let mut inner = pair.into_inner();
    let source_pair = next_inner(&mut inner, "activated direct damage missing source")?;
    let source = source_object_from_pair(source_pair)?;
    let assignments = inner
        .map(activated_damage_assignments_from_pair)
        .collect::<Result<Vec<_>, _>>()?
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();
    if assignments.is_empty() {
        return Err(ParseError::Internal(
            "activated direct damage missing assignment",
        ));
    }
    Ok(ActivatedDamageEffect::SourceDealsDamage {
        source,
        assignments,
    })
}

fn activated_damage_assignments_from_pair(
    pair: Pair<Rule>,
) -> Result<Vec<DamageAssignment<ActivatedDamageRecipient>>, ParseError> {
    if pair.as_rule() != Rule::damage_assignment {
        return Err(ParseError::Internal("activated damage assignment"));
    }
    let mut inner = pair.into_inner();
    let amount_pair = next_inner(&mut inner, "activated damage assignment missing amount")?;
    let recipient_list_pair = next_inner(
        &mut inner,
        "activated damage assignment missing recipient list",
    )?;
    let amount = damage_event_amount_from_pair(amount_pair)?;
    recipient_list_pair
        .into_inner()
        .map(|recipient_pair| {
            Ok(DamageAssignment {
                amount,
                recipient: activated_damage_recipient_from_pair(recipient_pair)?,
            })
        })
        .collect()
}

fn activated_damage_prevention_effect_from_pair(
    pair: Pair<Rule>,
) -> Result<DamagePreventionEffect<ActivatedDamageRecipient>, ParseError> {
    damage_prevention_effect_from_this_turn_pair_with_recipient(
        pair,
        Rule::activated_damage_prevention_recipient_clause,
        |recipient_pair| {
            activated_damage_recipient_from_pair(only_inner(
                recipient_pair,
                "activated damage prevention missing recipient",
            )?)
        },
        "activated damage prevention child",
    )
}

fn next_damage_event_effect_from_pair(
    pair: Pair<Rule>,
) -> Result<ActivatedDamageEffect, ParseError> {
    let mut source = None;
    let mut kind = DamageKind::Damage;
    let mut recipient = None;
    let mut effect = None;

    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::colored_damage_source
            | Rule::unblocked_creature_damage_source
            | Rule::chosen_damage_source => {
                source = Some(activated_damage_source_from_pair(child)?);
            }
            Rule::damage_kind => {
                kind = DamageKind::CombatDamage;
            }
            Rule::you_damage_recipient | Rule::target_permanent_damage_recipient => {
                recipient = Some(activated_damage_recipient_from_pair(child)?);
            }
            Rule::prevent_that_damage
            | Rule::prevent_all_but_that_damage
            | Rule::redirect_that_damage_to_you => {
                effect = Some(activated_damage_event_effect_from_pair(child)?);
            }
            _ => return Err(ParseError::Internal("next damage event part")),
        }
    }

    Ok(ActivatedDamageEffect::NextDamageEvent {
        event: DamageEventPattern {
            source: source.ok_or(ParseError::Internal("next damage event missing source"))?,
            kind,
            recipient: recipient
                .ok_or(ParseError::Internal("next damage event missing recipient"))?,
        },
        effect: effect.ok_or(ParseError::Internal("next damage event missing effect"))?,
    })
}

fn next_damage_redirection_effect_from_pair(
    pair: Pair<Rule>,
) -> Result<ActivatedDamageEffect, ParseError> {
    let mut amount = None;
    let mut kind = None;
    let mut recipient = None;
    let mut destination = None;

    for child in pair.into_inner() {
        match child.as_rule() {
            Rule::damage_prevention_amount => {
                amount = Some(damage_amount_from_pair(only_inner(
                    child,
                    "damage redirection missing amount",
                )?)?);
            }
            Rule::unsigned_number | Rule::variable_name => {
                amount = Some(damage_amount_from_pair(child)?);
            }
            Rule::damage_kind => kind = Some(damage_kind_from_pair(child)?),
            Rule::activated_damage_redirection_recipient => {
                recipient = Some(activated_damage_redirection_recipient_from_pair(child)?);
            }
            Rule::you_damage_recipient
            | Rule::target_permanent_damage_recipient
            | Rule::source_object_damage_recipient => {
                recipient = Some(activated_damage_recipient_from_pair(child)?);
            }
            Rule::its_owner_damage_destination => {
                destination = Some(DamageRedirectionDestination::ItsOwner);
            }
            _ => return Err(ParseError::Internal("damage redirection part")),
        }
    }

    Ok(ActivatedDamageEffect::RedirectNextDamageThisTurn {
        amount: amount.ok_or(ParseError::Internal("damage redirection missing amount"))?,
        kind,
        recipient: recipient.ok_or(ParseError::Internal("damage redirection missing recipient"))?,
        destination: destination.ok_or(ParseError::Internal(
            "damage redirection missing destination",
        ))?,
    })
}

fn activated_damage_source_from_pair(
    pair: Pair<Rule>,
) -> Result<ActivatedDamageSource, ParseError> {
    match pair.as_rule() {
        Rule::colored_damage_source => {
            let color_pair = pair
                .into_inner()
                .find(|child| child.as_rule() == Rule::color_word)
                .ok_or(ParseError::Internal("colored damage source missing color"))?;
            Ok(ActivatedDamageSource::ColoredSource {
                color: color_from_pair(color_pair)?,
            })
        }
        Rule::unblocked_creature_damage_source => Ok(ActivatedDamageSource::UnblockedCreature),
        Rule::chosen_damage_source => Ok(ActivatedDamageSource::Source),
        _ => Err(ParseError::Internal("activated damage source")),
    }
}

fn activated_damage_recipient_from_pair(
    pair: Pair<Rule>,
) -> Result<ActivatedDamageRecipient, ParseError> {
    match pair.as_rule() {
        Rule::you_damage_recipient => Ok(ActivatedDamageRecipient::You),
        Rule::any_target_prevention_recipient => Ok(ActivatedDamageRecipient::AnyTarget),
        Rule::each_creature_damage_recipient => Ok(ActivatedDamageRecipient::EachCreature),
        Rule::each_player_damage_recipient => Ok(ActivatedDamageRecipient::EachPlayer),
        Rule::source_object_damage_recipient => {
            let source_pair = only_inner(pair, "source damage recipient missing source")?;
            Ok(ActivatedDamageRecipient::Source(source_object_from_pair(
                source_pair,
            )?))
        }
        Rule::target_permanent_damage_recipient => {
            let permanent_type_pair = only_inner(pair, "target damage recipient missing type")?;
            Ok(ActivatedDamageRecipient::TargetPermanent {
                permanent_type: permanent_type_from_pair(permanent_type_pair)?,
            })
        }
        _ => Err(ParseError::Internal("activated damage recipient")),
    }
}

fn activated_damage_redirection_recipient_from_pair(
    pair: Pair<Rule>,
) -> Result<ActivatedDamageRecipient, ParseError> {
    let recipient_pair = only_inner(pair, "damage redirection recipient missing inner rule")?;
    match recipient_pair.as_rule() {
        Rule::you_damage_recipient
        | Rule::target_permanent_damage_recipient
        | Rule::source_object_damage_recipient => {
            activated_damage_recipient_from_pair(recipient_pair)
        }
        _ => Err(ParseError::Internal("damage redirection recipient")),
    }
}

fn activated_damage_event_effect_from_pair(
    pair: Pair<Rule>,
) -> Result<ActivatedDamageEventEffect, ParseError> {
    match pair.as_rule() {
        Rule::prevent_that_damage => Ok(ActivatedDamageEventEffect::PreventThatDamage),
        Rule::prevent_all_but_that_damage => {
            let amount_pair = only_inner(pair, "prevent all but damage missing amount")?;
            Ok(ActivatedDamageEventEffect::PreventAllBut {
                amount: damage_number_from_pair(amount_pair, "prevent all but damage amount")?,
            })
        }
        Rule::redirect_that_damage_to_you => Ok(ActivatedDamageEventEffect::RedirectToYou),
        _ => Err(ParseError::Internal("activated damage event effect")),
    }
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

fn put_pt_counter_on_it_from_pair(pair: Pair<Rule>) -> Result<TriggerEffect, ParseError> {
    let counter_pair = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("put counter missing counter"))?;
    Ok(TriggerEffect::PutCounterOnIt {
        counter: pt_modifier_from_counter_pair(counter_pair)?,
    })
}

fn put_that_many_named_counters_on_source_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let counter_pair = inner
        .next()
        .ok_or(ParseError::Internal("put named counters missing counter"))?;
    let source_pair = inner
        .next()
        .ok_or(ParseError::Internal("put named counters missing source"))?;
    Ok(TriggerEffect::PutThatManyNamedCountersOnSource {
        counter_name: counter_name_from_counter_pair(counter_pair)?,
        source: source_object_from_pair(source_pair)?,
    })
}

fn you_may_remove_named_counter_from_source_from_pair(
    pair: Pair<Rule>,
) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let counter_pair = inner
        .next()
        .ok_or(ParseError::Internal("remove named counter missing counter"))?;
    let source_pair = inner
        .next()
        .ok_or(ParseError::Internal("remove named counter missing source"))?;
    Ok(TriggerEffect::YouMayRemoveNamedCounterFromSource {
        counter_name: counter_name_from_counter_pair(counter_pair)?,
        source: source_object_from_pair(source_pair)?,
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
    let kind = only_inner(pair, "source_object missing kind")?;
    match kind.as_rule() {
        Rule::permanent_type => Ok(SourceObject::This(permanent_type_from_pair(kind)?)),
        Rule::aura_source_object => Ok(SourceObject::ThisAura),
        _ => Err(ParseError::Internal("source_object kind")),
    }
}

fn source_object_from_possessive_pair(pair: Pair<Rule>) -> Result<SourceObject, ParseError> {
    if pair.as_rule() != Rule::source_object_possessive {
        return Err(ParseError::Internal("source_object_possessive"));
    }
    let kind = only_inner(pair, "source_object_possessive missing kind")?;
    match kind.as_rule() {
        Rule::permanent_type => Ok(SourceObject::This(permanent_type_from_pair(kind)?)),
        Rule::aura_source_object => Ok(SourceObject::ThisAura),
        _ => Err(ParseError::Internal("source_object_possessive kind")),
    }
}

fn that_permanents_controller_from_pair(pair: Pair<Rule>) -> Result<PermanentType, ParseError> {
    if pair.as_rule() != Rule::that_permanents_controller {
        return Err(ParseError::Internal("that_permanents_controller"));
    }
    let pt = only_inner(pair, "that_permanents_controller missing permanent_type")?;
    permanent_type_from_pair(pt)
}

fn the_permanents_controller_from_pair(pair: Pair<Rule>) -> Result<PermanentType, ParseError> {
    if pair.as_rule() != Rule::the_permanents_controller {
        return Err(ParseError::Internal("the_permanents_controller"));
    }
    let pt = only_inner(pair, "the_permanents_controller missing permanent_type")?;
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
        Rule::static_colored_spells_cost_mana_more_to_cast => {
            let mut inner = pair.into_inner();
            let color_pair = inner
                .next()
                .expect("static_colored_spells_cost_mana_more_to_cast begins with a color");
            let mana_pair = inner
                .next()
                .expect("static_colored_spells_cost_mana_more_to_cast has a mana cost");
            Ok(StaticAbility::ColoredSpellsCostManaMoreToCast {
                color: color_from_pair(color_pair)?,
                mana: mana_cost_from_pair(mana_pair),
            })
        }
        Rule::static_activated_abilities_of_colored_permanents_cost_mana_more_to_activate => {
            let mut inner = pair.into_inner();
            let color_pair = inner.next().expect(
                "static_activated_abilities_of_colored_permanents_cost_mana_more_to_activate begins with a color",
            );
            let permanent_type_pair = inner.next().expect(
                "static_activated_abilities_of_colored_permanents_cost_mana_more_to_activate names a permanent type",
            );
            let mana_pair = inner.next().expect(
                "static_activated_abilities_of_colored_permanents_cost_mana_more_to_activate has a mana cost",
            );
            Ok(
                StaticAbility::ActivatedAbilitiesOfColoredPermanentsCostManaMoreToActivate {
                    color: color_from_pair(color_pair)?,
                    permanent_type: permanent_type_from_plural_pair(permanent_type_pair)?,
                    mana: mana_cost_from_pair(mana_pair),
                },
            )
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
        Rule::static_other_creature_type_get_and_have_keyword => {
            let mut inner = pair.into_inner();
            let creature_type_pair = inner.next().expect(
                "static_other_creature_type_get_and_have_keyword names the creature subtype",
            );
            let modifier_pair = inner
                .next()
                .expect("static_other_creature_type_get_and_have_keyword has a pt_modifier");
            let keyword_pair = inner
                .next()
                .expect("static_other_creature_type_get_and_have_keyword has a keyword");
            Ok(StaticAbility::OtherCreatureTypeGetAndHaveKeyword {
                creature_type: creature_type_from_plural_pair(creature_type_pair)?,
                modifier: pt_modifier_from_pair(modifier_pair)?,
                keyword: keyword_from_inner_pair(keyword_pair)?,
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
        Rule::static_enchanted_has_triggered_ability => {
            let mut inner = pair.into_inner();
            let object_pair = inner
                .next()
                .expect("static_enchanted_has_triggered_ability begins with enchanted object");
            let ability_pair = inner
                .next()
                .expect("static_enchanted_has_triggered_ability names granted ability");
            Ok(StaticAbility::EnchantedHasTriggeredAbility {
                object: enchanted_object_from_pair(object_pair)?,
                ability: triggered_ability_from_pair(ability_pair)?,
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
                land_type: basic_land_type_reference_from_pair(land_type_pair)?,
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
        Rule::static_enchanted_can_attack_as_though_it_had => {
            let mut inner = pair.into_inner();
            let object_pair = inner
                .next()
                .expect("static_enchanted_can_attack begins with enchanted object");
            let keyword_pair = inner
                .next()
                .expect("static_enchanted_can_attack names gained keyword");
            Ok(StaticAbility::EnchantedCanAttackAsThoughItHad {
                object: enchanted_object_from_pair(object_pair)?,
                keyword: keyword_from_inner_pair(keyword_pair)?,
            })
        }
        Rule::static_enchanted_can_attack_as_though_it_didnt_have => {
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
        Rule::static_enchanted_cant_be_blocked_except_by_creature_type => {
            let mut inner = pair.into_inner();
            let object_pair = inner
                .next()
                .expect("static_enchanted_cant_be_blocked begins with enchanted object");
            let except_type_pair = inner
                .next()
                .expect("static_enchanted_cant_be_blocked names blocking creature type");
            Ok(StaticAbility::EnchantedCantBeBlockedExceptByCreatureType {
                object: enchanted_object_from_pair(object_pair)?,
                except_type: creature_type_from_plural_pair(except_type_pair)?,
            })
        }
        Rule::static_all_creatures_able_to_block_enchanted_do_so => {
            let object_pair = pair
                .into_inner()
                .next()
                .expect("static_all_creatures_able_to_block_enchanted names enchanted object");
            Ok(StaticAbility::AllCreaturesAbleToBlockEnchantedDoSo {
                object: enchanted_object_from_pair(object_pair)?,
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
        Rule::static_you_have_no_maximum_hand_size => Ok(StaticAbility::YouHaveNoMaximumHandSize),
        Rule::you_dont_lose_game_for_having_zero_or_less_life => {
            Ok(StaticAbility::YouDontLoseGameForHavingZeroOrLessLife)
        }
        Rule::if_you_would_gain_life_draw_that_many_cards_instead => {
            Ok(StaticAbility::IfYouWouldGainLifeDrawThatManyCardsInstead)
        }
        Rule::static_if_effect_causes_you_to_discard_card_you_may_put_it_on_top_of_library_instead => {
            Ok(StaticAbility::IfEffectCausesYouToDiscardCardYouMayPutItOnTopOfYourLibraryInstead)
        }
        Rule::static_you_may_play_any_number_of_permanents_on_each_of_your_turns => {
            let permanent_type_pair = pair
                .into_inner()
                .next()
                .expect("play permission names a permanent type");
            Ok(
                StaticAbility::YouMayPlayAnyNumberOfPermanentsOnEachOfYourTurns {
                    permanent_type: permanent_type_from_plural_pair(permanent_type_pair)?,
                },
            )
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
        Rule::static_source_enters_tapped => {
            let source_pair = pair
                .into_inner()
                .next()
                .expect("enters-tapped ability begins with source object");
            Ok(StaticAbility::SourceEntersTapped {
                source: source_object_from_pair(source_pair)?,
            })
        }
        Rule::static_effect_doesnt_remove_this_aura => {
            Ok(StaticAbility::EffectDoesntRemoveThisAura)
        }
        Rule::static_source_attacks_each_combat_if_able => {
            let source_pair = pair
                .into_inner()
                .next()
                .expect("static attack requirement begins with a source object");
            Ok(StaticAbility::SourceAttacksEachCombatIfAble {
                source: source_object_from_pair(source_pair)?,
            })
        }
        Rule::static_source_cant_attack_unless_defending_player_controls_basic_land => {
            let mut inner = pair.into_inner();
            let source_pair = inner
                .next()
                .expect("static attack restriction begins with a source object");
            let land_type_pair = inner
                .next()
                .expect("static attack restriction names a basic land type");
            Ok(
                StaticAbility::SourceCantAttackUnlessDefendingPlayerControlsBasicLand {
                    source: source_object_from_pair(source_pair)?,
                    land_type: basic_land_type_from_pair(land_type_pair)?,
                },
            )
        }
        Rule::static_source_cant_be_blocked_by_creature_type => {
            let mut inner = pair.into_inner();
            let source_pair = inner
                .next()
                .expect("static blocking restriction begins with a source object");
            let blocked_by_pair = inner
                .next()
                .expect("static blocking restriction names blocked-by creature type");
            Ok(StaticAbility::SourceCantBeBlockedByCreatureType {
                source: source_object_from_pair(source_pair)?,
                blocked_by: creature_type_from_plural_pair(blocked_by_pair)?,
            })
        }
        Rule::static_source_doesnt_untap_during_your_untap_step => {
            let object_pair = pair
                .into_inner()
                .next()
                .expect("static untap restriction names an object");
            match object_pair.as_rule() {
                Rule::source_object => Ok(StaticAbility::SourceDoesntUntapDuringYourUntapStep {
                    source: source_object_from_pair(object_pair)?,
                }),
                Rule::permanent_type | Rule::creature_type => Ok(
                    StaticAbility::EnchantedDoesntUntapDuringItsControllersUntapStep {
                        object: enchanted_object_from_pair(object_pair)?,
                    },
                ),
                _ => Err(ParseError::Internal("static untap restriction object")),
            }
        }
        Rule::static_creatures_with_power_or_greater_dont_untap_during_their_controllers_untap_steps => {
            let power_pair = pair
                .into_inner()
                .next()
                .expect("static untap restriction names power threshold");
            let power = power_pair
                .as_str()
                .parse::<u32>()
                .map_err(|_| ParseError::Internal("static untap restriction power"))?;
            Ok(
                StaticAbility::CreaturesWithPowerOrGreaterDontUntapDuringTheirControllersUntapSteps {
                    power,
                },
            )
        }
        Rule::static_source_cant_block_creatures_with_power_or_greater => {
            let mut inner = pair.into_inner();
            let source_pair = inner
                .next()
                .expect("static block restriction begins with a source object");
            let power_pair = inner
                .next()
                .expect("static block restriction names power threshold");
            let power = power_pair
                .as_str()
                .parse::<u32>()
                .map_err(|_| ParseError::Internal("static block restriction power"))?;
            Ok(StaticAbility::SourceCantBlockCreaturesWithPowerOrGreater {
                source: source_object_from_pair(source_pair)?,
                power,
            })
        }
        Rule::static_named_source_pt_equal_to_count => {
            let mut inner = pair.into_inner();
            let source_pair = inner
                .next()
                .expect("named source P/T count begins with a source name");
            let count_pair = inner
                .next()
                .expect("named source P/T count names counted objects");
            let count = match count_pair.as_rule() {
                Rule::non_creature_type_creatures_you_control => {
                    let excluded_type_pair = count_pair
                        .into_inner()
                        .next()
                        .expect("non-creature count names an excluded creature type");
                    NamedSourcePowerToughnessCount::NonCreatureTypeCreatures {
                        excluded_type: creature_type_from_pair(excluded_type_pair)?,
                    }
                }
                Rule::basic_lands_you_control => {
                    let land_type_pair = count_pair
                        .into_inner()
                        .next()
                        .expect("basic land count names a land type");
                    NamedSourcePowerToughnessCount::BasicLands {
                        land_type: basic_land_type_from_plural_pair(land_type_pair)?,
                    }
                }
                Rule::creatures_named_on_battlefield => {
                    let name_pair = count_pair
                        .into_inner()
                        .next()
                        .expect("named creature count names a card");
                    NamedSourcePowerToughnessCount::CreaturesNamedOnTheBattlefield {
                        name: name_pair.as_str().to_string(),
                    }
                }
                _ => return Err(ParseError::Internal("named source P/T count objects")),
            };
            Ok(StaticAbility::NamedSourcePowerToughnessEachEqualToCount {
                source_name: source_pair.as_str().to_string(),
                count,
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
        Rule::static_basic_lands_are_pt_colored_creatures_still_lands => {
            let mut inner = pair.into_inner();
            let land_type_pair = inner
                .next()
                .expect("basic land creature effect names affected land type");
            let power_pair = inner
                .next()
                .expect("basic land creature effect names power");
            let toughness_pair = inner
                .next()
                .expect("basic land creature effect names toughness");
            let power = power_pair
                .as_str()
                .parse::<u32>()
                .map_err(|_| ParseError::Internal("basic land creature effect power"))?;
            let toughness = toughness_pair
                .as_str()
                .parse::<u32>()
                .map_err(|_| ParseError::Internal("basic land creature effect toughness"))?;
            let color = inner.next().map(color_from_pair).transpose()?;
            Ok(StaticAbility::BasicLandsAreColoredCreaturesStillLands {
                land_type: basic_land_type_from_plural_pair(land_type_pair)?,
                power,
                toughness,
                color,
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
        Rule::remove_target_creature_defending_player_controls_from_combat => {
            Ok(StaticAbility::RemoveTargetCreatureDefendingPlayerControlsFromCombat)
        }
        Rule::creatures_it_was_blocking_become_unblocked => {
            Ok(StaticAbility::CreaturesItWasBlockingBecomeUnblocked)
        }
        Rule::you_may_have_it_block_attacking_creature => {
            Ok(StaticAbility::YouMayHaveItBlockAttackingCreatureOfYourChoice)
        }
        Rule::that_creature_attacks_this_turn_if_able => {
            Ok(StaticAbility::ThatCreatureAttacksThisTurnIfAble)
        }
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

fn activated_ability_with_activation_permission_from_pair(
    pair: Pair<Rule>,
) -> Result<Statement, ParseError> {
    let mut inner = pair.into_inner();
    let ability_pair = inner.next().ok_or(ParseError::Internal(
        "activated ability with permission missing ability",
    ))?;
    let permission_pair = inner.next().ok_or(ParseError::Internal(
        "activated ability with permission missing permission",
    ))?;
    Ok(Statement::ActivatedAbilityWithActivationPermission {
        ability: activated_ability_from_pair(ability_pair)?,
        permission: activation_permission_from_pair(permission_pair)?,
    })
}

fn activation_permission_from_pair(pair: Pair<Rule>) -> Result<ActivationPermission, ParseError> {
    match pair.as_rule() {
        Rule::only_sources_owner_may_activate_this_ability => {
            let source_pair = only_inner(pair, "activation permission missing source")?;
            Ok(ActivationPermission::OnlySourcesOwner {
                source: source_object_from_possessive_pair(source_pair)?,
            })
        }
        _ => Err(ParseError::Internal("activation permission")),
    }
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
                let source_pair = only_inner(child, "sacrifice_source missing source_object")?;
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
            let mana_pair = only_inner(pair, "add_mana missing mana_cost")?;
            Ok(ActivatedEffect::AddMana(mana_cost_from_pair(mana_pair)))
        }
        Rule::add_one_mana_of_any_color => Ok(ActivatedEffect::AddOneManaOfAnyColor),
        Rule::add_mana_of_any_one_color => {
            let amount_pair = only_inner(pair, "add_mana_of_any_one_color missing number")?;
            let amount = number_word_to_u32(amount_pair.as_str())
                .ok_or(ParseError::Internal("number_word"))?;
            Ok(ActivatedEffect::AddManaOfAnyOneColor { amount })
        }
        Rule::tap_target_permanent_choice => {
            let choice_pair = only_inner(
                pair,
                "tap_target_permanent_choice missing target_permanent_choice",
            )?;
            let permanent_types = target_permanent_choice_from_pair(choice_pair)?;
            Ok(ActivatedEffect::TapTargetPermanentChoice { permanent_types })
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
        Rule::untap_target_permanent => {
            let permanent_type_pair =
                only_inner(pair, "untap_target_permanent missing permanent_type")?;
            Ok(ActivatedEffect::UntapTargetPermanent {
                permanent_type: permanent_type_from_pair(permanent_type_pair)?,
            })
        }
        Rule::untap_enchanted_object => {
            let object_pair = only_inner(pair, "untap_enchanted_object missing object")?;
            Ok(ActivatedEffect::UntapEnchanted(enchanted_object_from_pair(
                object_pair,
            )?))
        }
        Rule::regenerate_source => {
            let source_pair = only_inner(pair, "regenerate_source missing source_object")?;
            Ok(ActivatedEffect::Regenerate(source_object_from_pair(
                source_pair,
            )?))
        }
        Rule::colored_target_effect => match colored_target_effect_from_pair(pair)? {
            ColoredTargetEffect::CounterSpell { color } => {
                Ok(ActivatedEffect::CounterTargetColoredSpell { color })
            }
            ColoredTargetEffect::DestroyPermanent { color } => {
                Ok(ActivatedEffect::DestroyTargetColoredPermanent { color })
            }
        },
        Rule::counter_target_colored_spell => {
            let action_pair = only_inner(pair, "counter colored spell missing action")?;
            match colored_target_action_from_pair(action_pair)? {
                ColoredTargetEffect::CounterSpell { color } => {
                    Ok(ActivatedEffect::CounterTargetColoredSpell { color })
                }
                ColoredTargetEffect::DestroyPermanent { .. } => {
                    Err(ParseError::Internal("activated colored target effect"))
                }
            }
        }
        Rule::destroy => activated_destroy_from_pair(pair),
        Rule::look_at_target_players_hand => Ok(ActivatedEffect::LookAtTargetPlayersHand),
        Rule::activated_draw_cards => {
            let count_pair = pair
                .into_inner()
                .next()
                .ok_or(ParseError::Internal("activated draw missing count"))?;
            let count = match count_pair.as_rule() {
                Rule::activated_draw_one_card => CardCount::Number(1),
                Rule::activated_draw_counted_cards => {
                    let draw_count_pair = count_pair
                        .into_inner()
                        .next()
                        .ok_or(ParseError::Internal("activated draw counted missing count"))?;
                    card_count_from_pair(draw_count_pair)?
                }
                _ => return Err(ParseError::Internal("activated draw count")),
            };
            Ok(ActivatedEffect::DrawCards { count })
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
            let power_pair = only_inner(pair, "target creature unblockable missing power")?;
            let power = power_pair
                .as_str()
                .parse::<u32>()
                .map_err(|_| ParseError::Internal("target creature unblockable power"))?;
            Ok(ActivatedEffect::TargetCreatureWithPowerOrLessCantBeBlockedThisTurn { power })
        }
        Rule::target_permanent_gains_keyword_until_eot => {
            target_permanent_gains_keyword_until_eot_from_pair(pair)
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
        Rule::activated_source_gains_keyword_until_eot => {
            let mut inner = pair.into_inner();
            let source_pair = inner.next().ok_or(ParseError::Internal(
                "activated source gains missing source",
            ))?;
            let keyword_pair = inner.next().ok_or(ParseError::Internal(
                "activated source gains missing keyword",
            ))?;
            Ok(ActivatedEffect::SourceGainsKeywordUntilEndOfTurn {
                source: source_object_from_pair(source_pair)?,
                keyword: keyword_from_inner_pair(keyword_pair)?,
            })
        }
        Rule::activated_source_becomes_pt_creature_until_end_of_combat => {
            let mut inner = pair.into_inner();
            let source_pair = inner.next().ok_or(ParseError::Internal(
                "activated source becomes missing source",
            ))?;
            let power_pair = inner.next().ok_or(ParseError::Internal(
                "activated source becomes missing power",
            ))?;
            let toughness_pair = inner.next().ok_or(ParseError::Internal(
                "activated source becomes missing toughness",
            ))?;
            let creature_type_pair = inner.next().ok_or(ParseError::Internal(
                "activated source becomes missing creature_type",
            ))?;
            let permanent_types = inner
                .map(permanent_type_from_pair)
                .collect::<Result<Vec<_>, _>>()?;
            if permanent_types.is_empty() {
                return Err(ParseError::Internal(
                    "activated source becomes missing permanent types",
                ));
            }
            Ok(ActivatedEffect::SourceBecomesCreatureUntilEndOfCombat {
                source: source_object_from_pair(source_pair)?,
                power: power_pair
                    .as_str()
                    .parse::<u32>()
                    .map_err(|_| ParseError::Internal("activated source becomes power"))?,
                toughness: toughness_pair
                    .as_str()
                    .parse::<u32>()
                    .map_err(|_| ParseError::Internal("activated source becomes toughness"))?,
                creature_type: creature_type_from_pair(creature_type_pair)?,
                permanent_types,
            })
        }
        Rule::activated_direct_damage_effect
        | Rule::next_damage_event_effect
        | Rule::next_damage_redirection_effect
        | Rule::activated_damage_prevention_effect => Ok(ActivatedEffect::DamageEffect(
            activated_damage_effect_from_pair(pair)?,
        )),
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
        Rule::choose_creature_card_in_hand_payable_by_mana_spent_on_variable => {
            let variable_pair = only_inner(pair, "choose creature card missing variable")?;
            Ok(
                ActivatedEffect::ChooseCreatureCardInHandPayableByManaSpentOnVariable {
                    variable: variable_from_mana_symbol_pair(variable_pair)?,
                },
            )
        }
        Rule::choose_target_non_creature_type_creature_active_player_controlled_continuously => {
            let excluded_pair = only_inner(
                pair,
                "choose target non creature type missing creature_type",
            )?;
            Ok(
                ActivatedEffect::ChooseTargetNonCreatureTypeCreatureActivePlayerControlledContinuouslySinceBeginningOfTurn {
                    excluded_type: creature_type_from_pair(excluded_pair)?,
                },
            )
        }
        Rule::target_permanent_becomes_basic_land_type_until_source_leaves => {
            let mut inner = pair.into_inner();
            let permanent_type_pair = inner.next().ok_or(ParseError::Internal(
                "target becomes land type missing permanent_type",
            ))?;
            let land_type_pair = inner.next().ok_or(ParseError::Internal(
                "target becomes land type missing basic_land_type",
            ))?;
            let source_pair = inner.next().ok_or(ParseError::Internal(
                "target becomes land type missing source_object",
            ))?;
            Ok(
                ActivatedEffect::TargetPermanentBecomesBasicLandTypeUntilSourceLeavesBattlefield {
                    permanent_type: permanent_type_from_pair(permanent_type_pair)?,
                    land_type: basic_land_type_from_pair(land_type_pair)?,
                    source: source_object_from_pair(source_pair)?,
                },
            )
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

fn activated_destroy_from_pair(pair: Pair<Rule>) -> Result<ActivatedEffect, ParseError> {
    let target_pair = only_inner(pair, "destroy missing target")?;
    Ok(ActivatedEffect::destroy(destroy_target_from_pair(
        target_pair,
    )?))
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
            let source_pair = only_inner(pair, "physical turns-over missing source_object")?;
            Ok(PhysicalAction::IfSourceTurnsOverCompletelyAtLeastOnceDuringFlipDestroyAllNontokenPermanentsItTouches {
                source: source_object_from_pair(source_pair)?,
            })
        }
        Rule::then_destroy_source => {
            let source_pair = only_inner(pair, "physical then-destroy missing source_object")?;
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
            let amount_pair = only_inner(
                pair,
                "number-of-cards expression missing subtraction amount",
            )?;
            let amount = amount_pair
                .as_str()
                .parse::<u32>()
                .map_err(|_| ParseError::Internal("number-of-cards subtraction amount"))?;
            Ok(ValueExpression::NumberOfCardsInTheirHandMinus { amount })
        }
        Rule::number_of_status_permanents_they_controlled_at_beginning_of_this_turn => {
            let mut inner = pair.into_inner();
            let status_pair = inner
                .next()
                .expect("status-permanent count expression names a status");
            let permanent_type_pair = inner
                .next()
                .expect("status-permanent count expression names a permanent type");
            Ok(
                ValueExpression::NumberOfStatusPermanentsTheyControlledAtBeginningOfThisTurn {
                    status: object_status_from_pair(status_pair)?,
                    permanent_type: permanent_type_from_plural_pair(permanent_type_pair)?,
                },
            )
        }
        Rule::amount_of_mana_that_player_paid_this_way => {
            Ok(ValueExpression::AmountOfManaThatPlayerPaidThisWay)
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

fn basic_land_type_reference_from_pair(
    pair: Pair<Rule>,
) -> Result<BasicLandTypeReference, ParseError> {
    match pair.as_rule() {
        Rule::specific_basic_land_type => {
            let land_type_pair =
                only_inner(pair, "specific_basic_land_type missing basic_land_type")?;
            Ok(BasicLandTypeReference::Specific(basic_land_type_from_pair(
                land_type_pair,
            )?))
        }
        Rule::chosen_basic_land_type => Ok(BasicLandTypeReference::ChosenType),
        _ => Err(ParseError::Internal("basic_land_type_reference")),
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

fn permanent_controller_from_pair(pair: Pair<Rule>) -> Result<PermanentController, ParseError> {
    if pair.as_rule() != Rule::permanent_controller {
        return Err(ParseError::Internal("permanent_controller"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "you control" => Ok(PermanentController::You),
        "an opponent controls" | "a opponent controls" => Ok(PermanentController::Opponent),
        _ => Err(ParseError::Internal("permanent_controller variant")),
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
        Rule::source_isnt_attacking | Rule::source_is_attacking => {
            let is_attacking = pair.as_rule() == Rule::source_is_attacking;
            let source_name = pair
                .into_inner()
                .next()
                .expect("attacking condition begins with a source name")
                .as_str()
                .to_string();
            Ok(Condition::SourceIsAttacking {
                source_name,
                is_attacking,
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
        Rule::source_pt_equal_to_basic_lands_controlled => {
            let mut inner = pair.into_inner();
            let land_type_pair = inner
                .next()
                .expect("basic land count effect names a land type");
            let controller_pair = inner
                .next()
                .expect("basic land count effect names a controller");
            Ok(
                ContinuousEffect::SourcePowerToughnessEachEqualToBasicLandsControlled {
                    land_type: basic_land_type_from_plural_pair(land_type_pair)?,
                    controller: land_count_controller_from_pair(controller_pair)?,
                },
            )
        }
        _ => Err(ParseError::Internal("continuous_effect")),
    }
}

fn land_count_controller_from_pair(pair: Pair<Rule>) -> Result<LandCountController, ParseError> {
    if pair.as_rule() != Rule::land_count_controller {
        return Err(ParseError::Internal("land_count_controller"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "you control" => Ok(LandCountController::You),
        "defending player controls" => Ok(LandCountController::DefendingPlayer),
        _ => Err(ParseError::Internal("land_count_controller variant")),
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

fn permanent_type_plural_list_from_pair(
    pair: Pair<Rule>,
) -> Result<Vec<PermanentType>, ParseError> {
    if pair.as_rule() != Rule::permanent_type_plural_list {
        return Err(ParseError::Internal("permanent_type_plural_list"));
    }
    pair.into_inner()
        .map(permanent_type_from_plural_pair)
        .collect()
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

fn spell_type_from_pair(pair: Pair<Rule>) -> Result<SpellType, ParseError> {
    if pair.as_rule() != Rule::spell_type {
        return Err(ParseError::Internal("spell_type"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "instant" => Ok(SpellType::Instant),
        "sorcery" => Ok(SpellType::Sorcery),
        _ => Err(ParseError::Internal("spell_type variant")),
    }
}

fn creature_status_from_pair(pair: Pair<Rule>) -> Result<CreatureStatus, ParseError> {
    if pair.as_rule() != Rule::creature_status {
        return Err(ParseError::Internal("creature_status"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "attacking" => Ok(CreatureStatus::Attacking),
        "tapped" => Ok(CreatureStatus::Tapped),
        "untapped" => Ok(CreatureStatus::Untapped),
        _ => Err(ParseError::Internal("creature_status variant")),
    }
}

fn object_status_from_pair(pair: Pair<Rule>) -> Result<ObjectStatus, ParseError> {
    if pair.as_rule() != Rule::object_status {
        return Err(ParseError::Internal("object_status"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "tapped" => Ok(ObjectStatus::Tapped),
        "untapped" => Ok(ObjectStatus::Untapped),
        _ => Err(ParseError::Internal("object_status variant")),
    }
}

fn creature_type_from_pair(pair: Pair<Rule>) -> Result<CreatureType, ParseError> {
    if pair.as_rule() != Rule::creature_type {
        return Err(ParseError::Internal("creature_type"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "goblin" => Ok(CreatureType::Goblin),
        "golem" => Ok(CreatureType::Golem),
        "merfolk" => Ok(CreatureType::Merfolk),
        "wall" => Ok(CreatureType::Wall),
        _ => Err(ParseError::Internal("creature_type variant")),
    }
}

fn creature_type_from_plural_pair(pair: Pair<Rule>) -> Result<CreatureType, ParseError> {
    if pair.as_rule() != Rule::creature_type_plural {
        return Err(ParseError::Internal("creature_type_plural"));
    }
    match pair.as_str().to_ascii_lowercase().as_str() {
        "goblins" => Ok(CreatureType::Goblin),
        "golems" => Ok(CreatureType::Golem),
        "merfolk" => Ok(CreatureType::Merfolk),
        "walls" => Ok(CreatureType::Wall),
        _ => Err(ParseError::Internal("creature_type_plural variant")),
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
        Rule::variable_name => ManaSymbol::Variable(
            variable_from_str(body.as_str()).expect("variable_name restricts to known variables"),
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
        _ => unreachable!("mana_body is silent and only contains generic|color|variable_name"),
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
