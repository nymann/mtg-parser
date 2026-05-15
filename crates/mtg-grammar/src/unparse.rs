use std::fmt::Write;

use crate::ast::{
    ActivatedAbility, ActivatedCost, ActivatedEffect, BalanceSameWayAction, BasicLandType,
    CastRestriction, Color, Condition, ContinuousEffect, CreatureType, EnchantObject,
    EnchantedObject, InterveningIf, Keyword, ManaCost, ManaSymbol, MixedPtModifier, PermanentType,
    PtModifier, Rounding, Sign, SignedNumber, SignedPtComponent, SignedVariable, SourceObject,
    Statement, StaticAbility, Step, TriggerEffect, TriggerEvent, TriggeredAbility, ValueExpression,
    Variable, VariableDefinition, VariablePtModifier, Zone,
};

pub fn unparse(statement: &Statement) -> String {
    let mut out = String::new();
    write_statement(&mut out, statement);
    out
}

fn write_statement(out: &mut String, statement: &Statement) {
    match statement {
        Statement::ManaCost(mc) => write_mana_cost(out, mc),
        Statement::CastRestriction(restriction) => write_cast_restriction(out, *restriction),
        Statement::DestroyTargetCreature => out.push_str("Destroy target creature."),
        Statement::DestroyAll { permanent_type } => {
            out.push_str("Destroy all ");
            out.push_str(permanent_type_plural_name(*permanent_type));
            out.push('.');
        }
        Statement::Keyword(kw) => write_keyword(out, *kw),
        Statement::TargetPlayerDrawsCards { count } => {
            write!(
                out,
                "Target player draws {} cards.",
                u32_to_number_word(*count)
            )
            .expect("write to String never fails");
        }
        Statement::TargetPermanentGainsKeywordAndGetsUntilEndOfTurn {
            permanent_type,
            keyword,
            modifier,
            definitions,
        } => {
            out.push_str("Target ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" gains ");
            write_keyword_lowercase(out, *keyword);
            out.push_str(" and gets ");
            write_mixed_pt_modifier(out, *modifier);
            out.push_str(" until end of turn, where ");
            write_variable_definitions(out, definitions);
            out.push('.');
        }
        Statement::EachPlayerEqualizesControlledPermanents { permanent_type } => {
            out.push_str("Each player chooses a number of ");
            out.push_str(permanent_type_plural_name(*permanent_type));
            out.push_str(" they control equal to the number of ");
            out.push_str(permanent_type_plural_name(*permanent_type));
            out.push_str(
                " controlled by the player who controls the fewest, then sacrifices the rest.",
            );
        }
        Statement::PlayersDoActionsTheSameWay { actions } => {
            out.push_str("Players ");
            write_same_way_actions(out, actions);
            out.push_str(" the same way.");
        }
        Statement::StaticAbility(sa) => write_static_ability(out, sa),
        Statement::ActivatedAbility(aa) => write_activated_ability(out, aa),
        Statement::TriggeredAbility(ta) => write_triggered_ability(out, ta),
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

fn write_cast_restriction(out: &mut String, restriction: CastRestriction) {
    out.push_str("Cast this spell only ");
    match restriction {
        CastRestriction::BeforeStep { step } => {
            out.push_str("before the ");
            out.push_str(step_name(step));
            out.push_str(" step.");
        }
    }
}

fn write_same_way_actions(out: &mut String, actions: &[BalanceSameWayAction]) {
    for (i, action) in actions.iter().enumerate() {
        if i > 0 {
            if i + 1 == actions.len() {
                out.push_str(" and ");
            } else {
                out.push_str(", ");
            }
        }
        write_same_way_action(out, *action);
    }
}

fn write_same_way_action(out: &mut String, action: BalanceSameWayAction) {
    match action {
        BalanceSameWayAction::DiscardCards => out.push_str("discard cards"),
        BalanceSameWayAction::SacrificePermanents { permanent_type } => {
            out.push_str("sacrifice ");
            out.push_str(permanent_type_plural_name(permanent_type));
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
        Keyword::Defender => out.push_str("Defender"),
        Keyword::Banding => out.push_str("Banding"),
        Keyword::Trample => out.push_str("Trample"),
        Keyword::Enchant(object) => {
            out.push_str("Enchant ");
            write_enchant_object(out, object);
        }
    }
}

fn write_enchant_object(out: &mut String, object: EnchantObject) {
    match object {
        EnchantObject::Permanent(pt) => out.push_str(permanent_type_name(pt)),
        EnchantObject::CreatureType(ct) => out.push_str(creature_type_name(ct)),
        EnchantObject::CardInZone { card_type, zone } => {
            out.push_str(permanent_type_name(card_type));
            out.push_str(" card in ");
            out.push_str(zone_article(zone));
            out.push(' ');
            out.push_str(zone_name(zone));
        }
        EnchantObject::PutOntoBattlefieldByThisAura { card_type } => {
            out.push_str(permanent_type_name(card_type));
            out.push_str(" put onto the battlefield with this Aura");
        }
    }
}

fn zone_name(zone: Zone) -> &'static str {
    match zone {
        Zone::Graveyard => "graveyard",
    }
}

fn zone_article(zone: Zone) -> &'static str {
    match zone {
        Zone::Graveyard => "a",
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

fn write_activated_ability(out: &mut String, aa: &ActivatedAbility) {
    for cost in &aa.costs {
        write_activated_cost(out, cost);
    }
    out.push_str(": ");
    write_activated_effect(out, &aa.effect);
}

fn write_activated_cost(out: &mut String, cost: &ActivatedCost) {
    match cost {
        ActivatedCost::Mana(mana) => write_mana_cost(out, mana),
        ActivatedCost::Tap => out.push_str("{T}"),
    }
}

fn write_activated_effect(out: &mut String, effect: &ActivatedEffect) {
    match effect {
        ActivatedEffect::AddMana(mana) => {
            out.push_str("Add ");
            write_mana_cost(out, mana);
            out.push('.');
        }
        ActivatedEffect::AddOneManaOfAnyColor => {
            out.push_str("Add one mana of any color.");
        }
        ActivatedEffect::Untap(source) => {
            out.push_str("Untap ");
            write_source_object(out, *source);
            out.push('.');
        }
    }
}

fn write_static_ability(out: &mut String, sa: &StaticAbility) {
    match sa {
        StaticAbility::Conditional { condition, effect } => {
            out.push_str("As long as ");
            write_condition(out, condition);
            out.push_str(", ");
            write_continuous_effect(out, effect);
            out.push('.');
        }
        StaticAbility::EnchantedGets {
            permanent_type,
            modifier,
        } => {
            out.push_str("Enchanted ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" gets ");
            write_pt_modifier(out, *modifier);
            out.push('.');
        }
        StaticAbility::ColoredPermanentsGet {
            color,
            permanent_type,
            modifier,
        } => {
            out.push_str(color_name_capitalized(*color));
            out.push(' ');
            out.push_str(permanent_type_plural_name(*permanent_type));
            out.push_str(" get ");
            write_pt_modifier(out, *modifier);
            out.push('.');
        }
        StaticAbility::EnchantedGetsWithDefinitions {
            permanent_type,
            modifier,
            definitions,
        } => {
            out.push_str("Enchanted ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" gets ");
            write_variable_pt_modifier(out, *modifier);
            out.push_str(", where ");
            write_variable_definitions(out, definitions);
            out.push('.');
        }
        StaticAbility::EnchantedCanAttackAsThoughItDidntHave { object, keyword } => {
            out.push_str("Enchanted ");
            write_enchanted_object(out, *object);
            out.push_str(" can attack as though it didn't have ");
            write_keyword_lowercase(out, *keyword);
            out.push('.');
        }
        StaticAbility::SourceDoesntUntapDuringYourUntapStep { source } => {
            write_source_object_capitalized(out, *source);
            out.push_str(" doesn't untap during your untap step.");
        }
    }
}

fn write_triggered_ability(out: &mut String, ta: &TriggeredAbility) {
    out.push_str(match ta.event {
        TriggerEvent::PermanentEnters { .. } => "Whenever ",
        TriggerEvent::BeginningOfTheNextEndStep => "At ",
        TriggerEvent::ThisAuraEnters | TriggerEvent::ThisAuraLeavesTheBattlefield => "When ",
    });
    write_trigger_event(out, ta.event);
    out.push_str(", ");
    if let Some(iif) = ta.intervening_if {
        out.push_str("if ");
        write_intervening_if(out, iif);
        out.push_str(", ");
    }
    for (i, eff) in ta.effects.iter().enumerate() {
        if i > 0 {
            out.push(' ');
        }
        write_trigger_effect(out, eff);
    }
}

fn write_trigger_event(out: &mut String, ev: TriggerEvent) {
    match ev {
        TriggerEvent::ThisAuraEnters => out.push_str("this Aura enters"),
        TriggerEvent::ThisAuraLeavesTheBattlefield => {
            out.push_str("this Aura leaves the battlefield");
        }
        TriggerEvent::PermanentEnters { permanent_type } => {
            out.push_str(indefinite_article(permanent_type));
            out.push(' ');
            out.push_str(permanent_type_name(permanent_type));
            out.push_str(" enters");
        }
        TriggerEvent::BeginningOfTheNextEndStep => {
            out.push_str("the beginning of the next end step");
        }
    }
}

fn write_intervening_if(out: &mut String, iif: InterveningIf) {
    match iif {
        InterveningIf::ItsOnTheBattlefield => out.push_str("it's on the battlefield"),
    }
}

fn write_trigger_effect(out: &mut String, eff: &TriggerEffect) {
    match eff {
        TriggerEffect::DestroyThatCreatureIfItAttackedThisTurn => {
            out.push_str("destroy that creature if it attacked this turn.");
        }
        TriggerEffect::ThatCreaturesControllerSacrificesIt => {
            out.push_str("that creature's controller sacrifices it.");
        }
        TriggerEffect::SourceDealsDamageToThatPermanentController {
            source,
            amount,
            recipient,
        } => {
            write_source_object(out, *source);
            write!(out, " deals {amount} damage to that ").expect("write to String never fails");
            out.push_str(permanent_type_name(*recipient));
            out.push_str("'s controller.");
        }
        TriggerEffect::LosesAndGainsKeyword { loses, gains } => {
            out.push_str("it loses \"");
            write_keyword_lowercase(out, *loses);
            out.push_str("\" and gains \"");
            write_keyword_lowercase(out, *gains);
            out.push_str(".\"");
        }
        TriggerEffect::ReturnEnchantedCardAndAttach { card_type } => {
            out.push_str("Return enchanted ");
            out.push_str(permanent_type_name(*card_type));
            out.push_str(" card to the battlefield under your control and attach this Aura to it.");
        }
    }
}

fn write_source_object(out: &mut String, source: SourceObject) {
    match source {
        SourceObject::This(pt) => {
            out.push_str("this ");
            out.push_str(permanent_type_name(pt));
        }
    }
}

fn write_source_object_capitalized(out: &mut String, source: SourceObject) {
    match source {
        SourceObject::This(pt) => {
            out.push_str("This ");
            out.push_str(permanent_type_name(pt));
        }
    }
}

/// `write_keyword` capitalizes the first letter ("Flying", "Enchant ...").
/// Inside the quoted text of a loses-and-gains effect the quoted
/// keyword is printed lowercase, which is what we emit here.
fn write_keyword_lowercase(out: &mut String, kw: Keyword) {
    match kw {
        Keyword::Flying => out.push_str("flying"),
        Keyword::Defender => out.push_str("defender"),
        Keyword::Banding => out.push_str("banding"),
        Keyword::Trample => out.push_str("trample"),
        Keyword::Enchant(object) => {
            out.push_str("enchant ");
            write_enchant_object(out, object);
        }
    }
}

fn write_enchanted_object(out: &mut String, object: EnchantedObject) {
    match object {
        EnchantedObject::Permanent(pt) => out.push_str(permanent_type_name(pt)),
        EnchantedObject::CreatureType(ct) => out.push_str(creature_type_name(ct)),
    }
}

fn write_mixed_pt_modifier(out: &mut String, m: MixedPtModifier) {
    write_signed_pt_component(out, m.power);
    out.push('/');
    write_signed_pt_component(out, m.toughness);
}

fn write_signed_pt_component(out: &mut String, component: SignedPtComponent) {
    match component {
        SignedPtComponent::Number(n) => write_signed_number(out, n),
        SignedPtComponent::Variable(v) => write_signed_variable(out, v),
    }
}

fn write_pt_modifier(out: &mut String, m: PtModifier) {
    write_signed_number(out, m.power);
    out.push('/');
    write_signed_number(out, m.toughness);
}

fn write_signed_number(out: &mut String, n: SignedNumber) {
    out.push(match n.sign {
        Sign::Plus => '+',
        Sign::Minus => '-',
    });
    write!(out, "{}", n.magnitude).expect("write to String never fails");
}

fn write_variable_pt_modifier(out: &mut String, m: VariablePtModifier) {
    write_signed_variable(out, m.power);
    out.push('/');
    write_signed_variable(out, m.toughness);
}

fn write_signed_variable(out: &mut String, v: SignedVariable) {
    out.push(match v.sign {
        Sign::Plus => '+',
        Sign::Minus => '-',
    });
    out.push_str(variable_name(v.variable));
}

fn write_variable_definitions(out: &mut String, definitions: &[VariableDefinition]) {
    for (i, definition) in definitions.iter().enumerate() {
        if i > 0 {
            if i + 1 == definitions.len() {
                out.push_str(", and ");
            } else {
                out.push_str(", ");
            }
        }
        write_variable_definition(out, definition);
    }
}

fn write_variable_definition(out: &mut String, definition: &VariableDefinition) {
    out.push_str(variable_name(definition.variable));
    out.push_str(" is ");
    write_value_expression(out, &definition.value);
}

fn write_value_expression(out: &mut String, expression: &ValueExpression) {
    match expression {
        ValueExpression::HalfNumberOfBasicLandsYouControl {
            basic_land_type,
            rounding,
        } => {
            out.push_str("half the number of ");
            out.push_str(basic_land_type_plural_name(*basic_land_type));
            out.push_str(" you control, rounded ");
            out.push_str(rounding_name(*rounding));
        }
        ValueExpression::ItsPower => out.push_str("its power"),
    }
}

fn variable_name(variable: Variable) -> &'static str {
    match variable {
        Variable::X => "X",
        Variable::Y => "Y",
    }
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

fn permanent_type_plural_name(pt: PermanentType) -> &'static str {
    match pt {
        PermanentType::Artifact => "artifacts",
        PermanentType::Creature => "creatures",
        PermanentType::Enchantment => "enchantments",
        PermanentType::Land => "lands",
        PermanentType::Planeswalker => "planeswalkers",
    }
}

fn color_name_capitalized(color: Color) -> &'static str {
    match color {
        Color::White => "White",
        Color::Blue => "Blue",
        Color::Black => "Black",
        Color::Red => "Red",
        Color::Green => "Green",
    }
}

fn creature_type_name(ct: CreatureType) -> &'static str {
    match ct {
        CreatureType::Wall => "Wall",
    }
}

fn step_name(step: Step) -> &'static str {
    match step {
        Step::CombatDamage => "combat damage",
    }
}

fn basic_land_type_plural_name(land_type: BasicLandType) -> &'static str {
    match land_type {
        BasicLandType::Forest => "Forests",
    }
}

fn rounding_name(rounding: Rounding) -> &'static str {
    match rounding {
        Rounding::Down => "down",
        Rounding::Up => "up",
    }
}

fn indefinite_article(pt: PermanentType) -> &'static str {
    match pt {
        PermanentType::Artifact | PermanentType::Enchantment => "an",
        PermanentType::Creature | PermanentType::Land | PermanentType::Planeswalker => "a",
    }
}
