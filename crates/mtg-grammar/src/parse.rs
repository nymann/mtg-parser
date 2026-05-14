use pest::iterators::Pair;
use pest::Parser;
use pest_derive::Parser;

use crate::ast::{
    Condition, ContinuousEffect, Keyword, ManaCost, ManaSymbol, PermanentType, Statement,
    StaticAbility,
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
        Rule::static_ability => Ok(Statement::StaticAbility(static_ability_from_pair(pair)?)),
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
            let pt = inner
                .into_inner()
                .next()
                .expect("enchant always contains a permanent_type");
            Ok(Keyword::Enchant(permanent_type_from_pair(pt)?))
        }
        _ => Err(ParseError::Internal("keyword")),
    }
}

fn static_ability_from_pair(pair: Pair<Rule>) -> Result<StaticAbility, ParseError> {
    let mut inner = pair.into_inner();
    let cond_pair = inner
        .next()
        .expect("static_ability begins with a condition");
    let effect_pair = inner
        .next()
        .expect("static_ability has an effect after the condition");
    Ok(StaticAbility {
        condition: condition_from_pair(cond_pair)?,
        effect: continuous_effect_from_pair(effect_pair)?,
    })
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
