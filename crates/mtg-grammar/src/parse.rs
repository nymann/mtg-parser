use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use crate::ast::{
    ActivatedAbility, ActivatedCost, ActivatedEffect, BalanceSameWayAction, BasicLandType,
    CardCount, CastRestriction, Color, Condition, ContinuousEffect, CreatureType, EnchantObject,
    EnchantedObject, InterveningIf, Keyword, ManaCost, ManaSymbol, MixedPtModifier, ModalMode,
    PermanentType, PtModifier, Rounding, Sign, SignedNumber, SignedPtComponent, SignedVariable,
    SourceObject, Statement, StaticAbility, Step, TriggerEffect, TriggerEvent, TriggeredAbility,
    ValueExpression, Variable, VariableDefinition, VariablePtModifier, Zone,
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
        Rule::destroy => Ok(Statement::DestroyTargetCreature),
        Rule::destroy_all => destroy_all_from_pair(pair),
        Rule::draw_cards => draw_cards_from_pair(pair),
        Rule::target_permanent_gains_keyword_and_gets_eot => {
            target_permanent_gains_keyword_and_gets_eot_from_pair(pair)
        }
        Rule::each_player_equalizes_controlled_permanents => {
            each_player_equalizes_controlled_permanents_from_pair(pair)
        }
        Rule::players_do_actions_the_same_way => players_do_actions_the_same_way_from_pair(pair),
        Rule::as_this_permanent_enters_choose_opponent => {
            as_this_permanent_enters_choose_opponent_from_pair(pair)
        }
        Rule::keyword_ability => Ok(Statement::Keyword(keyword_from_pair(pair)?)),
        Rule::static_as_long_as
        | Rule::static_colored_permanents_get
        | Rule::static_enchanted_gets_with_definitions
        | Rule::static_enchanted_gets
        | Rule::static_enchanted_has_keyword
        | Rule::static_enchanted_can_attack_as_though
        | Rule::static_source_doesnt_untap_during_your_untap_step
        | Rule::target_creature_defending_player_controls_can_block_any_number
        | Rule::it_blocks_each_attacking_creature_if_able
        | Rule::static_effect_doesnt_remove_this_aura => {
            Ok(Statement::StaticAbility(static_ability_from_pair(pair)?))
        }
        Rule::activated_ability => Ok(Statement::ActivatedAbility(activated_ability_from_pair(
            pair,
        )?)),
        Rule::triggered_ability => Ok(Statement::TriggeredAbility(triggered_ability_from_pair(
            pair,
        )?)),
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
        _ => Err(ParseError::Internal("step variant")),
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
    let SourceObject::This(permanent_type) = source_object_from_pair(source_pair)?;
    Ok(Statement::AsThisPermanentEntersChooseOpponent { permanent_type })
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

fn draw_cards_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let count_pair = pair
        .into_inner()
        .next()
        .expect("draw_cards always contains a draw_count");
    let count = match count_pair.as_rule() {
        Rule::number_word => {
            let count = number_word_to_u32(count_pair.as_str())
                .ok_or(ParseError::Internal("number_word"))?;
            CardCount::Number(count)
        }
        Rule::variable_name => CardCount::Variable(variable_from_str(count_pair.as_str())?),
        _ => return Err(ParseError::Internal("draw_count")),
    };
    Ok(Statement::TargetPlayerDrawsCards { count })
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
    match inner.as_rule() {
        Rule::flying => Ok(Keyword::Flying),
        Rule::first_strike => Ok(Keyword::FirstStrike),
        Rule::defender => Ok(Keyword::Defender),
        Rule::banding => Ok(Keyword::Banding),
        Rule::trample => Ok(Keyword::Trample),
        Rule::mountainwalk => Ok(Keyword::Mountainwalk),
        Rule::swampwalk => Ok(Keyword::Swampwalk),
        Rule::protection => {
            let color = inner
                .into_inner()
                .next()
                .expect("protection always names a color");
            Ok(Keyword::Protection(color_from_pair(color)?))
        }
        Rule::enchant => {
            let object = inner
                .into_inner()
                .next()
                .expect("enchant always contains an enchant_object alternative");
            Ok(Keyword::Enchant(enchant_object_from_pair(object)?))
        }
        _ => Err(ParseError::Internal("keyword")),
    }
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
            Rule::beginning_of_the_next_end_step => {
                event = Some(TriggerEvent::BeginningOfTheNextEndStep);
            }
            Rule::beginning_of_chosen_players_upkeep => {
                event = Some(TriggerEvent::BeginningOfChosenPlayersUpkeep);
            }
            Rule::its_on_the_battlefield => {
                intervening_if = Some(InterveningIf::ItsOnTheBattlefield);
            }
            Rule::destroy_that_creature_if_it_attacked_this_turn => {
                effects.push(TriggerEffect::DestroyThatCreatureIfItAttackedThisTurn);
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
            Rule::source_deals_damage_to_that_permanents_controller => {
                effects.push(source_deals_damage_to_that_permanents_controller_from_pair(
                    child,
                )?);
            }
            Rule::source_deals_variable_damage_to_that_player => {
                effects.push(source_deals_variable_damage_to_that_player_from_pair(
                    child,
                )?);
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

fn source_object_from_pair(pair: Pair<Rule>) -> Result<SourceObject, ParseError> {
    if pair.as_rule() != Rule::source_object {
        return Err(ParseError::Internal("source_object"));
    }
    let pt = pair
        .into_inner()
        .next()
        .ok_or(ParseError::Internal("source_object missing permanent_type"))?;
    Ok(SourceObject::This(permanent_type_from_pair(pt)?))
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
        Rule::target_creature_defending_player_controls_can_block_any_number => Ok(
            StaticAbility::TargetCreatureDefendingPlayerControlsCanBlockAnyNumberOfCreaturesThisTurn,
        ),
        Rule::it_blocks_each_attacking_creature_if_able => {
            Ok(StaticAbility::ItBlocksEachAttackingCreatureThisTurnIfAble)
        }
        _ => Err(ParseError::Internal("static_ability variant")),
    }
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
            Rule::mana_symbol => Ok(ActivatedCost::Mana(ManaCost {
                symbols: vec![mana_symbol_from_pair(child)],
            })),
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
        _ => Err(ParseError::Internal("activated_effect")),
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
        "forests" => Ok(BasicLandType::Forest),
        _ => Err(ParseError::Internal("basic_land_type_plural variant")),
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
