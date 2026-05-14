use std::fmt::Write;

use crate::ast::{
    Condition, ContinuousEffect, Keyword, ManaCost, ManaSymbol, PermanentType, Statement,
    StaticAbility,
};

pub fn unparse(statement: &Statement) -> String {
    let mut out = String::new();
    write_statement(&mut out, statement);
    out
}

fn write_statement(out: &mut String, statement: &Statement) {
    match statement {
        Statement::ManaCost(mc) => write_mana_cost(out, mc),
        Statement::DestroyTargetCreature => out.push_str("Destroy target creature."),
        Statement::Keyword(kw) => write_keyword(out, *kw),
        Statement::TargetPlayerDrawsCards { count } => {
            write!(out, "Target player draws {} cards.", u32_to_number_word(*count))
                .expect("write to String never fails");
        }
        Statement::StaticAbility(sa) => write_static_ability(out, sa),
        Statement::Compound(stmts) => {
            for (i, s) in stmts.iter().enumerate() {
                if i > 0 {
                    out.push('\n');
                }
                write_statement(out, s);
            }
        }
    }
}

fn u32_to_number_word(n: u32) -> &'static str {
    match n {
        1 => "one",
        2 => "two",
        3 => "three",
        4 => "four",
        5 => "five",
        6 => "six",
        7 => "seven",
        8 => "eight",
        9 => "nine",
        10 => "ten",
        _ => panic!("u32_to_number_word: {n} outside supported range 1..=10"),
    }
}

fn write_keyword(out: &mut String, kw: Keyword) {
    match kw {
        Keyword::Flying => out.push_str("Flying"),
        Keyword::Enchant(pt) => {
            out.push_str("Enchant ");
            out.push_str(permanent_type_name(pt));
        }
    }
}

fn write_mana_cost(out: &mut String, cost: &ManaCost) {
    for sym in &cost.symbols {
        write_mana_symbol(out, *sym);
    }
}

fn write_mana_symbol(out: &mut String, sym: ManaSymbol) {
    match sym {
        ManaSymbol::Generic(n) => write!(out, "{{{n}}}").expect("write to String never fails"),
        ManaSymbol::White => out.push_str("{W}"),
        ManaSymbol::Blue => out.push_str("{U}"),
        ManaSymbol::Black => out.push_str("{B}"),
        ManaSymbol::Red => out.push_str("{R}"),
        ManaSymbol::Green => out.push_str("{G}"),
        ManaSymbol::Colorless => out.push_str("{C}"),
    }
}

fn write_static_ability(out: &mut String, sa: &StaticAbility) {
    out.push_str("As long as ");
    write_condition(out, &sa.condition);
    out.push_str(", ");
    write_continuous_effect(out, &sa.effect);
    out.push('.');
}

fn write_condition(out: &mut String, cond: &Condition) {
    match cond {
        Condition::EnchantedIsNot {
            permanent_type,
            negated_type,
        } => {
            out.push_str("enchanted ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" isn't ");
            out.push_str(indefinite_article(*negated_type));
            out.push(' ');
            out.push_str(permanent_type_name(*negated_type));
        }
    }
}

fn write_continuous_effect(out: &mut String, eff: &ContinuousEffect) {
    match eff {
        ContinuousEffect::BecomesWithPtFromManaValue { types } => {
            out.push_str("it's");
            if let Some(first) = types.first() {
                out.push(' ');
                out.push_str(indefinite_article(*first));
            }
            for t in types {
                out.push(' ');
                out.push_str(permanent_type_name(*t));
            }
            out.push_str(" with power and toughness each equal to its mana value");
        }
    }
}

fn permanent_type_name(pt: PermanentType) -> &'static str {
    match pt {
        PermanentType::Artifact => "artifact",
        PermanentType::Creature => "creature",
        PermanentType::Enchantment => "enchantment",
        PermanentType::Land => "land",
        PermanentType::Planeswalker => "planeswalker",
    }
}

fn indefinite_article(pt: PermanentType) -> &'static str {
    match pt {
        PermanentType::Artifact | PermanentType::Enchantment => "an",
        PermanentType::Creature | PermanentType::Land | PermanentType::Planeswalker => "a",
    }
}
