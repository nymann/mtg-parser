use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use crate::ast::{
    Condition, ContinuousEffect, EnchantObject, InterveningIf, Keyword, ManaCost, ManaSymbol,
    PermanentType, PtModifier, Sign, SignedNumber, Statement, StaticAbility, TriggerEffect,
    TriggerEvent, TriggeredAbility, Zone,
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
        Rule::destroy => Ok(Statement::DestroyTargetCreature),
        Rule::draw_cards => draw_cards_from_pair(pair),
        Rule::keyword_ability => Ok(Statement::Keyword(keyword_from_pair(pair)?)),
        Rule::static_as_long_as | Rule::static_enchanted_gets => {
            Ok(Statement::StaticAbility(static_ability_from_pair(pair)?))
        }
        Rule::triggered_ability => Ok(Statement::TriggeredAbility(
            triggered_ability_from_pair(pair)?,
        )),
        _ => Err(ParseError::Internal("statement")),
    }
}

fn draw_cards_from_pair(pair: Pair<Rule>) -> Result<Statement, ParseError> {
    let word = pair
        .into_inner()
        .next()
        .expect("draw_cards always contains a number_word");
    let count = number_word_to_u32(word.as_str()).ok_or(ParseError::Internal("number_word"))?;
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
            Rule::its_on_the_battlefield => {
                intervening_if = Some(InterveningIf::ItsOnTheBattlefield);
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

fn loses_and_gains_keyword_from_pair(pair: Pair<Rule>) -> Result<TriggerEffect, ParseError> {
    let mut inner = pair.into_inner();
    let loses_pair = inner
        .next()
        .ok_or(ParseError::Internal("loses_and_gains missing loses keyword"))?;
    let gains_pair = inner
        .next()
        .ok_or(ParseError::Internal("loses_and_gains missing gains keyword"))?;
    Ok(TriggerEffect::LosesAndGainsKeyword {
        loses: keyword_from_inner_pair(loses_pair)?,
        gains: keyword_from_inner_pair(gains_pair)?,
    })
}

fn keyword_from_inner_pair(pair: Pair<Rule>) -> Result<Keyword, ParseError> {
    match pair.as_rule() {
        Rule::flying => Ok(Keyword::Flying),
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

fn enchant_object_from_pair(pair: Pair<Rule>) -> Result<EnchantObject, ParseError> {
    match pair.as_rule() {
        Rule::enchant_permanent => {
            let pt = pair
                .into_inner()
                .next()
                .expect("enchant_permanent wraps a permanent_type");
            Ok(EnchantObject::Permanent(permanent_type_from_pair(pt)?))
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
        _ => Err(ParseError::Internal("static_ability variant")),
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
