use std::fmt::Write;

use crate::ast::{ManaCost, ManaSymbol, Statement};

pub fn unparse(statement: &Statement) -> String {
    let mut out = String::new();
    write_statement(&mut out, statement);
    out
}

fn write_statement(out: &mut String, statement: &Statement) {
    match statement {
        Statement::ManaCost(mc) => write_mana_cost(out, mc),
        Statement::DestroyTargetCreature => out.push_str("Destroy target creature."),
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
