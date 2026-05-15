use std::fmt::Write;

use crate::ast::{
    ActionTiming, ActivatedAbility, ActivatedCost, ActivatedEffect, BalanceSameWayAction,
    BasicLandType, CardCount, CastRestriction, Color, Condition, ContinuousEffect, CopyException,
    CreatureStatus, CreatureType, DamageAmount, DamageLifeGainCap, DamageRecipient,
    EachPlayerAction, EnchantObject, EnchantedObject, ImperativeAction, InterveningIf, Keyword,
    LandCountController, ManaCost, ManaSymbol, MixedPtModifier, ModalMode, OptionalCost,
    PermanentType, PhysicalAction, PreventionRecipient, PtModifier, Rounding, Sign, SignedNumber,
    SignedPtComponent, SignedVariable, SourceObject, SpellType, Statement, StaticAbility, Step,
    TriggerEffect, TriggerEvent, TriggeredAbility, ValueExpression, Variable, VariableDefinition,
    VariablePtModifier, Zone,
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
        Statement::CounterTargetSpell => out.push_str("Counter target spell."),
        Statement::ThisSpellCostsManaMoreToCastForEachTargetBeyondTheFirst { mana } => {
            out.push_str("This spell costs ");
            write_mana_cost(out, mana);
            out.push_str(" more to cast for each target beyond the first.");
        }
        Statement::DestroyTargetCreature => out.push_str("Destroy target creature."),
        Statement::RegenerateTargetCreature => out.push_str("Regenerate target creature."),
        Statement::NamedSourceDealsVariableDamageDividedEvenlyRoundedDownAmongAnyNumberOfTargets {
            source_name,
            amount,
        } => {
            out.push_str(source_name);
            out.push_str(" deals ");
            out.push_str(variable_name(*amount));
            out.push_str(
                " damage divided evenly, rounded down, among any number of targets.",
            );
        }
        Statement::NamedSourceDealsVariableDamageToAnyTarget {
            source_name,
            amount,
        } => {
            out.push_str(source_name);
            out.push_str(" deals ");
            out.push_str(variable_name(*amount));
            out.push_str(" damage to any target.");
        }
        Statement::NamedSourceDealsVariableDamageToDamageRecipients {
            source_name,
            amount,
            recipients,
        } => {
            out.push_str(source_name);
            out.push_str(" deals ");
            out.push_str(variable_name(*amount));
            out.push_str(" damage to ");
            write_damage_recipients(out, recipients);
            out.push('.');
        }
        Statement::PreventAllCombatDamageThisTurn => {
            out.push_str("Prevent all combat damage that would be dealt this turn.");
        }
        Statement::SpendOnlyColorManaOnVariable { color, variable } => {
            out.push_str("Spend only ");
            out.push_str(color_name(*color));
            out.push_str(" mana on ");
            out.push_str(variable_name(*variable));
            out.push('.');
        }
        Statement::YouGainLifeEqualToDamageDealtCapped { caps } => {
            out.push_str("You gain life equal to the damage dealt, but not more life than ");
            write_damage_life_gain_caps(out, caps);
            out.push('.');
        }
        Statement::IfItsPermanentCantBeRegeneratedAndWouldDieExileInsteadThisTurn {
            permanent_type,
        } => {
            out.push_str("If it's ");
            out.push_str(indefinite_article(*permanent_type));
            out.push(' ');
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(
                ", it can't be regenerated this turn, and if it would die this turn, exile it instead.",
            );
        }
        Statement::DestroyTargetPermanentChoice { permanent_types } => {
            out.push_str("Destroy target ");
            write_permanent_type_choice(out, permanent_types);
            out.push('.');
        }
        Statement::DestroyAll { permanent_type } => {
            out.push_str("Destroy all ");
            out.push_str(permanent_type_plural_name(*permanent_type));
            out.push('.');
        }
        Statement::DestroyAllBasicLands { basic_land_type } => {
            out.push_str("Destroy all ");
            out.push_str(basic_land_type_plural_name(*basic_land_type));
            out.push('.');
        }
        Statement::Keyword(kw) => write_keyword(out, *kw),
        Statement::TargetPlayerDrawsCards { count } => {
            out.push_str("Target player draws ");
            write_card_count(out, *count);
            out.push_str(" cards.");
        }
        Statement::TargetPlayerGainsLife { amount } => {
            write!(out, "Target player gains {amount} life.").expect("write to String never fails");
        }
        Statement::TargetPlayerActivatesManaAbilityOfEachPermanentTheyControl {
            permanent_type,
        } => {
            out.push_str("Target player activates a mana ability of each ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" they control.");
        }
        Statement::ThenThatPlayerLosesUnspentManaAndYouAddManaLostThisWay => {
            out.push_str(
                "Then that player loses all unspent mana and you add the mana lost this way.",
            );
        }
        Statement::AddMana { mana } => {
            out.push_str("Add ");
            write_mana_cost(out, mana);
            out.push('.');
        }
        Statement::AntePlayRestriction => {
            out.push_str(
                "Remove this card from your deck before playing if you're not playing for ante.",
            );
        }
        Statement::YouOwnTargetCardInZone { zone } => {
            out.push_str("You own target card in the ");
            out.push_str(zone_name(*zone));
            out.push('.');
        }
        Statement::ExchangeThatCardWithTopCardOfYourLibrary => {
            out.push_str("Exchange that card with the top card of your library.");
        }
        Statement::CopyTargetSpellExceptCopyIsColor { spell_types, color } => {
            out.push_str("Copy target ");
            write_spell_type_choice(out, spell_types);
            out.push_str(" spell, except that the copy is ");
            out.push_str(color_name(*color));
            out.push('.');
        }
        Statement::YouMayChooseNewTargetsForTheCopy => {
            out.push_str("You may choose new targets for the copy.");
        }
        Statement::ImperativeActionSequence { actions } => {
            write_imperative_action_sequence(out, actions);
        }
        Statement::EachPlayerPerformsAction { action } => {
            out.push_str("Each player ");
            write_each_player_action(out, *action);
            out.push('.');
        }
        Statement::UntilEndOfTurnYouMayPayCostAtTiming { timing, cost } => {
            out.push_str("Until end of turn, ");
            write_action_timing(out, *timing);
            out.push_str(", you may ");
            write_optional_cost(out, cost);
            out.push('.');
        }
        Statement::PreventNextDamageThatWouldBeDealtToRecipientThisTurn { amount, recipient } => {
            write_prevent_next_damage_to_recipient(out, *amount, *recipient);
        }
        Statement::IfYouDoPreventNextDamageThatWouldBeDealtToRecipientThisTurn {
            amount,
            recipient,
        } => {
            out.push_str("If you do, ");
            write_prevent_next_damage_to_recipient_lowercase(out, *amount, *recipient);
        }
        Statement::IfYouDoAddMana { mana } => {
            out.push_str("If you do, add ");
            write_mana_cost(out, mana);
            out.push('.');
        }
        Statement::IfYouDoGainLife { amount } => {
            write!(out, "If you do, you gain {amount} life.").expect("write to String never fails");
        }
        Statement::TargetSpellOrPermanentBecomesColor { color } => {
            out.push_str("Target spell or permanent becomes ");
            out.push_str(color_name(*color));
            out.push('.');
        }
        Statement::TargetPermanentGetsUntilEndOfTurn {
            permanent_type,
            modifier,
        } => {
            out.push_str("Target ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" gets ");
            write_pt_modifier(out, *modifier);
            out.push_str(" until end of turn.");
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
        Statement::AsThisPermanentEntersChooseOpponent { permanent_type } => {
            out.push_str("As this ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" enters, choose an opponent.");
        }
        Statement::ThisPermanentEntersWithCounters {
            source,
            amount,
            counter,
        } => {
            write_source_object_capitalized(out, *source);
            out.push_str(" enters with ");
            out.push_str(u32_to_number_word(*amount));
            out.push(' ');
            write_pt_modifier(out, *counter);
            out.push_str(" counters on it.");
        }
        Statement::ThisAbilityCantCauseTotalCountersGreaterThan {
            counter,
            source,
            maximum,
        } => {
            out.push_str("This ability can't cause the total number of ");
            write_pt_modifier(out, *counter);
            out.push_str(" counters on ");
            write_source_object(out, *source);
            out.push_str(" to be greater than ");
            out.push_str(u32_to_number_word(*maximum));
            out.push('.');
        }
        Statement::IfThisAbilityActivatedAtLeastTimesThisTurnSacrificeSourceAtNextEndStep {
            threshold,
            source,
        } => {
            out.push_str("If this ability has been activated ");
            out.push_str(u32_to_number_word(*threshold));
            out.push_str(" or more times this turn, sacrifice ");
            write_source_object(out, *source);
            out.push_str(" at the beginning of the next end step.");
        }
        Statement::ActivateOnlyDuringYourUpkeep => {
            out.push_str("Activate only during your upkeep.");
        }
        Statement::ActivateOnlyDuringYourTurn => {
            out.push_str("Activate only during your turn.");
        }
        Statement::ModalChoice { modes } => write_modal_choice(out, modes),
        Statement::StaticAbility(sa) => write_static_ability(out, sa),
        Statement::ActivatedAbility(aa) => write_activated_ability(out, aa),
        Statement::TriggeredAbility(ta) => write_triggered_ability(out, ta),
        Statement::PhysicalAction(pa) => write_physical_action(out, *pa),
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

fn write_modal_choice(out: &mut String, modes: &[ModalMode]) {
    out.push_str("Choose one —");
    for mode in modes {
        out.push_str("\n• ");
        write_modal_mode(out, *mode);
    }
}

fn write_modal_mode(out: &mut String, mode: ModalMode) {
    match mode {
        ModalMode::CounterTargetColoredSpell { color } => {
            out.push_str("Counter target ");
            out.push_str(color_name(color));
            out.push_str(" spell.");
        }
        ModalMode::DestroyTargetColoredPermanent { color } => {
            out.push_str("Destroy target ");
            out.push_str(color_name(color));
            out.push_str(" permanent.");
        }
        ModalMode::TargetPlayerGainsLife { amount } => {
            write!(out, "Target player gains {amount} life.").expect("write to String never fails");
        }
        ModalMode::PreventNextDamageThatWouldBeDealtToRecipientThisTurn { amount, recipient } => {
            write_prevent_next_damage_to_recipient(out, amount, recipient);
        }
    }
}

fn write_damage_life_gain_caps(out: &mut String, caps: &[DamageLifeGainCap]) {
    for (i, cap) in caps.iter().enumerate() {
        if i > 0 {
            if i + 1 == caps.len() {
                if caps.len() > 2 {
                    out.push(',');
                }
                out.push_str(" or ");
            } else {
                out.push_str(", ");
            }
        }
        write_damage_life_gain_cap(out, *cap);
    }
}

fn write_damage_life_gain_cap(out: &mut String, cap: DamageLifeGainCap) {
    match cap {
        DamageLifeGainCap::PlayerLifeTotalBeforeDamageDealt => {
            out.push_str("the player's life total before the damage was dealt");
        }
        DamageLifeGainCap::PlaneswalkerLoyaltyBeforeDamageDealt => {
            out.push_str("the planeswalker's loyalty before the damage was dealt");
        }
        DamageLifeGainCap::CreatureToughness => {
            out.push_str("the creature's toughness");
        }
    }
}

fn write_damage_recipients(out: &mut String, recipients: &[DamageRecipient]) {
    for (idx, recipient) in recipients.iter().enumerate() {
        if idx > 0 {
            out.push_str(" and ");
        }
        write_damage_recipient(out, *recipient);
    }
}

fn write_damage_recipient(out: &mut String, recipient: DamageRecipient) {
    match recipient {
        DamageRecipient::EachCreatureWithoutKeyword { keyword } => {
            out.push_str("each creature without ");
            write_keyword(out, keyword);
        }
        DamageRecipient::EachPlayer => out.push_str("each player"),
    }
}

fn write_spell_type_choice(out: &mut String, spell_types: &[SpellType]) {
    for (idx, spell_type) in spell_types.iter().enumerate() {
        if idx > 0 {
            out.push_str(" or ");
        }
        out.push_str(spell_type_name(*spell_type));
    }
}

fn spell_type_name(spell_type: SpellType) -> &'static str {
    match spell_type {
        SpellType::Instant => "instant",
        SpellType::Sorcery => "sorcery",
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
        CastRestriction::DuringStep { step } => {
            out.push_str("during the ");
            out.push_str(step_name(step));
            out.push_str(" step.");
        }
        CastRestriction::DuringYourStep { step } => {
            out.push_str("during your ");
            out.push_str(step_name(step));
            out.push_str(" step.");
        }
        CastRestriction::DuringCombatBeforeBlockersAreDeclared => {
            out.push_str("during combat before blockers are declared.");
        }
    }
}

fn write_imperative_action_sequence(out: &mut String, actions: &[ImperativeAction]) {
    for (i, action) in actions.iter().enumerate() {
        if i > 0 {
            if i + 1 == actions.len() {
                out.push_str(", then ");
            } else {
                out.push_str(", ");
            }
        }
        write_imperative_action(out, *action);
    }
    out.push('.');
}

fn write_imperative_action(out: &mut String, action: ImperativeAction) {
    match action {
        ImperativeAction::DiscardYourHand => out.push_str("Discard your hand"),
        ImperativeAction::AnteTopCardOfYourLibrary => {
            out.push_str("ante the top card of your library");
        }
        ImperativeAction::SearchYourLibraryForACard => {
            out.push_str("Search your library for a card");
        }
        ImperativeAction::PutThatCardIntoYourHand => {
            out.push_str("put that card into your hand");
        }
        ImperativeAction::Shuffle => out.push_str("shuffle"),
        ImperativeAction::DrawCards { count } => {
            out.push_str("draw ");
            write_card_count(out, count);
            out.push_str(" cards");
        }
        ImperativeAction::TapSource { source } => {
            out.push_str("tap ");
            write_source_object(out, source);
        }
        ImperativeAction::SacrificePermanentOfOpponentsChoice { permanent_type } => {
            out.push_str("sacrifice ");
            out.push_str(indefinite_article(permanent_type));
            out.push(' ');
            out.push_str(permanent_type_name(permanent_type));
            out.push_str(" of an opponent's choice");
        }
    }
}

fn write_trigger_action_list(out: &mut String, actions: &[ImperativeAction]) {
    for (i, action) in actions.iter().enumerate() {
        if i > 0 {
            if i + 1 == actions.len() {
                out.push_str(" and ");
            } else {
                out.push_str(", ");
            }
        }
        write_imperative_action(out, *action);
    }
}

fn write_each_player_action(out: &mut String, action: EachPlayerAction) {
    match action {
        EachPlayerAction::AnteTopCardOfTheirLibrary => {
            out.push_str("antes the top card of their library");
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

fn write_card_count(out: &mut String, count: CardCount) {
    match count {
        CardCount::Number(n) => out.push_str(u32_to_number_word(n)),
        CardCount::Variable(variable) => out.push_str(variable_name(variable)),
    }
}

fn write_action_timing(out: &mut String, timing: ActionTiming) {
    match timing {
        ActionTiming::AnyTimeYouCouldActivateAManaAbility => {
            out.push_str("any time you could activate a mana ability");
        }
        ActionTiming::AnyTimeYouCouldCastAnInstant => {
            out.push_str("any time you could cast an instant");
        }
    }
}

fn write_optional_cost(out: &mut String, cost: &OptionalCost) {
    match cost {
        OptionalCost::PayLife { amount } => {
            write!(out, "pay {amount} life").expect("write to String never fails");
        }
        OptionalCost::PayMana { mana } => {
            out.push_str("pay ");
            write_mana_cost(out, mana);
        }
    }
}

fn write_prevent_next_damage_to_recipient(
    out: &mut String,
    amount: DamageAmount,
    recipient: PreventionRecipient,
) {
    out.push_str("Prevent the next ");
    write_damage_amount(out, amount);
    out.push_str(" damage that would be dealt to ");
    write_prevention_recipient(out, recipient);
    out.push_str(" this turn.");
}

fn write_prevent_next_damage_to_recipient_lowercase(
    out: &mut String,
    amount: DamageAmount,
    recipient: PreventionRecipient,
) {
    out.push_str("prevent the next ");
    write_damage_amount(out, amount);
    out.push_str(" damage that would be dealt to ");
    write_prevention_recipient(out, recipient);
    out.push_str(" this turn.");
}

fn write_damage_amount(out: &mut String, amount: DamageAmount) {
    match amount {
        DamageAmount::Number(n) => write!(out, "{n}").expect("write to String never fails"),
        DamageAmount::Variable(variable) => out.push_str(variable_name(variable)),
    }
}

fn write_prevention_recipient(out: &mut String, recipient: PreventionRecipient) {
    match recipient {
        PreventionRecipient::AnyTarget => out.push_str("any target"),
        PreventionRecipient::ThatPermanentOrPlayer => out.push_str("that permanent or player"),
    }
}

fn write_keyword(out: &mut String, kw: Keyword) {
    match kw {
        Keyword::FirstStrike => out.push_str("First strike"),
        Keyword::Flying => out.push_str("Flying"),
        Keyword::Reach => out.push_str("Reach"),
        Keyword::Defender => out.push_str("Defender"),
        Keyword::Banding => out.push_str("Banding"),
        Keyword::Trample => out.push_str("Trample"),
        Keyword::Mountainwalk => out.push_str("Mountainwalk"),
        Keyword::Swampwalk => out.push_str("Swampwalk"),
        Keyword::Indestructible => out.push_str("Indestructible"),
        Keyword::Fear => out.push_str("Fear"),
        Keyword::Protection(color) => {
            out.push_str("Protection from ");
            out.push_str(color_name(color));
        }
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
        Zone::Ante => "ante",
    }
}

fn zone_article(zone: Zone) -> &'static str {
    match zone {
        Zone::Graveyard => "a",
        Zone::Ante => "the",
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
    for (i, cost) in aa.costs.iter().enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_activated_cost(out, cost);
    }
    out.push_str(": ");
    write_activated_effect(out, &aa.effect);
}

fn write_activated_cost(out: &mut String, cost: &ActivatedCost) {
    match cost {
        ActivatedCost::Mana(mana) => write_mana_cost(out, mana),
        ActivatedCost::VariableMana(variable) => {
            out.push('{');
            out.push_str(variable_name(*variable));
            out.push('}');
        }
        ActivatedCost::Tap => out.push_str("{T}"),
        ActivatedCost::Sacrifice(source) => {
            out.push_str("Sacrifice ");
            write_source_object(out, *source);
        }
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
        ActivatedEffect::AddManaOfAnyOneColor { amount } => {
            out.push_str("Add ");
            out.push_str(u32_to_number_word(*amount));
            out.push_str(" mana of any one color.");
        }
        ActivatedEffect::Untap(source) => {
            out.push_str("Untap ");
            write_source_object(out, *source);
            out.push('.');
        }
        ActivatedEffect::Regenerate(source) => {
            out.push_str("Regenerate ");
            write_source_object(out, *source);
            out.push('.');
        }
        ActivatedEffect::CounterTargetColoredSpell { color } => {
            out.push_str("Counter target ");
            out.push_str(color_name(*color));
            out.push_str(" spell.");
        }
        ActivatedEffect::DestroyTargetPermanent { permanent_type } => {
            out.push_str("Destroy target ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push('.');
        }
        ActivatedEffect::DestroyTargetCreatureType { creature_type } => {
            out.push_str("Destroy target ");
            write_creature_type(out, *creature_type);
            out.push('.');
        }
        ActivatedEffect::LookAtTargetPlayersHand => {
            out.push_str("Look at target player's hand.");
        }
        ActivatedEffect::TargetPlayerDiscardsCards { count } => {
            out.push_str("Target player discards ");
            write_discard_count(out, *count);
            out.push('.');
        }
        ActivatedEffect::TargetCreatureWithPowerOrLessCantBeBlockedThisTurn { power } => {
            write!(
                out,
                "Target creature with power {power} or less can't be blocked this turn."
            )
            .expect("writing to String cannot fail");
        }
        ActivatedEffect::TargetPermanentGainsKeywordUntilEndOfTurn {
            permanent_type,
            keyword,
        } => {
            out.push_str("Target ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" gains ");
            write_keyword_lowercase(out, *keyword);
            out.push_str(" until end of turn.");
        }
        ActivatedEffect::EnchantedGetsUntilEndOfTurn {
            permanent_type,
            modifier,
        } => {
            out.push_str("Enchanted ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" gets ");
            write_pt_modifier(out, *modifier);
            out.push_str(" until end of turn.");
        }
        ActivatedEffect::SourceGetsUntilEndOfTurn { source, modifier } => {
            write_source_object_capitalized(out, *source);
            out.push_str(" gets ");
            write_pt_modifier(out, *modifier);
            out.push_str(" until end of turn.");
        }
        ActivatedEffect::SourceGainsKeywordUntilEndOfTurn { source, keyword } => {
            write_source_object_capitalized(out, *source);
            out.push_str(" gains ");
            write_keyword_lowercase(out, *keyword);
            out.push_str(" until end of turn.");
        }
        ActivatedEffect::PreventNextDamageFromColoredSource { color } => {
            out.push_str("The next time ");
            out.push_str(color_article(*color));
            out.push(' ');
            out.push_str(color_name(*color));
            out.push_str(
                " source of your choice would deal damage to you this turn, prevent that damage.",
            );
        }
        ActivatedEffect::PreventAllButDamageFromUnblockedCreature { amount } => {
            write!(
                out,
                "The next time an unblocked creature of your choice would deal combat damage to you this turn, prevent all but {amount} of that damage."
            )
            .expect("writing to String cannot fail");
        }
        ActivatedEffect::PreventNextDamageToYouThisTurn { amount } => {
            write!(
                out,
                "Prevent the next {amount} damage that would be dealt to you this turn."
            )
            .expect("writing to String cannot fail");
        }
        ActivatedEffect::PutUpToVariableCountersOnSource {
            amount,
            counter,
            source,
        } => {
            out.push_str("Put up to ");
            out.push_str(variable_name(*amount));
            out.push(' ');
            write_pt_modifier(out, *counter);
            out.push_str(" counters on ");
            write_source_object(out, *source);
            out.push('.');
        }
        ActivatedEffect::PutNamedCounterOnTargetNonBasicLand {
            counter_name,
            excluded_land_type,
        } => {
            out.push_str("Put a ");
            out.push_str(counter_name);
            out.push_str(" counter on target non-");
            out.push_str(basic_land_type_name(*excluded_land_type));
            out.push_str(" land.");
        }
        ActivatedEffect::TargetPermanentBecomesBasicLandTypeUntilSourceLeavesBattlefield {
            permanent_type,
            land_type,
            source,
        } => {
            out.push_str("Target ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" becomes a ");
            out.push_str(basic_land_type_name(*land_type));
            out.push_str(" until ");
            write_source_object(out, *source);
            out.push_str(" leaves the battlefield.");
        }
        ActivatedEffect::PhysicalAction(action) => write_physical_action(out, *action),
    }
}

fn write_discard_count(out: &mut String, count: CardCount) {
    match count {
        CardCount::Number(1) => out.push_str("a card"),
        CardCount::Number(n) => {
            out.push_str(u32_to_number_word(n));
            out.push_str(" cards");
        }
        CardCount::Variable(variable) => {
            out.push_str(variable_name(variable));
            out.push_str(" cards");
        }
    }
}

fn write_physical_action(out: &mut String, action: PhysicalAction) {
    match action {
        PhysicalAction::IfSourceOnBattlefieldFlipOntoBattlefieldFromHeight {
            source,
            minimum_height_feet,
        } => {
            out.push_str("If ");
            write_source_object(out, source);
            out.push_str(" is on the battlefield, flip it onto the battlefield from a height of at least ");
            out.push_str(u32_to_number_word(minimum_height_feet));
            out.push(' ');
            out.push_str(if minimum_height_feet == 1 { "foot" } else { "feet" });
            out.push('.');
        }
        PhysicalAction::IfSourceTurnsOverCompletelyAtLeastOnceDuringFlipDestroyAllNontokenPermanentsItTouches {
            source,
        } => {
            out.push_str("If ");
            write_source_object(out, source);
            out.push_str(" turns over completely at least once during the flip, destroy all nontoken permanents it touches.");
        }
        PhysicalAction::ThenDestroySource { source } => {
            out.push_str("Then destroy ");
            write_source_object(out, source);
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
        StaticAbility::ColoredSpellsCostManaMoreToCast { color, mana } => {
            out.push_str(color_name_capitalized(*color));
            out.push_str(" spells cost ");
            write_mana_cost(out, mana);
            out.push_str(" more to cast.");
        }
        StaticAbility::ActivatedAbilitiesOfColoredPermanentsCostManaMoreToActivate {
            color,
            permanent_type,
            mana,
        } => {
            out.push_str("Activated abilities of ");
            out.push_str(color_name(*color));
            out.push(' ');
            out.push_str(permanent_type_plural_name(*permanent_type));
            out.push_str(" cost ");
            write_mana_cost(out, mana);
            out.push_str(" more to activate.");
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
        StaticAbility::OtherCreatureTypeGetAndHaveKeyword {
            creature_type,
            modifier,
            keyword,
        } => {
            out.push_str("Other ");
            out.push_str(creature_type_plural_name(*creature_type));
            out.push_str(" get ");
            write_pt_modifier(out, *modifier);
            out.push_str(" and have ");
            write_keyword_lowercase(out, *keyword);
            out.push('.');
        }
        StaticAbility::StatusCreaturesYouControlGet { status, modifier } => {
            out.push_str(creature_status_name_capitalized(*status));
            out.push_str(" creatures you control get ");
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
        StaticAbility::EnchantedHasKeyword { object, keyword } => {
            out.push_str("Enchanted ");
            write_enchanted_object(out, *object);
            out.push_str(" has ");
            write_keyword_lowercase(out, *keyword);
            out.push('.');
        }
        StaticAbility::EnchantedHasTriggeredAbility { object, ability } => {
            out.push_str("Enchanted ");
            write_enchanted_object(out, *object);
            out.push_str(" has \"");
            write_triggered_ability(out, ability);
            out.push('"');
        }
        StaticAbility::EnchantedLosesKeyword { object, keyword } => {
            out.push_str("Enchanted ");
            write_enchanted_object(out, *object);
            out.push_str(" loses ");
            write_keyword_lowercase(out, *keyword);
            out.push('.');
        }
        StaticAbility::EnchantedIsBasicLandType { object, land_type } => {
            out.push_str("Enchanted ");
            write_enchanted_object(out, *object);
            out.push_str(" is a ");
            out.push_str(basic_land_type_name(*land_type));
            out.push('.');
        }
        StaticAbility::EnchantedHasKeywordAndCantBeEnchantedByOtherAuras { object, keyword } => {
            out.push_str("Enchanted ");
            write_enchanted_object(out, *object);
            out.push_str(" has ");
            write_keyword_lowercase(out, *keyword);
            out.push_str(" and can't be enchanted by other Auras.");
        }
        StaticAbility::EnchantedCanAttackAsThoughItDidntHave { object, keyword } => {
            out.push_str("Enchanted ");
            write_enchanted_object(out, *object);
            out.push_str(" can attack as though it didn't have ");
            write_keyword_lowercase(out, *keyword);
            out.push('.');
        }
        StaticAbility::YouControlEnchanted { object } => {
            out.push_str("You control enchanted ");
            write_enchanted_object(out, *object);
            out.push('.');
        }
        StaticAbility::YouMayPlayAnyNumberOfPermanentsOnEachOfYourTurns { permanent_type } => {
            out.push_str("You may play any number of ");
            out.push_str(permanent_type_plural_name(*permanent_type));
            out.push_str(" on each of your turns.");
        }
        StaticAbility::YouMayHaveSourceEnterAsCopyOfAnyPermanentOnBattlefield {
            source,
            permanent_type,
            exception,
        } => {
            out.push_str("You may have ");
            write_source_object(out, *source);
            out.push_str(" enter as a copy of any ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" on the battlefield");
            if let Some(exception) = exception {
                write_copy_exception(out, *exception);
            }
            out.push('.');
        }
        StaticAbility::EffectDoesntRemoveThisAura => {
            out.push_str("This effect doesn't remove this Aura.");
        }
        StaticAbility::SourceDoesntUntapDuringYourUntapStep { source } => {
            write_source_object_capitalized(out, *source);
            out.push_str(" doesn't untap during your untap step.");
        }
        StaticAbility::BasicLandsAreBasicLands { from, to } => {
            out.push_str("All ");
            out.push_str(basic_land_type_plural_name(*from));
            out.push_str(" are ");
            out.push_str(basic_land_type_plural_name(*to));
            out.push('.');
        }
        StaticAbility::ThatPermanentIsBasicLandTypeWhileHasNamedCounter {
            permanent_type,
            land_type,
            counter_name,
        } => {
            out.push_str("That ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" is a ");
            out.push_str(basic_land_type_name(*land_type));
            out.push_str(" for as long as it has a ");
            out.push_str(counter_name);
            out.push_str(" counter on it.");
        }
        StaticAbility::TargetCreatureDefendingPlayerControlsCanBlockAnyNumberOfCreaturesThisTurn => {
            out.push_str(
                "Target creature defending player controls can block any number of creatures this turn.",
            );
        }
        StaticAbility::RemoveTargetCreatureDefendingPlayerControlsFromCombat => {
            out.push_str("Remove target creature defending player controls from combat.");
        }
        StaticAbility::CreaturesItWasBlockingBecomeUnblocked => {
            out.push_str("Creatures it was blocking that had become blocked by only that creature this combat become unblocked.");
        }
        StaticAbility::YouMayHaveItBlockAttackingCreatureOfYourChoice => {
            out.push_str("You may have it block an attacking creature of your choice.");
        }
        StaticAbility::ItBlocksEachAttackingCreatureThisTurnIfAble => {
            out.push_str("It blocks each attacking creature this turn if able.");
        }
        StaticAbility::ThisTurnDefendingPlayersMakeRandomBlockingPiles => {
            out.push_str("This turn, instead of declaring blockers, each defending player chooses any number of creatures they control and divides them into a number of piles equal to the number of attacking creatures for whom that player is the defending player.");
        }
        StaticAbility::AdditionalBlockersMayBePutIntoAdditionalPiles => {
            out.push_str("Creatures those players control that can block additional creatures may likewise be put into additional piles.");
        }
        StaticAbility::AssignEachPileToAttackingCreatureAtRandom => {
            out.push_str("Assign each pile to a different one of those attacking creatures at random.");
        }
        StaticAbility::CreaturesInAssignedPileBlockIfAble => {
            out.push_str("Each creature in a pile that can block the creature that pile is assigned to does so.");
        }
    }
}

fn write_triggered_ability(out: &mut String, ta: &TriggeredAbility) {
    out.push_str(match ta.event {
        TriggerEvent::PermanentEnters { .. }
        | TriggerEvent::PermanentPutIntoGraveyardFromBattlefield { .. }
        | TriggerEvent::YouPlayPermanent { .. }
        | TriggerEvent::PlayerCastsColoredSpell { .. }
        | TriggerEvent::BasicLandTypeIsTappedForMana { .. }
        | TriggerEvent::SourceIsDealtDamage { .. }
        | TriggerEvent::SourceBlocksOrBecomesBlockedByNonCreatureTypeCreature { .. } => "Whenever ",
        TriggerEvent::BeginningOfTheNextEndStep
        | TriggerEvent::BeginningOfChosenPlayersUpkeep
        | TriggerEvent::BeginningOfEachPlayersUpkeep
        | TriggerEvent::BeginningOfYourUpkeep
        | TriggerEvent::BeginningOfUpkeepOfEnchantedPermanentController { .. }
        | TriggerEvent::EndOfCombat => "At ",
        TriggerEvent::ThisAuraEnters
        | TriggerEvent::ThisAuraLeavesTheBattlefield
        | TriggerEvent::SourcePutIntoGraveyardFromBattlefield { .. } => "When ",
        TriggerEvent::EnchantedPermanentDies { .. } => "When ",
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
            if matches!(eff, TriggerEffect::SourceGainsStaticAbility { .. }) {
                out.push_str(" and ");
            } else {
                out.push(' ');
            }
        }
        write_trigger_effect(out, eff, i + 1 == ta.effects.len());
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
        TriggerEvent::PlayerCastsColoredSpell { color } => {
            out.push_str("a player casts a ");
            out.push_str(color_name(color));
            out.push_str(" spell");
        }
        TriggerEvent::BasicLandTypeIsTappedForMana { land_type } => {
            out.push_str("a ");
            out.push_str(basic_land_type_name(land_type));
            out.push_str(" is tapped for mana");
        }
        TriggerEvent::YouPlayPermanent { permanent_type } => {
            out.push_str("you play ");
            out.push_str(indefinite_article(permanent_type));
            out.push(' ');
            out.push_str(permanent_type_name(permanent_type));
        }
        TriggerEvent::EnchantedPermanentDies { permanent_type } => {
            out.push_str("enchanted ");
            out.push_str(permanent_type_name(permanent_type));
            out.push_str(" dies");
        }
        TriggerEvent::BeginningOfTheNextEndStep => {
            out.push_str("the beginning of the next end step");
        }
        TriggerEvent::BeginningOfChosenPlayersUpkeep => {
            out.push_str("the beginning of the chosen player's upkeep");
        }
        TriggerEvent::BeginningOfEachPlayersUpkeep => {
            out.push_str("the beginning of each player's upkeep");
        }
        TriggerEvent::BeginningOfYourUpkeep => {
            out.push_str("the beginning of your upkeep");
        }
        TriggerEvent::SourcePutIntoGraveyardFromBattlefield { source } => {
            write_source_object(out, source);
            out.push_str(" is put into a graveyard from the battlefield");
        }
        TriggerEvent::SourceIsDealtDamage { source } => {
            write_source_object(out, source);
            out.push_str(" is dealt damage");
        }
        TriggerEvent::PermanentPutIntoGraveyardFromBattlefield { permanent_type } => {
            out.push_str(indefinite_article(permanent_type));
            out.push(' ');
            out.push_str(permanent_type_name(permanent_type));
            out.push_str(" is put into a graveyard from the battlefield");
        }
        TriggerEvent::BeginningOfUpkeepOfEnchantedPermanentController { permanent_type } => {
            out.push_str("the beginning of the upkeep of enchanted ");
            out.push_str(permanent_type_name(permanent_type));
            out.push_str("'s controller");
        }
        TriggerEvent::EndOfCombat => {
            out.push_str("end of combat");
        }
        TriggerEvent::SourceBlocksOrBecomesBlockedByNonCreatureTypeCreature {
            source,
            excluded_type,
        } => {
            write_source_object(out, source);
            out.push_str(" blocks or becomes blocked by a non-");
            write_creature_type(out, excluded_type);
            out.push_str(" creature");
        }
    }
}

fn write_intervening_if(out: &mut String, iif: InterveningIf) {
    match iif {
        InterveningIf::ItsOnTheBattlefield => out.push_str("it's on the battlefield"),
        InterveningIf::EnchantedHasKeyword { object, keyword } => {
            out.push_str("enchanted ");
            write_enchanted_object(out, object);
            out.push_str(" has ");
            write_keyword_lowercase(out, keyword);
        }
        InterveningIf::ItWasntFirstPermanentYouPlayedThisTurn { permanent_type } => {
            out.push_str("it wasn't the first ");
            out.push_str(permanent_type_name(permanent_type));
            out.push_str(" you played this turn");
        }
        InterveningIf::SourceAttackedOrBlockedThisCombat { source } => {
            write_source_object(out, source);
            out.push_str(" attacked or blocked this combat");
        }
    }
}

fn write_trigger_effect(out: &mut String, eff: &TriggerEffect, terminal: bool) {
    match eff {
        TriggerEffect::DestroyThatCreatureIfItAttackedThisTurn => {
            out.push_str("destroy that creature if it attacked this turn.");
        }
        TriggerEffect::DestroyThatCreatureAtEndOfCombat => {
            out.push_str("destroy that creature at end of combat.");
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
        TriggerEffect::SourceDealsDamageToThatPlayer { source, amount } => {
            write_source_object(out, *source);
            write!(out, " deals {amount} damage to that player.")
                .expect("write to String never fails");
        }
        TriggerEffect::SourceDealsDamageToYou { source, amount } => {
            write_source_object(out, *source);
            write!(out, " deals {amount} damage to you.").expect("write to String never fails");
        }
        TriggerEffect::SourceDealsDamageToYouUnlessYouPay {
            source,
            amount,
            cost,
        } => {
            write_source_object(out, *source);
            write!(out, " deals {amount} damage to you unless you pay ")
                .expect("write to String never fails");
            write_mana_cost(out, cost);
            out.push('.');
        }
        TriggerEffect::SourceDealsDamageToThatPermanent {
            source,
            amount,
            recipient,
        } => {
            write_source_object(out, *source);
            write!(out, " deals {amount} damage to that ")
                .expect("write to String never fails");
            out.push_str(permanent_type_name(*recipient));
            if terminal {
                out.push('.');
            }
        }
        TriggerEffect::SourceDealsVariableDamageToThatPlayer {
            source,
            amount,
            definitions,
        } => {
            write_source_object(out, *source);
            out.push_str(" deals ");
            out.push_str(variable_name(*amount));
            out.push_str(" damage to that player, where ");
            write_variable_definitions(out, definitions);
            out.push('.');
        }
        TriggerEffect::ItsControllerAddsAdditionalMana { mana } => {
            out.push_str("its controller adds an additional ");
            write_mana_symbol(out, *mana);
            out.push('.');
        }
        TriggerEffect::SourceDealsDamageEqualToThatPermanentsToughnessToThePermanentsController {
            source,
            permanent_type,
        } => {
            write_source_object(out, *source);
            out.push_str(" deals damage equal to that ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str("'s toughness to the ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str("'s controller.");
        }
        TriggerEffect::RemoveCounterFromIt { counter } => {
            out.push_str("remove a ");
            write_pt_modifier(out, *counter);
            out.push_str(" counter from it.");
        }
        TriggerEffect::PutCounterOnIt { counter } => {
            out.push_str("put a ");
            write_pt_modifier(out, *counter);
            out.push_str(" counter on it.");
        }
        TriggerEffect::SourceGainsStaticAbility { source, ability } => {
            write_source_object(out, *source);
            out.push_str(" gains \"");
            write_static_ability(out, ability);
            out.push('"');
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
        TriggerEffect::SacrificeSourceUnlessYouPay { source, cost } => {
            out.push_str("sacrifice ");
            write_source_object(out, *source);
            out.push_str(" unless you pay ");
            write_mana_cost(out, cost);
            out.push('.');
        }
        TriggerEffect::YouMayPayMana { cost } => {
            out.push_str("you may pay ");
            write_mana_cost(out, cost);
            out.push('.');
        }
        TriggerEffect::IfYouDoGainLife { amount } => {
            write!(out, "If you do, you gain {amount} life.")
                .expect("write to String never fails");
        }
        TriggerEffect::UnlessYouPayManaDoActions { cost, actions } => {
            out.push_str("unless you pay ");
            write_mana_cost(out, cost);
            out.push_str(", ");
            write_trigger_action_list(out, actions);
            out.push('.');
        }
        TriggerEffect::DelayedRemoveAllNamedCountersFromLinkedPermanent {
            counter_name,
            permanent_type,
            source,
        } => {
            out.push_str("at the beginning of each of your upkeeps for the rest of the game, remove all ");
            out.push_str(counter_name);
            out.push_str(" counters from a ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" that a ");
            out.push_str(counter_name);
            out.push_str(" counter was put onto with ");
            write_source_object(out, *source);
            out.push_str(" but that a ");
            out.push_str(counter_name);
            out.push_str(" counter has not been removed from with ");
            write_source_object(out, *source);
            out.push('.');
        }
    }
}

fn write_source_object(out: &mut String, source: SourceObject) {
    match source {
        SourceObject::This(pt) => {
            out.push_str("this ");
            out.push_str(permanent_type_name(pt));
        }
        SourceObject::ThisAura => out.push_str("this Aura"),
    }
}

fn write_source_object_capitalized(out: &mut String, source: SourceObject) {
    match source {
        SourceObject::This(pt) => {
            out.push_str("This ");
            out.push_str(permanent_type_name(pt));
        }
        SourceObject::ThisAura => out.push_str("This Aura"),
    }
}

fn write_copy_exception(out: &mut String, exception: CopyException) {
    match exception {
        CopyException::PermanentTypeInAdditionToItsOtherTypes { permanent_type } => {
            out.push_str(", except it's ");
            out.push_str(indefinite_article(permanent_type));
            out.push(' ');
            out.push_str(permanent_type_name(permanent_type));
            out.push_str(" in addition to its other types");
        }
    }
}

/// `write_keyword` capitalizes the first letter ("Flying", "Enchant ...").
/// Inside the quoted text of a loses-and-gains effect the quoted
/// keyword is printed lowercase, which is what we emit here.
fn write_keyword_lowercase(out: &mut String, kw: Keyword) {
    match kw {
        Keyword::FirstStrike => out.push_str("first strike"),
        Keyword::Flying => out.push_str("flying"),
        Keyword::Reach => out.push_str("reach"),
        Keyword::Defender => out.push_str("defender"),
        Keyword::Banding => out.push_str("banding"),
        Keyword::Trample => out.push_str("trample"),
        Keyword::Mountainwalk => out.push_str("mountainwalk"),
        Keyword::Swampwalk => out.push_str("swampwalk"),
        Keyword::Indestructible => out.push_str("indestructible"),
        Keyword::Fear => out.push_str("fear"),
        Keyword::Protection(color) => {
            out.push_str("protection from ");
            out.push_str(color_name(color));
        }
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
        ValueExpression::NumberOfCardsInTheirHandMinus { amount } => {
            write!(out, "the number of cards in their hand minus {amount}")
                .expect("write to String never fails");
        }
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
        Condition::SourceIsAttacking {
            source_name,
            is_attacking,
        } => {
            out.push_str(source_name);
            if *is_attacking {
                out.push_str(" is attacking");
            } else {
                out.push_str(" isn't attacking");
            }
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
        ContinuousEffect::SourcePowerToughnessEachEqualToBasicLandsControlled {
            land_type,
            controller,
        } => {
            out.push_str("its power and toughness are each equal to the number of ");
            out.push_str(basic_land_type_plural_name(*land_type));
            out.push(' ');
            match controller {
                LandCountController::You => out.push_str("you control"),
                LandCountController::DefendingPlayer => out.push_str("defending player controls"),
            }
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

fn write_permanent_type_choice(out: &mut String, permanent_types: &[PermanentType]) {
    for (index, permanent_type) in permanent_types.iter().enumerate() {
        if index > 0 {
            if index == permanent_types.len() - 1 {
                out.push_str(" or ");
            } else {
                out.push_str(", ");
            }
        }
        out.push_str(permanent_type_name(*permanent_type));
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

fn color_name(color: Color) -> &'static str {
    match color {
        Color::White => "white",
        Color::Blue => "blue",
        Color::Black => "black",
        Color::Red => "red",
        Color::Green => "green",
    }
}

fn color_article(color: Color) -> &'static str {
    match color {
        Color::White | Color::Blue | Color::Black | Color::Red | Color::Green => "a",
    }
}

fn creature_status_name_capitalized(status: CreatureStatus) -> &'static str {
    match status {
        CreatureStatus::Tapped => "Tapped",
        CreatureStatus::Untapped => "Untapped",
    }
}

fn creature_type_name(ct: CreatureType) -> &'static str {
    match ct {
        CreatureType::Goblin => "Goblin",
        CreatureType::Wall => "Wall",
    }
}

fn creature_type_plural_name(ct: CreatureType) -> &'static str {
    match ct {
        CreatureType::Goblin => "Goblins",
        CreatureType::Wall => "Walls",
    }
}

fn write_creature_type(out: &mut String, ct: CreatureType) {
    out.push_str(creature_type_name(ct));
}

fn step_name(step: Step) -> &'static str {
    match step {
        Step::CombatDamage => "combat damage",
        Step::DeclareAttackers => "declare attackers",
        Step::DeclareBlockers => "declare blockers",
    }
}

fn basic_land_type_plural_name(land_type: BasicLandType) -> &'static str {
    match land_type {
        BasicLandType::Plains => "Plains",
        BasicLandType::Island => "Islands",
        BasicLandType::Swamp => "Swamps",
        BasicLandType::Mountain => "Mountains",
        BasicLandType::Forest => "Forests",
    }
}

fn basic_land_type_name(land_type: BasicLandType) -> &'static str {
    match land_type {
        BasicLandType::Plains => "Plains",
        BasicLandType::Island => "Island",
        BasicLandType::Swamp => "Swamp",
        BasicLandType::Mountain => "Mountain",
        BasicLandType::Forest => "Forest",
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
