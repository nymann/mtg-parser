use std::fmt::Write;

use crate::ast::{
    ActionTiming, ActivatedAbility, ActivatedCost, ActivatedDamageEffect,
    ActivatedDamageEventEffect, ActivatedDamageRecipient, ActivatedDamageSource, ActivatedEffect,
    ActivationPermission, AddManaAmount, AsEntersChoice, AttackRequirementSubject,
    BalanceSameWayAction, BasicLandType, BasicLandTypeReference, CardCount, CastRestriction, Color,
    ColoredTargetEffect, CombatRole, Condition, ConditionalEffectOrder, ContinuousEffect,
    CopyException, CounterAmount, CounterUnlessCost, CreatureStatus, CreatureType, DamageAmount,
    DamageAssignment, DamageKind, DamageLifeGainCap, DamageLifeGainReference,
    DamagePreventionAmount, DamagePreventionDuration, DamagePreventionEffect,
    DamagePreventionEvent, DamageRecipient, DamageRecipients, DamageRedirectionDestination,
    DestroyTarget, EachPlayerAction, EnchantObject, EnchantedObject, IfYouDoEffect,
    ImperativeAction, InterveningIf, Keyword, LandCountController, LifeLossAmount, LifeLossPlayer,
    ManaCost, ManaSymbol, MixedPtModifier, ModalMode, NamedCounterAmount, NamedDamageEvent,
    NamedKeywordAbility, NamedSourcePowerToughnessCount, ObjectStatus, OptionalCost, PayManaAmount,
    PayManaPlayer, PaymentFailureEffect, PermanentController, PermanentType, PhysicalAction,
    PreventionRecipient, PtModifier, RegenerateRecipient, Rounding, Sign, SignedNumber,
    SignedPtComponent, SignedVariable, SourceObject, SpellAdditionalCost, SpellType, Statement,
    StaticAbility, StaticUntapRestriction, Step, TapAllPermanentsActor,
    TargetPermanentEndOfTurnEffect, TargetPermanentSelector, TextChangeReplacementTerm,
    TriggerCondition, TriggerCounterRecipient, TriggerDamageCondition, TriggerDamageRecipient,
    TriggerDamageSource, TriggerEffect, TriggerEvent, TriggeredAbility, TriggeredDamage,
    ValueExpression, Variable, VariableDefinition, VariablePtModifier, Zone,
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
        Statement::IgnoreThisEffectForEachCreaturePlayerDidntControlContinuouslySinceBeginningOfTurn => {
            out.push_str("Ignore this effect for each creature the player didn't control continuously since the beginning of the turn.");
        }
        Statement::CounterTargetSpell { unless_cost } => {
            out.push_str("Counter target spell");
            if let Some(unless_cost) = unless_cost {
                write_counter_unless_cost(out, unless_cost);
            }
            out.push('.');
        }
        Statement::AsAdditionalCostToCastThisSpell { cost } => {
            out.push_str("As an additional cost to cast this spell, ");
            write_spell_additional_cost(out, *cost);
            out.push('.');
        }
        Statement::ThisSpellCostsManaMoreToCastForEachTargetBeyondTheFirst { mana } => {
            out.push_str("This spell costs ");
            write_mana_cost(out, mana);
            out.push_str(" more to cast for each target beyond the first.");
        }
        Statement::RegenerateTargetCreature => out.push_str("Regenerate target creature."),
        Statement::NamedSourceDealsDamage { event } => {
            write_named_damage_event(out, event);
        }
        Statement::DamageEffect(effect) => write_activated_damage_effect(out, effect),
        Statement::PreventDamageThisTurn {
            effect,
            definitions,
        } => write_damage_prevention_effect_statement(out, *effect, definitions),
        Statement::ForEachDamagePreventedByRemovingCounter {
            amount,
            source,
            counter,
        } => {
            out.push_str("For each ");
            write_damage_amount(out, *amount);
            out.push_str(" damage that would be dealt to ");
            write_source_object(out, *source);
            out.push_str(", if it has a ");
            write_pt_modifier(out, *counter);
            out.push_str(" counter on it, remove a ");
            write_pt_modifier(out, *counter);
            out.push_str(" counter from it and prevent that ");
            write_damage_amount(out, *amount);
            out.push_str(" damage.");
        }
        Statement::SpendOnlyColorManaOnVariable { color, variable } => {
            out.push_str("Spend only ");
            out.push_str(color_name(*color));
            out.push_str(" mana on ");
            out.push_str(variable_name(*variable));
            out.push('.');
        }
        Statement::AsSourceEntersYouLoseLifeEqualToYourLifeTotal { source } => {
            out.push_str("As ");
            write_source_object(out, *source);
            out.push_str(" enters, you lose life equal to your life total.");
        }
        Statement::YouGainLifeEqualToDamage { reference } => {
            out.push_str("You gain life equal to the ");
            write_damage_life_gain_reference(out, reference);
            out.push('.');
        }
        Statement::IfYouCantYouLoseTheGame => {
            out.push_str("If you can't, you lose the game.");
        }
        Statement::IfYouCantSourceDealsDamageToYou { source, amount } => {
            out.push_str("If you can't, ");
            write_source_object(out, *source);
            out.push_str(" deals ");
            write_damage_amount(out, *amount);
            out.push_str(" damage to you.");
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
        Statement::Destroy { target } => write_destroy(out, target),
        Statement::ThatPermanentsControllerMayAttachThisAuraToPermanentOfTheirChoice {
            controller_of,
            attach_to,
        } => {
            out.push_str("That ");
            out.push_str(permanent_type_name(*controller_of));
            out.push_str("'s controller may attach this Aura to ");
            out.push_str(indefinite_article(*attach_to));
            out.push(' ');
            out.push_str(permanent_type_name(*attach_to));
            out.push_str(" of their choice.");
        }
        Statement::Keyword(kw) => write_keyword(out, *kw),
        Statement::KeywordList(keywords) => write_keyword_list(out, keywords),
        Statement::SemicolonKeywordList(keywords) => write_semicolon_keyword_list(out, keywords),
        Statement::TargetPlayerDrawsCards { count } => {
            out.push_str("Target player draws ");
            write_card_count(out, *count);
            out.push_str(" cards.");
        }
        Statement::TargetPlayerDiscardsCardsAtRandom { count } => {
            out.push_str("Target player discards ");
            write_discard_count(out, *count);
            out.push_str(" at random.");
        }
        Statement::IfYouWouldDrawCardDuringYourDrawStepInsteadYouMaySkipThatDraw => {
            out.push_str(
                "If you would draw a card during your draw step, instead you may skip that draw.",
            );
        }
        Statement::LookAtTopCardsOfTargetPlayersLibraryThenPutThemBackInAnyOrder { count } => {
            out.push_str("Look at the top ");
            write_card_count(out, *count);
            out.push_str(" cards of target player's library, then put them back in any order.");
        }
        Statement::YouMayHaveThatPlayerShuffle => {
            out.push_str("You may have that player shuffle.");
        }
        Statement::TargetPlayerGainsLife { amount } => {
            write_target_player_gains_life(out, *amount);
        }
        Statement::TapAllPermanentsAndPlayerLosesUnspentMana {
            actor,
            permanent_type,
            with_mana_abilities,
        } => {
            out.push_str("Tap all ");
            out.push_str(permanent_type_plural_name(*permanent_type));
            if *with_mana_abilities {
                out.push_str(" with mana abilities");
            }
            match actor {
                TapAllPermanentsActor::TargetPlayer => {
                    out.push_str(" target player controls and that player loses all unspent mana.");
                }
                TapAllPermanentsActor::ThatPlayer => {
                    out.push_str(" they control and that player loses all unspent mana.");
                }
            }
        }
        Statement::PlayerPaymentFailure { effect } => {
            out.push_str("If that player doesn't, ");
            write_payment_failure_effect(out, effect);
            out.push('.');
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
        Statement::ChangeTextOfTargetSpellOrPermanentReplacing { term } => {
            out.push_str("Change the text of target spell or permanent by replacing all instances of one ");
            out.push_str(text_change_replacement_term_name(*term));
            out.push_str(" with another.");
        }
        Statement::AddMana { amount } => {
            write_add_mana_sentence(out, amount, SentenceCase::Upper);
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
        Statement::ReturnTargetCardFromYourZoneToZone {
            card_type,
            from,
            to,
        } => {
            out.push_str("Return target ");
            if let Some(card_type) = card_type {
                out.push_str(permanent_type_name(*card_type));
                out.push(' ');
            }
            out.push_str("card from your ");
            out.push_str(zone_name(*from));
            match to {
                Zone::Battlefield => out.push_str(" to the "),
                _ => out.push_str(" to your "),
            }
            out.push_str(zone_name(*to));
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
        Statement::Label { label } => {
            write_label_title(out, label);
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
        Statement::IfYouDoPreventDamageThisTurn { effect } => {
            write_if_you_do_effect(
                out,
                IfYouDoEffect::PreventDamageThisTurn { effect: *effect },
            );
        }
        Statement::IfYouDoAddMana { amount } => {
            write_if_you_do_effect(
                out,
                IfYouDoEffect::AddMana {
                    amount: amount.clone(),
                },
            );
        }
        Statement::IfYouDoUntap { source } => {
            write_if_you_do_effect(out, IfYouDoEffect::Untap { source: *source });
        }
        Statement::IfYouDoUntapReferencedPermanent { permanent_type } => {
            write_if_you_do_effect(
                out,
                IfYouDoEffect::UntapReferencedPermanent {
                    permanent_type: *permanent_type,
                },
            );
        }
        Statement::IfYouDoGainLife { amount } => {
            write_if_you_do_effect(out, IfYouDoEffect::GainLife { amount: *amount });
        }
        Statement::IfYouDoUntilYourNextTurnYouCantBeAttackedExceptByCreaturesWithKeywords {
            keywords,
        } => {
            out.push_str(
                "If you do, until your next turn, you can't be attacked except by creatures with ",
            );
            write_keyword_and_or_list(out, keywords);
            out.push('.');
        }
        Statement::IfYouDoCastThatCardFaceDownWithoutPayingManaCost { power, toughness } => {
            write!(
                out,
                "If you do, you may cast that card face down as a {power}/{toughness} creature spell without paying its mana cost."
            )
            .expect("write to String never fails");
        }
        Statement::IfFaceDownSpellCreatureWouldAssignOrDealDamageOrTapTurnFaceUpInstead => {
            out.push_str(
                "If the creature that spell becomes as it resolves has not been turned face up and would assign or deal damage, be dealt damage, or become tapped, instead it's turned face up and assigns or deals damage, is dealt damage, or becomes tapped.",
            );
        }
        Statement::TargetSpellOrPermanentBecomesColor { color } => {
            out.push_str("Target spell or permanent becomes ");
            out.push_str(color_name(*color));
            out.push('.');
        }
        Statement::TargetPermanentGetsUntilEndOfTurn { target, modifier } => {
            write_target_permanent_until_end_of_turn(
                out,
                *target,
                &TargetPermanentEndOfTurnEffect::gets_numbered(*modifier),
            );
        }
        Statement::TargetPermanentGetsMixedUntilEndOfTurn { target, modifier } => {
            write_target_permanent_until_end_of_turn(
                out,
                *target,
                &TargetPermanentEndOfTurnEffect::Gets(*modifier),
            );
        }
        Statement::TargetPermanentGainsKeywordUntilEndOfTurn { target, keyword } => {
            write_target_permanent_until_end_of_turn(
                out,
                *target,
                &TargetPermanentEndOfTurnEffect::GainsKeyword(*keyword),
            );
        }
        Statement::TargetPermanentGainsKeywordAndGetsUntilEndOfTurn {
            target,
            keyword,
            modifier,
            definitions,
        } => {
            write_target_permanent_until_end_of_turn(
                out,
                *target,
                &TargetPermanentEndOfTurnEffect::GainsKeywordAndGets {
                    keyword: *keyword,
                    modifier: *modifier,
                    definitions: definitions.clone(),
                },
            );
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
        Statement::ForEachAttackingCreatureChooseLabelBlockingRestriction { labels, keyword } => {
            out.push_str("Then, for each attacking creature you control, choose ");
            write_quoted_label(out, labels.first().map(String::as_str).unwrap_or(""));
            out.push_str(" or \"");
            out.push_str(labels.get(1).map(String::as_str).unwrap_or(""));
            out.push_str(
                ".\" That creature can't be blocked this combat except by creatures with ",
            );
            write_keyword_lowercase(out, *keyword);
            out.push_str(" and creatures in a pile with the chosen label.");
        }
        Statement::AsSourceEntersChoose { source, choice } => {
            out.push_str("As ");
            write_source_object(out, *source);
            out.push_str(" enters, choose ");
            write_as_enters_choice(out, *choice);
            out.push('.');
        }
        Statement::ThisPermanentEntersWithCounters {
            source,
            amount,
            counter,
        } => {
            write_source_object_capitalized(out, *source);
            out.push_str(" enters with ");
            write_counter_amount(out, *amount);
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
        Statement::OnlySourcesOwnerMayActivateThisAbility { source } => {
            out.push_str("Only ");
            write_source_object_possessive_without_apostrophe(out, *source);
            out.push_str(" owner may activate this ability.");
        }
        Statement::ActivateOnlyDuringYourUpkeep => {
            out.push_str("Activate only during your upkeep.");
        }
        Statement::ActivateOnlyDuringCombat => {
            out.push_str("Activate only during combat.");
        }
        Statement::ActivateOnlyDuringYourTurn => {
            out.push_str("Activate only during your turn.");
        }
        Statement::ActivateOnlyDuringYourTurnAndOnlyOnceEachTurn => {
            out.push_str("Activate only during your turn and only once each turn.");
        }
        Statement::ActivateOnlyDuringOpponentsTurnBeforeAttackersDeclared => {
            out.push_str("Activate only during an opponent's turn, before attackers are declared.");
        }
        Statement::ActivateOnlyAsSorcery => {
            out.push_str("Activate only as a sorcery.");
        }
        Statement::DestroyItAtBeginningOfNextEndStepIfItDidntAttackThisTurn => {
            out.push_str(
                "Destroy it at the beginning of the next end step if it didn't attack this turn.",
            );
        }
        Statement::ModalChoice { modes } => write_modal_choice(out, modes),
        Statement::StaticAbility(sa) => write_static_ability(out, sa),
        Statement::ActivatedAbility(aa) => write_activated_ability(out, aa),
        Statement::ActivatedAbilityWithActivationPermission {
            ability,
            permission,
        } => {
            write_activated_ability(out, ability);
            out.push(' ');
            write_activation_permission(out, *permission);
        }
        Statement::TriggeredAbility(ta) => write_triggered_ability(out, ta),
        Statement::PhysicalAction(pa) => write_physical_action(out, *pa),
        Statement::Compound(stmts) => {
            for (i, s) in stmts.iter().enumerate() {
                if i > 0 {
                    if statement_continues_previous_sentence(s) {
                        out.push(' ');
                    } else {
                        out.push('\n');
                    }
                }
                write_statement(out, s);
            }
        }
    }
}

fn statement_continues_previous_sentence(statement: &Statement) -> bool {
    matches!(
        statement,
        Statement::PlayerPaymentFailure { .. }
            | Statement::YouGainLifeEqualToDamage { .. }
            | Statement::NamedSourceDealsDamage { .. }
            | Statement::IgnoreThisEffectForEachCreaturePlayerDidntControlContinuouslySinceBeginningOfTurn
            | Statement::ForEachAttackingCreatureChooseLabelBlockingRestriction { .. }
    )
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
            write_colored_target_effect(out, ColoredTargetEffect::CounterSpell { color });
        }
        ModalMode::DestroyTargetColoredPermanent { color } => {
            write_colored_target_effect(out, ColoredTargetEffect::DestroyPermanent { color });
        }
        ModalMode::TargetPlayerGainsLife { amount } => {
            write_target_player_gains_life(out, amount);
        }
        ModalMode::PreventDamageThisTurn { effect } => {
            write_damage_prevention_effect(out, effect);
        }
    }
}

fn write_colored_target_effect(out: &mut String, effect: ColoredTargetEffect) {
    match effect {
        ColoredTargetEffect::CounterSpell { color } => {
            out.push_str("Counter target ");
            out.push_str(color_name(color));
            out.push_str(" spell.");
        }
        ColoredTargetEffect::DestroyPermanent { color } => {
            out.push_str("Destroy target ");
            out.push_str(color_name(color));
            out.push_str(" permanent.");
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

fn write_damage_life_gain_reference(out: &mut String, reference: &DamageLifeGainReference) {
    match reference {
        DamageLifeGainReference::DamageDealtCapped { caps } => {
            out.push_str("damage dealt, but not more life than ");
            write_damage_life_gain_caps(out, caps);
        }
        DamageLifeGainReference::DamageDealtToYouThisTurn => {
            out.push_str("damage dealt to you this turn");
        }
        DamageLifeGainReference::DamagePreventedThisWay => {
            out.push_str("damage prevented this way");
        }
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

fn write_damage_event_recipients(out: &mut String, recipients: &DamageRecipients) {
    match recipients {
        DamageRecipients::AnyTarget => out.push_str(" to any target"),
        DamageRecipients::DividedEvenlyRoundedDownAmongAnyNumberOfTargets => {
            out.push_str(" divided evenly, rounded down, among any number of targets");
        }
        DamageRecipients::List(recipients) => {
            out.push_str(" to ");
            write_damage_recipients(out, recipients);
        }
        DamageRecipients::Assignments(assignments) => write_damage_assignments(out, assignments),
    }
}

fn write_damage_assignments(out: &mut String, assignments: &[DamageAssignment<DamageRecipient>]) {
    let mut idx = 0;
    while idx < assignments.len() {
        if idx > 0 {
            out.push_str(" and ");
        }
        let assignment = &assignments[idx];
        write_damage_amount(out, assignment.amount);
        out.push_str(" damage to ");
        write_damage_recipient(out, assignment.recipient);
        let mut next_idx = idx + 1;
        while next_idx < assignments.len() && assignments[next_idx].amount == assignment.amount {
            out.push_str(" and ");
            write_damage_recipient(out, assignments[next_idx].recipient);
            next_idx += 1;
        }
        idx = next_idx;
    }
}

fn write_named_damage_event(out: &mut String, event: &NamedDamageEvent) {
    out.push_str(&event.source);
    out.push_str(" deals ");
    if let DamageRecipients::Assignments(assignments) = &event.recipient {
        write_damage_assignments(out, assignments);
        out.push('.');
        return;
    }
    if event.amount == DamageAmount::DamageDealtToYouThisTurn {
        out.push_str("damage");
        write_damage_event_recipients(out, &event.recipient);
        out.push_str(" equal to the ");
        write_damage_amount(out, event.amount);
        out.push('.');
        return;
    }
    write_damage_amount(out, event.amount);
    out.push_str(" damage");
    write_damage_event_recipients(out, &event.recipient);
    out.push('.');
}

fn write_damage_recipient(out: &mut String, recipient: DamageRecipient) {
    match recipient {
        DamageRecipient::AnyTarget => out.push_str("any target"),
        DamageRecipient::You => out.push_str("you"),
        DamageRecipient::TargetCreatureYouControl => out.push_str("target creature you control"),
        DamageRecipient::EachCreature => out.push_str("each creature"),
        DamageRecipient::EachCreatureWithKeyword { keyword } => {
            out.push_str("each creature with ");
            write_keyword(out, keyword);
        }
        DamageRecipient::EachCreatureWithoutKeyword { keyword } => {
            out.push_str("each creature without ");
            write_keyword(out, keyword);
        }
        DamageRecipient::EachPlayer => out.push_str("each player"),
        DamageRecipient::ThatPlayer => out.push_str("that player"),
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
        CastRestriction::DuringOpponentsTurnBeforeAttackersDeclared => {
            out.push_str("during an opponent's turn, before attackers are declared.");
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

fn write_if_you_do_effect(out: &mut String, effect: IfYouDoEffect) {
    write_if_you_do(out, |out| match effect {
        IfYouDoEffect::PreventDamageThisTurn { effect } => {
            write_damage_prevention_effect_lowercase(out, effect);
        }
        IfYouDoEffect::AddMana { amount } => {
            write_add_mana_sentence(out, &amount, SentenceCase::Lower)
        }
        IfYouDoEffect::Untap { source } => write_untap_sentence(out, source, SentenceCase::Lower),
        IfYouDoEffect::UntapReferencedPermanent { permanent_type } => {
            write_untap_referenced_permanent_sentence(out, permanent_type, SentenceCase::Lower);
        }
        IfYouDoEffect::GainLife { amount } => write_you_gain_life(out, amount, SentenceCase::Lower),
    });
}

fn write_if_you_do(out: &mut String, write_effect: impl FnOnce(&mut String)) {
    out.push_str("If you do, ");
    write_effect(out);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SentenceCase {
    Upper,
    Lower,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SentenceTemplate {
    upper_prefix: &'static str,
    lower_prefix: &'static str,
}

impl SentenceTemplate {
    const fn new(upper_prefix: &'static str, lower_prefix: &'static str) -> Self {
        Self {
            upper_prefix,
            lower_prefix,
        }
    }

    fn prefix(self, case: SentenceCase) -> &'static str {
        match case {
            SentenceCase::Upper => self.upper_prefix,
            SentenceCase::Lower => self.lower_prefix,
        }
    }
}

const ADD_MANA_SENTENCE: SentenceTemplate = SentenceTemplate::new("Add ", "add ");
const UNTAP_SENTENCE: SentenceTemplate = SentenceTemplate::new("Untap ", "untap ");
const UNTAP_REFERENCED_PERMANENT_SENTENCE: SentenceTemplate =
    SentenceTemplate::new("Untap the ", "untap the ");
const YOU_GAIN_LIFE_SENTENCE: SentenceTemplate = SentenceTemplate::new("You gain ", "you gain ");
const TARGET_PLAYER_GAINS_LIFE_SENTENCE: SentenceTemplate =
    SentenceTemplate::new("Target player gains ", "target player gains ");

fn write_template_sentence(
    out: &mut String,
    template: SentenceTemplate,
    case: SentenceCase,
    write_body: impl FnOnce(&mut String),
) {
    out.push_str(template.prefix(case));
    write_body(out);
    out.push('.');
}

fn write_add_mana_sentence(out: &mut String, amount: &AddManaAmount, case: SentenceCase) {
    write_template_sentence(out, ADD_MANA_SENTENCE, case, |out| {
        write_add_mana_amount(out, amount)
    });
}

fn write_add_mana_amount(out: &mut String, amount: &AddManaAmount) {
    match amount {
        AddManaAmount::Cost(mana) => write_mana_cost(out, mana),
        AddManaAmount::EqualToSacrificedPermanentManaValue {
            mana,
            permanent_type,
        } => {
            out.push_str("an amount of ");
            write_mana_symbol(out, *mana);
            out.push_str(" equal to the sacrificed ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str("'s mana value");
        }
    }
}

fn write_spell_additional_cost(out: &mut String, cost: SpellAdditionalCost) {
    match cost {
        SpellAdditionalCost::SacrificePermanent { permanent_type } => {
            out.push_str("sacrifice ");
            out.push_str(indefinite_article(permanent_type));
            out.push(' ');
            out.push_str(permanent_type_name(permanent_type));
        }
    }
}

fn write_untap_sentence(out: &mut String, source: SourceObject, case: SentenceCase) {
    write_template_sentence(out, UNTAP_SENTENCE, case, |out| {
        write_source_object(out, source);
    });
}

fn write_untap_referenced_permanent_sentence(
    out: &mut String,
    permanent_type: PermanentType,
    case: SentenceCase,
) {
    write_template_sentence(out, UNTAP_REFERENCED_PERMANENT_SENTENCE, case, |out| {
        out.push_str(permanent_type_name(permanent_type));
    });
}

fn write_you_gain_life(out: &mut String, amount: u32, case: SentenceCase) {
    write_template_sentence(out, YOU_GAIN_LIFE_SENTENCE, case, |out| {
        write!(out, "{amount} life").expect("write to String never fails");
    });
}

fn write_target_player_gains_life(out: &mut String, amount: u32) {
    write_template_sentence(
        out,
        TARGET_PLAYER_GAINS_LIFE_SENTENCE,
        SentenceCase::Upper,
        |out| {
            write!(out, "{amount} life").expect("write to String never fails");
        },
    );
}

fn write_damage_prevention_effect(
    out: &mut String,
    effect: DamagePreventionEffect<PreventionRecipient>,
) {
    write_damage_prevention_effect_with_prefix(out, effect, "Prevent ", write_prevention_recipient);
}

fn write_damage_prevention_effect_statement(
    out: &mut String,
    effect: DamagePreventionEffect<PreventionRecipient>,
    definitions: &[VariableDefinition],
) {
    write_damage_prevention_effect_without_period(
        out,
        effect,
        "Prevent ",
        write_prevention_recipient,
    );
    if !definitions.is_empty() {
        out.push_str(", where ");
        write_variable_definitions(out, definitions);
    }
    out.push('.');
}

fn write_damage_prevention_effect_lowercase(
    out: &mut String,
    effect: DamagePreventionEffect<PreventionRecipient>,
) {
    write_damage_prevention_effect_with_prefix(out, effect, "prevent ", write_prevention_recipient);
}

fn write_damage_prevention_effect_with_prefix<R: Copy>(
    out: &mut String,
    effect: DamagePreventionEffect<R>,
    prevention_verb: &str,
    write_recipient: impl FnMut(&mut String, R),
) {
    write_damage_prevention_effect_without_period(out, effect, prevention_verb, write_recipient);
    out.push('.');
}

fn write_damage_prevention_effect_without_period<R: Copy>(
    out: &mut String,
    effect: DamagePreventionEffect<R>,
    prevention_verb: &str,
    mut write_recipient: impl FnMut(&mut String, R),
) {
    write_damage_prevention_replacement_event(
        out,
        prevention_verb,
        effect.amount,
        effect.event,
        effect.kind,
    );
    if let Some(recipient) = effect.recipient {
        out.push_str(" to ");
        write_recipient(out, recipient);
    }
    write_damage_prevention_duration(out, effect.duration);
}

fn write_damage_prevention_duration(out: &mut String, duration: Option<DamagePreventionDuration>) {
    match duration {
        Some(DamagePreventionDuration::ThisTurn) => out.push_str(" this turn"),
        None => {}
    }
}

fn write_damage_prevention_replacement_event(
    out: &mut String,
    prevention_verb: &str,
    amount: DamagePreventionAmount,
    event: DamagePreventionEvent,
    kind: Option<DamageKind>,
) {
    out.push_str(prevention_verb);
    match amount {
        DamagePreventionAmount::All => out.push_str("all "),
        DamagePreventionAmount::Next(amount) => {
            out.push_str("the next ");
            write_damage_amount(out, amount);
            out.push(' ');
        }
        DamagePreventionAmount::Amount(amount) => {
            write_damage_amount(out, amount);
            out.push_str(" of ");
        }
    }
    match event {
        DamagePreventionEvent::ThatWouldBeDealt => {
            if let Some(kind) = kind {
                write_damage_kind_prefix(out, kind);
            }
            out.push_str("damage that would be dealt");
        }
        DamagePreventionEvent::OfThatDamage => out.push_str("that damage"),
    }
}

fn write_damage_kind_prefix(out: &mut String, kind: DamageKind) {
    match kind {
        DamageKind::Damage => {}
        DamageKind::CombatDamage => out.push_str("combat "),
    }
}

fn write_damage_amount(out: &mut String, amount: DamageAmount) {
    match amount {
        DamageAmount::Number(n) => write!(out, "{n}").expect("write to String never fails"),
        DamageAmount::Variable(variable) => out.push_str(variable_name(variable)),
        DamageAmount::DamageDealtToYouThisTurn => out.push_str("damage dealt to you this turn"),
        DamageAmount::ThatPermanentsToughness(permanent_type) => {
            out.push_str("equal to that ");
            out.push_str(permanent_type_name(permanent_type));
            out.push_str("'s toughness");
        }
        DamageAmount::NumberOfBasicLandsTheyControl(basic_land_type) => {
            out.push_str("equal to the number of ");
            out.push_str(basic_land_type_plural_name(basic_land_type));
            out.push_str(" they control");
        }
    }
}

fn write_prevention_recipient(out: &mut String, recipient: PreventionRecipient) {
    match recipient {
        PreventionRecipient::AnyTarget => out.push_str("any target"),
        PreventionRecipient::ThatPermanentOrPlayer => out.push_str("that permanent or player"),
    }
}

fn write_counter_unless_cost(out: &mut String, cost: &CounterUnlessCost) {
    match cost {
        CounterUnlessCost::ItsControllerPays(mana) => {
            out.push_str(" unless its controller pays ");
            write_mana_cost(out, mana);
        }
    }
}

fn write_payment_failure_effect(out: &mut String, effect: &PaymentFailureEffect) {
    match effect {
        PaymentFailureEffect::TapAllPermanentsAndLoseUnspentMana {
            permanent_type,
            with_mana_abilities,
        } => {
            out.push_str("they tap all ");
            out.push_str(permanent_type_plural_name(*permanent_type));
            if *with_mana_abilities {
                out.push_str(" with mana abilities");
            }
            out.push_str(" they control and lose all unspent mana");
        }
    }
}

fn write_pay_mana_player(out: &mut String, player: PayManaPlayer) {
    match player {
        PayManaPlayer::You => out.push_str("you"),
        PayManaPlayer::ThatPlayer => out.push_str("that player"),
    }
}

fn write_pay_mana_amount(out: &mut String, amount: &PayManaAmount) {
    match amount {
        PayManaAmount::Cost(cost) => write_mana_cost(out, cost),
        PayManaAmount::AnyAmountOfMana => out.push_str("any amount of mana"),
    }
}

fn write_keyword(out: &mut String, kw: Keyword) {
    match kw {
        Keyword::Named(name) => out.push_str(keyword_ability_title_name(name)),
        Keyword::Landwalk(land_type) => write_landwalk(out, land_type),
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

fn write_keyword_list(out: &mut String, keywords: &[Keyword]) {
    for (index, keyword) in keywords.iter().enumerate() {
        if index == 0 {
            write_keyword(out, *keyword);
        } else {
            out.push_str(", ");
            write_keyword_lowercase(out, *keyword);
        }
    }
}

fn write_semicolon_keyword_list(out: &mut String, keywords: &[Keyword]) {
    for (index, keyword) in keywords.iter().enumerate() {
        if index == 0 {
            write_keyword(out, *keyword);
        } else {
            out.push_str("; ");
            write_keyword_lowercase(out, *keyword);
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
        Zone::Hand => "hand",
        Zone::Ante => "ante",
        Zone::Battlefield => "battlefield",
    }
}

fn zone_article(zone: Zone) -> &'static str {
    match zone {
        Zone::Graveyard => "a",
        Zone::Hand => "a",
        Zone::Ante => "the",
        Zone::Battlefield => "the",
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
        ManaSymbol::Variable(v) => {
            write!(out, "{{{}}}", variable_name(v)).expect("write to String never fails")
        }
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

fn write_activation_permission(out: &mut String, permission: ActivationPermission) {
    match permission {
        ActivationPermission::OnlySourcesOwner { source } => {
            out.push_str("Only ");
            write_source_object_possessive_without_apostrophe(out, source);
            out.push_str(" owner may activate this ability.");
        }
        ActivationPermission::ActivateOnlyDuringYourUpkeep => {
            out.push_str("Activate only during your upkeep.");
        }
    }
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
        ActivatedCost::RemoveNamedCounterFromSource {
            counter_name,
            source,
        } => {
            out.push_str("Remove a ");
            out.push_str(counter_name);
            out.push_str(" counter from ");
            write_source_object(out, *source);
        }
    }
}

fn write_activated_effect(out: &mut String, effect: &ActivatedEffect) {
    match effect {
        ActivatedEffect::AddMana(amount) => {
            write_add_mana_sentence(out, amount, SentenceCase::Upper);
        }
        ActivatedEffect::AddOneManaOfAnyColor => {
            out.push_str("Add one mana of any color.");
        }
        ActivatedEffect::AddManaOfAnyOneColor { amount } => {
            out.push_str("Add ");
            out.push_str(u32_to_number_word(*amount));
            out.push_str(" mana of any one color.");
        }
        ActivatedEffect::TapTargetPermanentChoice { permanent_types } => {
            out.push_str("Tap target ");
            write_permanent_type_choice(out, permanent_types);
            out.push('.');
        }
        ActivatedEffect::Untap(source) => {
            write_untap_sentence(out, *source, SentenceCase::Upper);
        }
        ActivatedEffect::UntapTargetPermanent { permanent_type } => {
            out.push_str("Untap target ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push('.');
        }
        ActivatedEffect::UntapEnchanted(object) => {
            out.push_str("Untap enchanted ");
            write_enchanted_object(out, *object);
            out.push('.');
        }
        ActivatedEffect::Regenerate(recipient) => {
            out.push_str("Regenerate ");
            write_regenerate_recipient(out, *recipient);
            out.push('.');
        }
        ActivatedEffect::CounterTargetColoredSpell { color } => {
            write_colored_target_effect(out, ColoredTargetEffect::CounterSpell { color: *color });
        }
        ActivatedEffect::DestroyTargetColoredPermanent { color } => {
            write_colored_target_effect(out, ColoredTargetEffect::DestroyPermanent {
                color: *color,
            });
        }
        ActivatedEffect::Destroy { target } => write_destroy(out, target),
        ActivatedEffect::LookAtTargetPlayersHand => {
            out.push_str("Look at target player's hand.");
        }
        ActivatedEffect::DrawCards { count } => {
            out.push_str("Draw ");
            write_card_count_object(out, *count);
            out.push('.');
        }
        ActivatedEffect::TargetPlayerDiscardsCards { count } => {
            out.push_str("Target player discards ");
            write_discard_count(out, *count);
            out.push('.');
        }
        ActivatedEffect::ChooseTargetNonCreatureTypeCreatureActivePlayerControlledContinuouslySinceBeginningOfTurn {
            excluded_type,
        } => {
            out.push_str("Choose target non-");
            write_creature_type(out, *excluded_type);
            out.push_str(
                " creature the active player has controlled continuously since the beginning of the turn.",
            );
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
            write_target_permanent_until_end_of_turn(
                out,
                TargetPermanentSelector::Permanent(*permanent_type),
                &TargetPermanentEndOfTurnEffect::GainsKeyword(*keyword),
            );
        }
        ActivatedEffect::EnchantedGetsUntilEndOfTurn {
            permanent_type,
            modifier,
        } => {
            write_until_end_of_turn_sentence(
                out,
                |out| {
                    out.push_str("Enchanted ");
                    out.push_str(permanent_type_name(*permanent_type));
                },
                |out| write_gets_pt_modifier_clause(out, *modifier),
            );
        }
        ActivatedEffect::SourceGetsUntilEndOfTurn { source, modifier } => {
            write_until_end_of_turn_sentence(
                out,
                |out| write_source_object_capitalized(out, *source),
                |out| write_gets_pt_modifier_clause(out, *modifier),
            );
        }
        ActivatedEffect::SourceGainsKeywordUntilEndOfTurn { source, keyword } => {
            write_until_end_of_turn_sentence(
                out,
                |out| write_source_object_capitalized(out, *source),
                |out| write_gains_keyword_clause(out, *keyword),
            );
        }
        ActivatedEffect::SourceBecomesCreatureUntilEndOfCombat {
            source,
            power,
            toughness,
            creature_type,
            permanent_types,
        } => {
            write_source_object_capitalized(out, *source);
            write!(out, " becomes a {power}/{toughness} ").expect("writing to String cannot fail");
            write_creature_type(out, *creature_type);
            for permanent_type in permanent_types {
                out.push(' ');
                out.push_str(permanent_type_name(*permanent_type));
            }
            out.push_str(" until end of combat.");
        }
        ActivatedEffect::DamageEffect(effect) => write_activated_damage_effect(out, effect),
        ActivatedEffect::PutCountersOnSource {
            amount,
            up_to,
            counter,
            source,
        } => {
            out.push_str("Put ");
            if *up_to {
                out.push_str("up to ");
            }
            match *amount {
                CounterAmount::Number(1) if !*up_to => out.push('a'),
                _ => write_counter_amount(out, *amount),
            }
            out.push(' ');
            write_pt_modifier(out, *counter);
            if matches!(*amount, CounterAmount::Number(1)) && !*up_to {
                out.push_str(" counter on ");
            } else {
                out.push_str(" counters on ");
            }
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
        ActivatedEffect::ChooseCreatureCardInHandPayableByManaSpentOnVariable { variable } => {
            out.push_str(
                "You may choose a creature card in your hand whose mana cost could be paid by some amount of, or all of, the mana you spent on ",
            );
            write!(out, "{{{}}}.", variable_name(*variable)).expect("write to String never fails");
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
    write_card_count_object(out, count);
}

fn write_card_count_object(out: &mut String, count: CardCount) {
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
        StaticAbility::Conditional {
            order,
            condition,
            effect,
        } => {
            match order {
                ConditionalEffectOrder::ConditionThenEffect => {
                    out.push_str("As long as ");
                    write_condition(out, condition);
                    out.push_str(", ");
                    write_continuous_effect(out, effect);
                }
                ConditionalEffectOrder::EffectThenCondition => {
                    write_continuous_effect(out, effect);
                    out.push_str(" as long as ");
                    write_condition(out, condition);
                }
            }
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
            out.push_str(" is ");
            write_basic_land_type_reference(out, *land_type);
            out.push('.');
        }
        StaticAbility::EnchantedHasKeywordAndCantBeEnchantedByOtherAuras { object, keyword } => {
            out.push_str("Enchanted ");
            write_enchanted_object(out, *object);
            out.push_str(" has ");
            write_keyword_lowercase(out, *keyword);
            out.push_str(" and can't be enchanted by other Auras.");
        }
        StaticAbility::EnchantedCanAttackAsThoughItHad { object, keyword } => {
            out.push_str("Enchanted ");
            write_enchanted_object(out, *object);
            out.push_str(" can attack as though it had ");
            write_keyword_lowercase(out, *keyword);
            out.push('.');
        }
        StaticAbility::EnchantedCanAttackAsThoughItDidntHave { object, keyword } => {
            out.push_str("Enchanted ");
            write_enchanted_object(out, *object);
            out.push_str(" can attack as though it didn't have ");
            write_keyword_lowercase(out, *keyword);
            out.push('.');
        }
        StaticAbility::EnchantedCantBeBlockedExceptByCreatureType {
            object,
            except_type,
        } => {
            out.push_str("Enchanted ");
            write_enchanted_object(out, *object);
            out.push_str(" can't be blocked except by ");
            out.push_str(creature_type_plural_name(*except_type));
            out.push('.');
        }
        StaticAbility::AllCreaturesAbleToBlockEnchantedDoSo { object } => {
            out.push_str("All creatures able to block enchanted ");
            write_enchanted_object(out, *object);
            out.push_str(" do so.");
        }
        StaticAbility::YouControlEnchanted { object } => {
            out.push_str("You control enchanted ");
            write_enchanted_object(out, *object);
            out.push('.');
        }
        StaticAbility::YouHaveNoMaximumHandSize => {
            out.push_str("You have no maximum hand size.");
        }
        StaticAbility::YouDontLoseGameForHavingZeroOrLessLife => {
            out.push_str("You don't lose the game for having 0 or less life.");
        }
        StaticAbility::IfYouWouldGainLifeDrawThatManyCardsInstead => {
            out.push_str("If you would gain life, draw that many cards instead.");
        }
        StaticAbility::IfEffectCausesYouToDiscardCardYouMayPutItOnTopOfYourLibraryInstead => {
            out.push_str("If an effect causes you to discard a card, discard it, but you may put it on top of your library instead of into your graveyard.");
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
        StaticAbility::SourceEntersTapped { source } => {
            write_source_object_capitalized(out, *source);
            out.push_str(" enters tapped.");
        }
        StaticAbility::EffectDoesntRemoveThisAura => {
            out.push_str("This effect doesn't remove this Aura.");
        }
        StaticAbility::SourceAttacksEachCombatIfAble { source } => {
            write_source_object_capitalized(out, *source);
            out.push_str(" attacks each combat if able.");
        }
        StaticAbility::SourceCantAttackUnlessDefendingPlayerControlsBasicLand {
            source,
            land_type,
        } => {
            write_source_object_capitalized(out, *source);
            out.push_str(" can't attack unless defending player controls ");
            out.push_str(indefinite_article_for_basic_land_type(*land_type));
            out.push(' ');
            out.push_str(basic_land_type_name(*land_type));
            out.push('.');
        }
        StaticAbility::SourceCantBeBlockedByCreatureType { source, blocked_by } => {
            write_source_object_capitalized(out, *source);
            out.push_str(" can't be blocked by ");
            out.push_str(creature_type_plural_name(*blocked_by));
            out.push('.');
        }
        StaticAbility::SourceDoesntUntapDuringYourUntapStep { source } => {
            write_source_object_capitalized(out, *source);
            out.push_str(" doesn't untap during your untap step.");
        }
        StaticAbility::EnchantedDoesntUntapDuringItsControllersUntapStep { object } => {
            out.push_str("Enchanted ");
            write_enchanted_object(out, *object);
            out.push_str(" doesn't untap during its controller's untap step.");
        }
        StaticAbility::UntapRestrictionDuringUntapSteps { restriction } => {
            match restriction {
                StaticUntapRestriction::CreaturesWithPowerOrGreater { power } => {
                    write!(
                        out,
                        "Creatures with power {power} or greater don't untap during their controllers' untap steps."
                    )
                    .expect("writing to String cannot fail");
                }
                StaticUntapRestriction::PlayersCantUntapMoreThanPermanents {
                    amount,
                    permanent_type,
                } => {
                    out.push_str("Players can't untap more than ");
                    out.push_str(u32_to_number_word(*amount));
                    out.push(' ');
                    out.push_str(permanent_type_name(*permanent_type));
                    out.push_str(" during their untap steps.");
                }
            }
        }
        StaticAbility::SourceCantBlockCreaturesWithPowerOrGreater { source, power } => {
            write_source_object_capitalized(out, *source);
            write!(
                out,
                " can't block creatures with power {power} or greater."
            )
            .expect("writing to String cannot fail");
        }
        StaticAbility::NamedSourcePowerToughnessEachEqualToCount {
            source_name,
            count,
        } => {
            out.push_str(source_name);
            out.push_str("'s power and toughness are each equal to the number of ");
            match count {
                NamedSourcePowerToughnessCount::NonCreatureTypeCreatures { excluded_type } => {
                    out.push_str("non-");
                    out.push_str(creature_type_name(*excluded_type));
                    out.push_str(" creatures");
                }
                NamedSourcePowerToughnessCount::BasicLands { land_type } => {
                    out.push_str(basic_land_type_plural_name(*land_type));
                }
                NamedSourcePowerToughnessCount::CreaturesNamedOnTheBattlefield { name } => {
                    out.push_str("creatures named ");
                    out.push_str(name);
                    out.push_str(" on the battlefield");
                }
            }
            match count {
                NamedSourcePowerToughnessCount::NonCreatureTypeCreatures { .. }
                | NamedSourcePowerToughnessCount::BasicLands { .. } => {
                    out.push_str(" you control");
                }
                NamedSourcePowerToughnessCount::CreaturesNamedOnTheBattlefield { .. } => {}
            }
            out.push('.');
        }
        StaticAbility::BasicLandsAreBasicLands { from, to } => {
            out.push_str("All ");
            out.push_str(basic_land_type_plural_name(*from));
            out.push_str(" are ");
            out.push_str(basic_land_type_plural_name(*to));
            out.push('.');
        }
        StaticAbility::BasicLandsAreColoredCreaturesStillLands {
            land_type,
            power,
            toughness,
            color,
        } => {
            out.push_str("All ");
            out.push_str(basic_land_type_plural_name(*land_type));
            write!(out, " are {power}/{toughness} ").expect("writing to String cannot fail");
            if let Some(color) = color {
                out.push_str(color_name(*color));
                out.push(' ');
            }
            out.push_str(" creatures that are still lands.");
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
        StaticAbility::CreaturesAttackThisTurnIfAble { subject } => {
            match subject {
                AttackRequirementSubject::ThatCreature => out.push_str("That creature"),
                AttackRequirementSubject::CreaturesActivePlayerControls => {
                    out.push_str("Creatures the active player controls")
                }
            }
            match subject {
                AttackRequirementSubject::ThatCreature => out.push_str(" attacks this turn if able."),
                AttackRequirementSubject::CreaturesActivePlayerControls => {
                    out.push_str(" attack this turn if able.")
                }
            }
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
    write_trigger_condition(out, ta.condition());
    write_trigger_effect_sequence(out, &ta.effects);
}

fn write_trigger_condition(out: &mut String, condition: TriggerCondition) {
    out.push_str(match condition.event {
        TriggerEvent::PermanentEnters { .. }
        | TriggerEvent::PermanentPutIntoGraveyardFromBattlefield { .. }
        | TriggerEvent::PermanentDealtDamageBySourceThisTurnDies { .. }
        | TriggerEvent::YouPlayPermanent { .. }
        | TriggerEvent::PlayerCastsColoredSpell { .. }
        | TriggerEvent::PlayerTapsPermanentForMana { .. }
        | TriggerEvent::BasicLandTypeIsTappedForMana { .. }
        | TriggerEvent::BasicLandTypeControllerBecomesStatus { .. }
        | TriggerEvent::OneOrMoreCreaturesYouControlAttack
        | TriggerEvent::YouAreDealtDamage
        | TriggerEvent::SourceIsDealtDamage { .. }
        | TriggerEvent::SourceDealsDamageToAnOpponent { .. }
        | TriggerEvent::EnchantedObjectBecomesStatus { .. }
        | TriggerEvent::SourceBlocksOrBecomesBlockedByNonCreatureTypeCreature { .. } => "Whenever ",
        TriggerEvent::BeginningOfTheNextEndStep
        | TriggerEvent::BeginningOfChosenPlayersUpkeep
        | TriggerEvent::BeginningOfEachPlayersDrawStep
        | TriggerEvent::BeginningOfYourDrawStep
        | TriggerEvent::BeginningOfEachPlayersUpkeep
        | TriggerEvent::BeginningOfYourUpkeep
        | TriggerEvent::BeginningOfTheEndStep
        | TriggerEvent::BeginningOfEachEndStep
        | TriggerEvent::BeginningOfUpkeepOfEnchantedPermanentController { .. }
        | TriggerEvent::EndOfCombat => "At ",
        TriggerEvent::ThisAuraEnters
        | TriggerEvent::ThisAuraLeavesTheBattlefield
        | TriggerEvent::SourcePutIntoGraveyardFromBattlefield { .. }
        | TriggerEvent::SourceDies { .. }
        | TriggerEvent::YouControlNoBasicLands { .. } => "When ",
        TriggerEvent::EnchantedPermanentDies { .. } => "When ",
    });
    write_trigger_event(out, condition.event);
    out.push_str(", ");
    if let Some(iif) = condition.intervening_if {
        out.push_str("if ");
        write_intervening_if(out, iif);
        out.push_str(", ");
    }
}

fn write_trigger_effect_sequence(out: &mut String, effects: &[TriggerEffect]) {
    for (i, eff) in effects.iter().enumerate() {
        if i > 0 {
            if matches!(eff, TriggerEffect::SourceGainsStaticAbility { .. }) {
                out.push_str(" and ");
            } else {
                out.push(' ');
            }
        }
        let next_starts_sentence = effects
            .get(i + 1)
            .is_some_and(|next| matches!(next, TriggerEffect::PreventDamage { .. }));
        write_trigger_effect(
            out,
            eff,
            i + 1 == effects.len() || next_starts_sentence,
            i > 0,
        );
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
        TriggerEvent::PlayerTapsPermanentForMana { permanent_type } => {
            out.push_str("a player taps ");
            out.push_str(indefinite_article(permanent_type));
            out.push(' ');
            out.push_str(permanent_type_name(permanent_type));
            out.push_str(" for mana");
        }
        TriggerEvent::BasicLandTypeIsTappedForMana { land_type } => {
            out.push_str("a ");
            out.push_str(basic_land_type_name(land_type));
            out.push_str(" is tapped for mana");
        }
        TriggerEvent::BasicLandTypeControllerBecomesStatus {
            land_type,
            controller,
            status,
        } => {
            out.push_str("a ");
            out.push_str(basic_land_type_name(land_type));
            out.push(' ');
            write_permanent_controller(out, controller);
            out.push_str(" becomes ");
            out.push_str(object_status_name(status));
        }
        TriggerEvent::YouPlayPermanent { permanent_type } => {
            out.push_str("you play ");
            out.push_str(indefinite_article(permanent_type));
            out.push(' ');
            out.push_str(permanent_type_name(permanent_type));
        }
        TriggerEvent::OneOrMoreCreaturesYouControlAttack => {
            out.push_str("one or more creatures you control attack");
        }
        TriggerEvent::EnchantedPermanentDies { permanent_type } => {
            out.push_str("enchanted ");
            out.push_str(permanent_type_name(permanent_type));
            out.push_str(" dies");
        }
        TriggerEvent::SourceDies { source } => {
            write_source_object(out, source);
            out.push_str(" dies");
        }
        TriggerEvent::EnchantedObjectBecomesStatus { object, status } => {
            out.push_str("enchanted ");
            write_enchanted_object(out, object);
            out.push_str(" becomes ");
            out.push_str(object_status_name(status));
        }
        TriggerEvent::BeginningOfTheNextEndStep => {
            out.push_str("the beginning of the next end step");
        }
        TriggerEvent::BeginningOfTheEndStep => {
            out.push_str("the beginning of the end step");
        }
        TriggerEvent::BeginningOfEachEndStep => {
            out.push_str("the beginning of each end step");
        }
        TriggerEvent::BeginningOfChosenPlayersUpkeep => {
            out.push_str("the beginning of the chosen player's upkeep");
        }
        TriggerEvent::BeginningOfEachPlayersUpkeep => {
            out.push_str("the beginning of each player's upkeep");
        }
        TriggerEvent::BeginningOfEachPlayersDrawStep => {
            out.push_str("the beginning of each player's draw step");
        }
        TriggerEvent::BeginningOfYourDrawStep => {
            out.push_str("the beginning of your draw step");
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
        TriggerEvent::YouAreDealtDamage => {
            out.push_str("you're dealt damage");
        }
        TriggerEvent::PermanentPutIntoGraveyardFromBattlefield { permanent_type } => {
            out.push_str(indefinite_article(permanent_type));
            out.push(' ');
            out.push_str(permanent_type_name(permanent_type));
            out.push_str(" is put into a graveyard from the battlefield");
        }
        TriggerEvent::PermanentDealtDamageBySourceThisTurnDies {
            permanent_type,
            source,
        } => {
            out.push_str(indefinite_article(permanent_type));
            out.push(' ');
            out.push_str(permanent_type_name(permanent_type));
            out.push_str(" dealt damage by ");
            write_source_object(out, source);
            out.push_str(" this turn dies");
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
        TriggerEvent::SourceDealsDamageToAnOpponent { source } => {
            write_source_object(out, source);
            out.push_str(" deals damage to an opponent");
        }
        TriggerEvent::YouControlNoBasicLands { land_type } => {
            out.push_str("you control no ");
            out.push_str(basic_land_type_plural_name(land_type));
        }
    }
}

fn write_intervening_if(out: &mut String, iif: InterveningIf) {
    match iif {
        InterveningIf::ItsOnTheBattlefield => out.push_str("it's on the battlefield"),
        InterveningIf::NoPermanentsAreOnTheBattlefield { permanent_type } => {
            out.push_str("no ");
            out.push_str(permanent_type_plural_name(permanent_type));
            out.push_str(" are on the battlefield");
        }
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
        InterveningIf::SourceIsStatus { source, status } => {
            write_source_object(out, source);
            out.push_str(" is ");
            out.push_str(object_status_name(status));
        }
        InterveningIf::ThisCardInYourZoneWithCardsAboveIt {
            zone,
            count,
            card_type,
        } => {
            out.push_str("this card is in your ");
            out.push_str(zone_name(zone));
            out.push_str(" with ");
            out.push_str(u32_to_number_word(count));
            out.push_str(" or more ");
            out.push_str(permanent_type_name(card_type));
            out.push_str(" cards above it");
        }
    }
}

fn write_trigger_counter_recipient(out: &mut String, recipient: TriggerCounterRecipient) {
    match recipient {
        TriggerCounterRecipient::It => out.push_str("it"),
        TriggerCounterRecipient::Source(source) => write_source_object(out, source),
    }
}

fn write_trigger_effect(
    out: &mut String,
    eff: &TriggerEffect,
    terminal: bool,
    starts_sentence: bool,
) {
    match eff {
        TriggerEffect::DestroyThatCreatureIfItAttackedThisTurn => {
            out.push_str("destroy that creature if it attacked this turn.");
        }
        TriggerEffect::DestroyAllNonCreatureTypeCreaturesThatPlayerControlsThatDidntAttackThisTurn {
            excluded_type,
        } => {
            out.push_str("destroy all non-");
            write_creature_type(out, *excluded_type);
            out.push_str(" creatures that player controls that didn't attack this turn.");
        }
        TriggerEffect::DestroyIt => {
            out.push_str("destroy it.");
        }
        TriggerEffect::DestroyThatCreatureAtEndOfCombat => {
            out.push_str("destroy that creature at end of combat.");
        }
        TriggerEffect::ThatCreaturesControllerSacrificesIt => {
            out.push_str("that creature's controller sacrifices it.");
        }
        TriggerEffect::SourceDealsDamage(damage) => {
            write_triggered_damage(out, damage, terminal, starts_sentence);
        }
        TriggerEffect::ThatPlayerDrawsAnAdditionalCard => {
            out.push_str("that player draws an additional card.");
        }
        TriggerEffect::ThatPlayerDiscardsCardAtRandom => {
            out.push_str("that player discards a card at random.");
        }
        TriggerEffect::ThatPlayerAddsManaOfAnyTypeThatPermanentProduced {
            amount,
            permanent_type,
        } => {
            out.push_str("that player adds ");
            out.push_str(u32_to_number_word(*amount));
            out.push_str(" mana of any type that ");
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" produced.");
        }
        TriggerEffect::DefendingPlayerDividesCreaturesWithoutKeywordIntoLabeledPiles {
            keyword,
            labels,
        } => {
            out.push_str("each defending player divides all creatures without ");
            write_keyword_lowercase(out, *keyword);
            out.push_str(" they control into a ");
            write_quoted_label(out, labels.first().map(String::as_str).unwrap_or(""));
            out.push_str(" pile and a ");
            write_quoted_label(out, labels.get(1).map(String::as_str).unwrap_or(""));
            out.push_str(" pile.");
        }
        TriggerEffect::ItsControllerAddsAdditionalMana { mana } => {
            out.push_str("its controller adds an additional ");
            write_mana_symbol(out, *mana);
            out.push('.');
        }
        TriggerEffect::RemoveCounterFromIt { counter } => {
            out.push_str("remove a ");
            write_pt_modifier(out, *counter);
            out.push_str(" counter from it.");
        }
        TriggerEffect::PutCounter { counter, recipient } => {
            out.push_str("put a ");
            write_pt_modifier(out, *counter);
            out.push_str(" counter on ");
            write_trigger_counter_recipient(out, *recipient);
            out.push('.');
        }
        TriggerEffect::PutNamedCountersOnSource {
            amount,
            counter_name,
            source,
        } => {
            out.push_str("put ");
            match amount {
                NamedCounterAmount::ThatMany => {
                    write_named_counter_amount(out, *amount, counter_name);
                    out.push_str(" on ");
                    write_source_object(out, *source);
                }
                NamedCounterAmount::OneForEachPermanentThatDiedThisTurn { permanent_type } => {
                    out.push_str("a ");
                    out.push_str(counter_name);
                    out.push_str(" counter on ");
                    write_source_object(out, *source);
                    out.push_str(" for each ");
                    out.push_str(permanent_type_name(*permanent_type));
                    out.push_str(" that died this turn");
                }
            }
            out.push('.');
        }
        TriggerEffect::YouMayRemoveNamedCounterFromSource {
            counter_name,
            source,
        } => {
            out.push_str("you may remove a ");
            out.push_str(counter_name);
            out.push_str(" counter from ");
            write_source_object(out, *source);
            out.push('.');
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
        TriggerEffect::SacrificeSource { source } => {
            out.push_str("sacrifice ");
            write_source_object(out, *source);
            out.push('.');
        }
        TriggerEffect::SacrificePermanentOtherThanSource {
            permanent_type,
            source,
        } => {
            out.push_str("sacrifice ");
            out.push_str(indefinite_article(*permanent_type));
            out.push(' ');
            out.push_str(permanent_type_name(*permanent_type));
            out.push_str(" other than ");
            write_source_object(out, *source);
            out.push('.');
        }
        TriggerEffect::SacrificeThatManyNontokenPermanents => {
            out.push_str("sacrifice that many nontoken permanents.");
        }
        TriggerEffect::YouLoseTheGame => {
            out.push_str("you lose the game.");
        }
        TriggerEffect::YouGainLife { amount } => {
            write_you_gain_life(out, *amount, SentenceCase::Lower);
        }
        TriggerEffect::PlayerLosesLife { player, amount } => {
            write_life_loss_player(out, *player);
            out.push_str(" loses ");
            write_life_loss_amount(out, *amount);
            out.push_str(" life");
            if let LifeLossAmount::HalfTheirLife { rounding } = *amount {
                out.push_str(", rounded ");
                out.push_str(rounding_name(rounding));
            }
            out.push('.');
        }
        TriggerEffect::YouMayPayMana { player, amount } => {
            write_pay_mana_player(out, *player);
            out.push_str(" may pay ");
            write_pay_mana_amount(out, amount);
            out.push('.');
        }
        TriggerEffect::PreventDamage {
            effect,
            definitions,
        } => {
            write_damage_prevention_effect_statement(out, *effect, definitions);
        }
        TriggerEffect::TapEnchanted(object) => {
            out.push_str("tap enchanted ");
            write_enchanted_object(out, *object);
            out.push('.');
        }
        TriggerEffect::YouMayPutThisCardOntoTheBattlefield => {
            out.push_str("you may put this card onto the battlefield.");
        }
        TriggerEffect::IfYouDoGainLife { amount } => {
            write_if_you_do_effect(out, IfYouDoEffect::GainLife { amount: *amount });
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
            out.push_str(
                "at the beginning of each of your upkeeps for the rest of the game, remove all ",
            );
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

fn write_counter_amount(out: &mut String, amount: CounterAmount) {
    match amount {
        CounterAmount::Number(n) => out.push_str(u32_to_number_word(n)),
        CounterAmount::Variable(variable) => out.push_str(variable_name(variable)),
    }
}

fn write_named_counter_amount(out: &mut String, amount: NamedCounterAmount, counter_name: &str) {
    match amount {
        NamedCounterAmount::ThatMany => {
            out.push_str("that many ");
            out.push_str(counter_name);
            out.push_str(" counters");
        }
        NamedCounterAmount::OneForEachPermanentThatDiedThisTurn { permanent_type } => {
            out.push_str("a ");
            out.push_str(counter_name);
            out.push_str(" counter for each ");
            out.push_str(permanent_type_name(permanent_type));
            out.push_str(" that died this turn");
        }
    }
}

fn write_triggered_damage(
    out: &mut String,
    damage: &TriggeredDamage,
    terminal: bool,
    starts_sentence: bool,
) {
    match damage.event.source {
        TriggerDamageSource::Source(source) => {
            if starts_sentence {
                write_source_object_capitalized(out, source);
            } else {
                write_source_object(out, source);
            }
        }
        TriggerDamageSource::It => out.push_str("it"),
    }
    out.push_str(" deals ");
    match damage.event.amount {
        DamageAmount::Number(_) | DamageAmount::Variable(_) => {
            let amount = damage.event.amount;
            write_damage_amount(out, amount);
            out.push_str(" damage to ");
            write_trigger_damage_recipient(out, damage.event.recipient);
        }
        DamageAmount::DamageDealtToYouThisTurn => {
            out.push_str("damage to ");
            write_trigger_damage_recipient(out, damage.event.recipient);
            out.push_str(" equal to ");
            write_damage_amount(out, damage.event.amount);
        }
        DamageAmount::ThatPermanentsToughness(permanent_type) => {
            out.push_str("damage equal to that ");
            out.push_str(permanent_type_name(permanent_type));
            out.push_str("'s toughness to ");
            write_trigger_damage_recipient(out, damage.event.recipient);
        }
        DamageAmount::NumberOfBasicLandsTheyControl(basic_land_type) => {
            out.push_str("damage to ");
            write_trigger_damage_recipient(out, damage.event.recipient);
            out.push_str(" equal to the number of ");
            out.push_str(basic_land_type_plural_name(basic_land_type));
            out.push_str(" they control");
        }
    }
    if let Some(condition) = &damage.condition {
        match condition {
            TriggerDamageCondition::UnlessYouPay(cost) => {
                out.push_str(" unless you pay ");
                write_mana_cost(out, cost);
            }
        }
    }
    match damage.event.amount {
        DamageAmount::Variable(_) => {
            out.push_str(", where ");
            write_variable_definitions(out, &damage.definitions);
            out.push('.');
        }
        DamageAmount::Number(_)
        | DamageAmount::DamageDealtToYouThisTurn
        | DamageAmount::ThatPermanentsToughness(_)
        | DamageAmount::NumberOfBasicLandsTheyControl(_) => {
            if terminal {
                out.push('.');
            }
        }
    }
}

fn write_trigger_damage_recipient(out: &mut String, recipient: TriggerDamageRecipient) {
    match recipient {
        TriggerDamageRecipient::You => out.push_str("you"),
        TriggerDamageRecipient::ThatPlayer => out.push_str("that player"),
        TriggerDamageRecipient::ThatPermanent(permanent_type) => {
            out.push_str("that ");
            out.push_str(permanent_type_name(permanent_type));
        }
        TriggerDamageRecipient::ThatPermanentController(permanent_type) => {
            out.push_str("that ");
            out.push_str(permanent_type_name(permanent_type));
            out.push_str("'s controller");
        }
    }
}

fn write_life_loss_player(out: &mut String, player: LifeLossPlayer) {
    match player {
        LifeLossPlayer::ItsOwner => out.push_str("its owner"),
    }
}

fn write_life_loss_amount(out: &mut String, amount: LifeLossAmount) {
    match amount {
        LifeLossAmount::Number(n) => write!(out, "{n}").expect("write to String never fails"),
        LifeLossAmount::HalfTheirLife { .. } => out.push_str("half their"),
    }
}

fn write_activated_damage_effect(out: &mut String, effect: &ActivatedDamageEffect) {
    match effect {
        ActivatedDamageEffect::SourceDealsDamage {
            source,
            assignments,
        } => {
            if !assignments.is_empty() {
                write_source_object_capitalized(out, *source);
                out.push_str(" deals ");
                let mut idx = 0;
                while idx < assignments.len() {
                    if idx > 0 {
                        out.push_str(" and ");
                    }
                    let assignment = &assignments[idx];
                    write_damage_amount(out, assignment.amount);
                    out.push_str(" damage to ");
                    write_activated_damage_recipient(out, assignment.recipient);
                    let mut next_idx = idx + 1;
                    while next_idx < assignments.len()
                        && assignments[next_idx].amount == assignment.amount
                    {
                        out.push_str(" and ");
                        write_activated_damage_recipient(out, assignments[next_idx].recipient);
                        next_idx += 1;
                    }
                    idx = next_idx;
                }
                out.push('.');
            }
        }
        ActivatedDamageEffect::NextDamageEvent { event, effect } => {
            out.push_str("The next time ");
            write_activated_damage_source(out, event.source);
            out.push_str(" of your choice would deal ");
            if event.kind == DamageKind::CombatDamage {
                out.push_str("combat ");
            }
            out.push_str("damage to ");
            write_activated_damage_recipient(out, event.recipient);
            out.push_str(" this turn, ");
            write_activated_damage_event_effect(out, *effect);
            out.push('.');
        }
        ActivatedDamageEffect::RedirectNextDamageThisTurn {
            amount,
            kind,
            recipient,
            destination,
        } => {
            out.push_str("The next ");
            write_damage_amount(out, *amount);
            out.push(' ');
            if let Some(kind) = kind {
                write_damage_kind_prefix(out, *kind);
            }
            out.push_str("damage that would be dealt to ");
            write_activated_damage_recipient(out, *recipient);
            out.push_str(" this turn is dealt to ");
            write_damage_redirection_destination(out, *destination);
            out.push_str(" instead.");
        }
        ActivatedDamageEffect::PreventDamageThisTurn { effect } => {
            write_activated_damage_prevention_effect(out, *effect);
        }
    }
}

fn write_activated_damage_prevention_effect(
    out: &mut String,
    effect: DamagePreventionEffect<ActivatedDamageRecipient>,
) {
    write_damage_prevention_effect_with_prefix(
        out,
        effect,
        "Prevent ",
        write_activated_damage_recipient,
    );
}

fn write_activated_damage_source(out: &mut String, source: ActivatedDamageSource) {
    match source {
        ActivatedDamageSource::ColoredSource { color } => {
            out.push_str(color_article(color));
            out.push(' ');
            out.push_str(color_name(color));
            out.push_str(" source");
        }
        ActivatedDamageSource::UnblockedCreature => out.push_str("an unblocked creature"),
        ActivatedDamageSource::Source => out.push_str("a source"),
    }
}

fn write_activated_damage_recipient(out: &mut String, recipient: ActivatedDamageRecipient) {
    match recipient {
        ActivatedDamageRecipient::You => out.push_str("you"),
        ActivatedDamageRecipient::AnyTarget => out.push_str("any target"),
        ActivatedDamageRecipient::EachCreature => out.push_str("each creature"),
        ActivatedDamageRecipient::EachPlayer => out.push_str("each player"),
        ActivatedDamageRecipient::TargetPermanent { permanent_type } => {
            out.push_str("target ");
            out.push_str(permanent_type_name(permanent_type));
        }
        ActivatedDamageRecipient::Source(source) => write_source_object(out, source),
    }
}

fn write_damage_redirection_destination(
    out: &mut String,
    destination: DamageRedirectionDestination,
) {
    match destination {
        DamageRedirectionDestination::ItsOwner => out.push_str("its owner"),
    }
}

fn write_activated_damage_event_effect(out: &mut String, effect: ActivatedDamageEventEffect) {
    match effect {
        ActivatedDamageEventEffect::PreventThatDamage => out.push_str("prevent that damage"),
        ActivatedDamageEventEffect::PreventAllBut { amount } => {
            write!(out, "prevent all but {amount} of that damage")
                .expect("writing to String cannot fail");
        }
        ActivatedDamageEventEffect::RedirectToYou => {
            out.push_str("that source deals that damage to you instead");
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

fn write_regenerate_recipient(out: &mut String, recipient: RegenerateRecipient) {
    match recipient {
        RegenerateRecipient::Source(source) => write_source_object(out, source),
        RegenerateRecipient::Enchanted(object) => {
            out.push_str("enchanted ");
            write_enchanted_object(out, object);
        }
    }
}

fn write_source_object_possessive_without_apostrophe(out: &mut String, source: SourceObject) {
    match source {
        SourceObject::This(pt) => {
            out.push_str("this ");
            out.push_str(permanent_type_name(pt));
            out.push('s');
        }
        SourceObject::ThisAura => out.push_str("this Auras"),
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

fn write_as_enters_choice(out: &mut String, choice: AsEntersChoice) {
    match choice {
        AsEntersChoice::Opponent => out.push_str("an opponent"),
        AsEntersChoice::BasicLandType => out.push_str("a basic land type"),
    }
}

fn write_label_title(out: &mut String, label: &str) {
    let mut chars = label.chars();
    if let Some(first) = chars.next() {
        out.extend(first.to_uppercase());
        out.push_str(chars.as_str());
    }
}

fn write_quoted_label(out: &mut String, label: &str) {
    out.push('"');
    out.push_str(label);
    out.push('"');
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
        Keyword::Named(name) => out.push_str(keyword_ability_name(name)),
        Keyword::Landwalk(land_type) => write_landwalk_lowercase(out, land_type),
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

fn write_landwalk(out: &mut String, land_type: BasicLandType) {
    out.push_str(basic_land_type_name(land_type));
    out.push_str("walk");
}

fn write_landwalk_lowercase(out: &mut String, land_type: BasicLandType) {
    out.push_str(basic_land_type_lowercase_name(land_type));
    out.push_str("walk");
}

fn keyword_ability_title_name(keyword: NamedKeywordAbility) -> &'static str {
    match keyword {
        NamedKeywordAbility::FirstStrike => "First strike",
        NamedKeywordAbility::Flying => "Flying",
        NamedKeywordAbility::Reach => "Reach",
        NamedKeywordAbility::Haste => "Haste",
        NamedKeywordAbility::Defender => "Defender",
        NamedKeywordAbility::Banding => "Banding",
        NamedKeywordAbility::Trample => "Trample",
        NamedKeywordAbility::Indestructible => "Indestructible",
        NamedKeywordAbility::Fear => "Fear",
        NamedKeywordAbility::Vigilance => "Vigilance",
    }
}

fn keyword_ability_name(keyword: NamedKeywordAbility) -> &'static str {
    match keyword {
        NamedKeywordAbility::FirstStrike => "first strike",
        NamedKeywordAbility::Flying => "flying",
        NamedKeywordAbility::Reach => "reach",
        NamedKeywordAbility::Haste => "haste",
        NamedKeywordAbility::Defender => "defender",
        NamedKeywordAbility::Banding => "banding",
        NamedKeywordAbility::Trample => "trample",
        NamedKeywordAbility::Indestructible => "indestructible",
        NamedKeywordAbility::Fear => "fear",
        NamedKeywordAbility::Vigilance => "vigilance",
    }
}

fn write_keyword_and_or_list(out: &mut String, keywords: &[Keyword]) {
    for (index, keyword) in keywords.iter().enumerate() {
        if index > 0 {
            out.push_str(" and/or ");
        }
        write_keyword_lowercase(out, *keyword);
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
        ValueExpression::NumberOfStatusPermanentsTheyControlledAtBeginningOfThisTurn {
            status,
            permanent_type,
        } => {
            out.push_str("the number of ");
            out.push_str(object_status_name(*status));
            out.push(' ');
            out.push_str(permanent_type_plural_name(*permanent_type));
            out.push_str(" they controlled at the beginning of this turn");
        }
        ValueExpression::AmountOfManaThatPlayerPaidThisWay => {
            out.push_str("the amount of mana that player paid this way");
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
        Condition::YouControlBasicLand { land_type } => {
            out.push_str("you control ");
            out.push_str(indefinite_article_for_basic_land_type(*land_type));
            out.push(' ');
            out.push_str(basic_land_type_name(*land_type));
        }
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
        ContinuousEffect::SourceGets { source, modifier } => {
            write_source_object_capitalized(out, *source);
            out.push_str(" gets ");
            write_pt_modifier(out, *modifier);
        }
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

fn write_destroy(out: &mut String, target: &DestroyTarget) {
    out.push_str("Destroy ");
    write_destroy_target(out, target);
    out.push('.');
}

fn write_destroy_target(out: &mut String, target: &DestroyTarget) {
    match target {
        DestroyTarget::TargetPermanents(permanent_types) => {
            out.push_str("target ");
            write_permanent_type_choice(out, permanent_types);
        }
        DestroyTarget::TargetColoredPermanent(color) => {
            out.push_str("target ");
            out.push_str(color_name(*color));
            out.push_str(" permanent");
        }
        DestroyTarget::TargetStatusCreature(status) => {
            out.push_str("target ");
            out.push_str(creature_status_name(*status));
            out.push_str(" creature");
        }
        DestroyTarget::TargetCreatureType(creature_type) => {
            out.push_str("target ");
            write_creature_type(out, *creature_type);
        }
        DestroyTarget::AllPermanents(permanent_types) => {
            out.push_str("all ");
            write_permanent_type_plural_list(out, permanent_types);
        }
        DestroyTarget::AllBasicLands(basic_land_type) => {
            out.push_str("all ");
            out.push_str(basic_land_type_plural_name(*basic_land_type));
        }
    }
}

fn write_target_permanent_until_end_of_turn(
    out: &mut String,
    target: TargetPermanentSelector,
    effect: &TargetPermanentEndOfTurnEffect,
) {
    match effect {
        TargetPermanentEndOfTurnEffect::Gets(modifier) => {
            write_until_end_of_turn_sentence(
                out,
                |out| write_target_permanent_subject(out, target),
                |out| write_gets_mixed_pt_modifier_clause(out, *modifier),
            );
        }
        TargetPermanentEndOfTurnEffect::GainsKeyword(keyword) => {
            write_until_end_of_turn_sentence(
                out,
                |out| write_target_permanent_subject(out, target),
                |out| write_gains_keyword_clause(out, *keyword),
            );
        }
        TargetPermanentEndOfTurnEffect::GainsKeywordAndGets {
            keyword,
            modifier,
            definitions,
        } => {
            write_until_end_of_turn_sentence_with_tail(
                out,
                |out| write_target_permanent_subject(out, target),
                |out| {
                    write_gains_keyword_clause(out, *keyword);
                    out.push_str(" and gets ");
                    write_mixed_pt_modifier(out, *modifier);
                },
                UntilEndOfTurnTail::Where(definitions),
            );
        }
    }
}

fn write_target_permanent_subject(out: &mut String, target: TargetPermanentSelector) {
    out.push_str("Target ");
    write_target_permanent_selector(out, target);
}

fn write_target_permanent_selector(out: &mut String, target: TargetPermanentSelector) {
    match target {
        TargetPermanentSelector::Permanent(permanent_type) => {
            out.push_str(permanent_type_name(permanent_type));
        }
        TargetPermanentSelector::CombatRoleCreature { role } => {
            out.push_str(combat_role_name(role));
            out.push_str(" creature");
        }
    }
}

fn combat_role_name(role: CombatRole) -> &'static str {
    match role {
        CombatRole::Attacking => "attacking",
        CombatRole::Blocking => "blocking",
    }
}

fn write_until_end_of_turn_sentence(
    out: &mut String,
    write_subject: impl FnOnce(&mut String),
    write_effect: impl FnOnce(&mut String),
) {
    write_until_end_of_turn_sentence_with_tail(
        out,
        write_subject,
        write_effect,
        UntilEndOfTurnTail::Period,
    );
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum UntilEndOfTurnTail<'a> {
    Period,
    Where(&'a [VariableDefinition]),
}

fn write_until_end_of_turn_sentence_with_tail(
    out: &mut String,
    write_subject: impl FnOnce(&mut String),
    write_effect: impl FnOnce(&mut String),
    tail: UntilEndOfTurnTail<'_>,
) {
    write_subject(out);
    out.push(' ');
    write_effect(out);
    write_until_end_of_turn_tail(out, tail);
}

fn write_until_end_of_turn_tail(out: &mut String, tail: UntilEndOfTurnTail<'_>) {
    out.push_str(" until end of turn");
    match tail {
        UntilEndOfTurnTail::Period => out.push('.'),
        UntilEndOfTurnTail::Where(definitions) => {
            out.push_str(", where ");
            write_variable_definitions(out, definitions);
            out.push('.');
        }
    }
}

fn write_gets_pt_modifier_clause(out: &mut String, modifier: PtModifier) {
    out.push_str("gets ");
    write_pt_modifier(out, modifier);
}

fn write_gets_mixed_pt_modifier_clause(out: &mut String, modifier: MixedPtModifier) {
    out.push_str("gets ");
    write_mixed_pt_modifier(out, modifier);
}

fn write_gains_keyword_clause(out: &mut String, keyword: Keyword) {
    out.push_str("gains ");
    write_keyword_lowercase(out, keyword);
}

fn write_permanent_type_plural_list(out: &mut String, permanent_types: &[PermanentType]) {
    for (index, permanent_type) in permanent_types.iter().enumerate() {
        if index > 0 {
            if index == permanent_types.len() - 1 {
                if permanent_types.len() > 2 {
                    out.push_str(", and ");
                } else {
                    out.push_str(" and ");
                }
            } else {
                out.push_str(", ");
            }
        }
        out.push_str(permanent_type_plural_name(*permanent_type));
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
        CreatureStatus::Attacking => "Attacking",
        CreatureStatus::Tapped => "Tapped",
        CreatureStatus::Untapped => "Untapped",
    }
}

fn text_change_replacement_term_name(term: TextChangeReplacementTerm) -> &'static str {
    match term {
        TextChangeReplacementTerm::BasicLandType => "basic land type",
        TextChangeReplacementTerm::ColorWord => "color word",
    }
}

fn creature_status_name(status: CreatureStatus) -> &'static str {
    match status {
        CreatureStatus::Attacking => "attacking",
        CreatureStatus::Tapped => "tapped",
        CreatureStatus::Untapped => "untapped",
    }
}

fn object_status_name(status: ObjectStatus) -> &'static str {
    match status {
        ObjectStatus::Tapped => "tapped",
        ObjectStatus::Untapped => "untapped",
    }
}

fn write_permanent_controller(out: &mut String, controller: PermanentController) {
    match controller {
        PermanentController::You => out.push_str("you control"),
        PermanentController::Opponent => out.push_str("an opponent controls"),
    }
}

fn creature_type_name(ct: CreatureType) -> &'static str {
    match ct {
        CreatureType::Goblin => "Goblin",
        CreatureType::Golem => "Golem",
        CreatureType::Merfolk => "Merfolk",
        CreatureType::Wall => "Wall",
    }
}

fn creature_type_plural_name(ct: CreatureType) -> &'static str {
    match ct {
        CreatureType::Goblin => "Goblins",
        CreatureType::Golem => "Golems",
        CreatureType::Merfolk => "Merfolk",
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

fn indefinite_article_for_basic_land_type(land_type: BasicLandType) -> &'static str {
    match land_type {
        BasicLandType::Island => "an",
        BasicLandType::Plains
        | BasicLandType::Swamp
        | BasicLandType::Mountain
        | BasicLandType::Forest => "a",
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

fn write_basic_land_type_reference(out: &mut String, land_type: BasicLandTypeReference) {
    match land_type {
        BasicLandTypeReference::Specific(basic_land_type) => {
            out.push_str("a ");
            out.push_str(basic_land_type_name(basic_land_type));
        }
        BasicLandTypeReference::ChosenType => out.push_str("the chosen type"),
    }
}

fn basic_land_type_lowercase_name(land_type: BasicLandType) -> &'static str {
    match land_type {
        BasicLandType::Plains => "plains",
        BasicLandType::Island => "island",
        BasicLandType::Swamp => "swamp",
        BasicLandType::Mountain => "mountain",
        BasicLandType::Forest => "forest",
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
