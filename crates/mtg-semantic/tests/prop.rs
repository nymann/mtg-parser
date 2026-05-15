// Tier 2 lowering property tests.
//
// Totality (M4 exit criterion): every AST the parser can produce must
// lower without error.
//
// AST strategies are intentionally duplicated from
// crates/mtg-grammar/tests/prop.rs. When the AST grows enough that
// drift between the two copies becomes painful, lift the strategies
// into a feature-gated `mtg_grammar::testing` module and update the
// xtask runner to enable that feature for tier 2.

use mtg_grammar::{
    ActivatedAbility, ActivatedCost, ActivatedEffect, BasicLandType, CardCount, Color,
    DamageAmount, DamageEvent, DamageKind, DamageLifeGainCap, DamagePreventionAmount,
    DamagePreventionDuration, DamagePreventionEffect, DamageRecipient, DamageRecipients,
    EachPlayerAction, EnchantedObject, ImperativeAction, Keyword, ManaCost, ManaSymbol, ModalMode,
    PermanentType, PreventionRecipient, PtModifier, Sign, SignedNumber, SignedPtComponent,
    SignedVariable, SourceObject, SpellType, Statement, StaticAbility, TriggerEffect, TriggerEvent,
    TriggeredAbility, Variable, Zone,
};
use mtg_semantic::{lower, CardEffect};
use proptest::prelude::*;

fn arb_mana_symbol() -> impl Strategy<Value = ManaSymbol> {
    prop_oneof![
        (0u32..=999).prop_map(ManaSymbol::Generic),
        Just(ManaSymbol::White),
        Just(ManaSymbol::Blue),
        Just(ManaSymbol::Black),
        Just(ManaSymbol::Red),
        Just(ManaSymbol::Green),
        Just(ManaSymbol::Colorless),
    ]
}

fn arb_mana_cost() -> impl Strategy<Value = ManaCost> {
    prop::collection::vec(arb_mana_symbol(), 1..16).prop_map(|symbols| ManaCost { symbols })
}

fn arb_card_count() -> impl Strategy<Value = CardCount> {
    prop_oneof![
        (1u32..=10).prop_map(CardCount::Number),
        arb_variable().prop_map(CardCount::Variable),
    ]
}

fn arb_color() -> impl Strategy<Value = Color> {
    prop_oneof![
        Just(Color::White),
        Just(Color::Blue),
        Just(Color::Black),
        Just(Color::Red),
        Just(Color::Green),
    ]
}

fn arb_spell_type() -> impl Strategy<Value = SpellType> {
    prop_oneof![Just(SpellType::Instant), Just(SpellType::Sorcery)]
}

fn arb_permanent_type() -> impl Strategy<Value = PermanentType> {
    prop_oneof![
        Just(PermanentType::Artifact),
        Just(PermanentType::Creature),
        Just(PermanentType::Enchantment),
        Just(PermanentType::Land),
        Just(PermanentType::Planeswalker),
    ]
}

fn arb_basic_land_type() -> impl Strategy<Value = BasicLandType> {
    prop_oneof![
        Just(BasicLandType::Plains),
        Just(BasicLandType::Island),
        Just(BasicLandType::Swamp),
        Just(BasicLandType::Mountain),
        Just(BasicLandType::Forest),
    ]
}

fn arb_signed_number() -> impl Strategy<Value = SignedNumber> {
    (prop_oneof![Just(Sign::Plus), Just(Sign::Minus)], 0u32..=10)
        .prop_map(|(sign, magnitude)| SignedNumber { sign, magnitude })
}

fn arb_pt_modifier() -> impl Strategy<Value = PtModifier> {
    (arb_signed_number(), arb_signed_number())
        .prop_map(|(power, toughness)| PtModifier { power, toughness })
}

fn arb_variable() -> impl Strategy<Value = Variable> {
    prop_oneof![Just(Variable::X), Just(Variable::Y)]
}

fn arb_signed_variable() -> impl Strategy<Value = SignedVariable> {
    (
        prop_oneof![Just(Sign::Plus), Just(Sign::Minus)],
        arb_variable(),
    )
        .prop_map(|(sign, variable)| SignedVariable { sign, variable })
}

fn arb_signed_pt_component() -> impl Strategy<Value = SignedPtComponent> {
    prop_oneof![
        arb_signed_number().prop_map(SignedPtComponent::Number),
        arb_signed_variable().prop_map(SignedPtComponent::Variable),
    ]
}

fn arb_mixed_pt_modifier() -> impl Strategy<Value = mtg_grammar::MixedPtModifier> {
    (arb_signed_pt_component(), arb_signed_pt_component())
        .prop_map(|(power, toughness)| mtg_grammar::MixedPtModifier { power, toughness })
        .prop_filter("mixed P/T modifier contains a variable", |modifier| {
            matches!(modifier.power, SignedPtComponent::Variable(_))
                || matches!(modifier.toughness, SignedPtComponent::Variable(_))
        })
}

fn arb_damage_life_gain_cap() -> impl Strategy<Value = DamageLifeGainCap> {
    prop_oneof![
        Just(DamageLifeGainCap::PlayerLifeTotalBeforeDamageDealt),
        Just(DamageLifeGainCap::PlaneswalkerLoyaltyBeforeDamageDealt),
        Just(DamageLifeGainCap::CreatureToughness),
    ]
}

fn arb_damage_amount() -> impl Strategy<Value = DamageAmount> {
    prop_oneof![
        (1u32..=10).prop_map(DamageAmount::Number),
        arb_variable().prop_map(DamageAmount::Variable),
    ]
}

fn arb_prevention_recipient() -> impl Strategy<Value = PreventionRecipient> {
    prop_oneof![
        Just(PreventionRecipient::AnyTarget),
        Just(PreventionRecipient::ThatPermanentOrPlayer),
    ]
}

fn arb_modal_mode() -> impl Strategy<Value = ModalMode> {
    prop_oneof![
        arb_color().prop_map(|color| ModalMode::CounterTargetColoredSpell { color }),
        arb_color().prop_map(|color| ModalMode::DestroyTargetColoredPermanent { color }),
        (1u32..=10).prop_map(|amount| ModalMode::TargetPlayerGainsLife { amount }),
        (arb_damage_amount(), arb_prevention_recipient()).prop_map(|(amount, recipient)| {
            ModalMode::PreventDamageThisTurn {
                effect: DamagePreventionEffect {
                    amount: DamagePreventionAmount::Next(amount),
                    kind: None,
                    recipient: Some(recipient),
                    duration: DamagePreventionDuration::ThisTurn,
                },
            }
        }),
    ]
}

fn arb_evasion_keyword() -> impl Strategy<Value = Keyword> {
    prop_oneof![Just(Keyword::Flying), Just(Keyword::Islandwalk)]
}

fn arb_simple_keyword() -> impl Strategy<Value = Keyword> {
    prop_oneof![
        Just(Keyword::FirstStrike),
        Just(Keyword::Flying),
        Just(Keyword::Trample),
        Just(Keyword::Islandwalk),
    ]
}

fn arb_imperative_action() -> impl Strategy<Value = ImperativeAction> {
    prop_oneof![
        Just(ImperativeAction::DiscardYourHand),
        Just(ImperativeAction::AnteTopCardOfYourLibrary),
        arb_card_count().prop_map(|count| ImperativeAction::DrawCards { count }),
    ]
}

fn arb_player_casts_colored_spell_pay_mana_trigger() -> impl Strategy<Value = Statement> {
    (arb_color(), arb_mana_cost()).prop_map(|(color, cost)| {
        Statement::TriggeredAbility(TriggeredAbility {
            event: TriggerEvent::PlayerCastsColoredSpell { color },
            intervening_if: None,
            effects: vec![TriggerEffect::YouMayPayMana { cost }],
        })
    })
}

fn arb_player_casts_colored_spell_pay_mana_gain_life_trigger() -> impl Strategy<Value = Statement> {
    (arb_color(), arb_mana_cost(), 1u32..=10).prop_map(|(color, cost, amount)| {
        Statement::TriggeredAbility(TriggeredAbility {
            event: TriggerEvent::PlayerCastsColoredSpell { color },
            intervening_if: None,
            effects: vec![
                TriggerEffect::YouMayPayMana { cost },
                TriggerEffect::IfYouDoGainLife { amount },
            ],
        })
    })
}

fn arb_enchanted_land_has_upkeep_pay_mana_gain_life() -> impl Strategy<Value = Statement> {
    (arb_mana_cost(), 1u32..=10).prop_map(|(cost, amount)| {
        Statement::StaticAbility(StaticAbility::EnchantedHasTriggeredAbility {
            object: EnchantedObject::Permanent(PermanentType::Land),
            ability: TriggeredAbility {
                event: TriggerEvent::BeginningOfYourUpkeep,
                intervening_if: None,
                effects: vec![
                    TriggerEffect::YouMayPayMana { cost },
                    TriggerEffect::IfYouDoGainLife { amount },
                ],
            },
        })
    })
}

fn arb_target_player_discards_activated_ability() -> impl Strategy<Value = Statement> {
    arb_card_count().prop_map(|count| {
        Statement::ActivatedAbility(ActivatedAbility {
            costs: vec![
                ActivatedCost::Mana(ManaCost {
                    symbols: vec![ManaSymbol::Generic(3)],
                }),
                ActivatedCost::Tap,
            ],
            effect: ActivatedEffect::TargetPlayerDiscardsCards { count },
        })
    })
}

fn arb_statement() -> impl Strategy<Value = Statement> {
    prop_oneof![
        arb_mana_cost().prop_map(Statement::ManaCost),
        arb_mana_cost().prop_map(|mana| Statement::AddMana { mana }),
        Just(Statement::CounterTargetSpell),
        Just(Statement::DestroyTargetPermanents {
            permanent_types: vec![PermanentType::Creature],
        }),
        arb_mana_cost().prop_map(|mana| {
            Statement::ThisSpellCostsManaMoreToCastForEachTargetBeyondTheFirst { mana }
        }),
        Just(Statement::NamedSourceDealsDamage {
            event: DamageEvent {
                source: "Fireball".to_string(),
                amount: DamageAmount::Variable(Variable::X),
                recipient: DamageRecipients::DividedEvenlyRoundedDownAmongAnyNumberOfTargets,
            },
        }),
        Just(Statement::NamedSourceDealsDamage {
            event: DamageEvent {
                source: "Disintegrate".to_string(),
                amount: DamageAmount::Variable(Variable::X),
                recipient: DamageRecipients::AnyTarget,
            },
        }),
        Just(Statement::NamedSourceDealsDamage {
            event: DamageEvent {
                source: "Lightning Bolt".to_string(),
                amount: DamageAmount::Number(3),
                recipient: DamageRecipients::AnyTarget,
            },
        }),
        Just(Statement::NamedSourceDealsDamage {
            event: DamageEvent {
                source: "Earthquake".to_string(),
                amount: DamageAmount::Variable(Variable::X),
                recipient: DamageRecipients::List(vec![
                    DamageRecipient::EachCreatureWithoutKeyword {
                        keyword: Keyword::Flying,
                    },
                    DamageRecipient::EachPlayer,
                ]),
            },
        }),
        Just(Statement::PreventDamageThisTurn {
            effect: DamagePreventionEffect {
                amount: DamagePreventionAmount::All,
                kind: Some(DamageKind::CombatDamage),
                recipient: None,
                duration: DamagePreventionDuration::ThisTurn,
            },
        }),
        (arb_damage_amount(), arb_prevention_recipient()).prop_map(|(amount, recipient)| {
            Statement::PreventDamageThisTurn {
                effect: DamagePreventionEffect {
                    amount: DamagePreventionAmount::Next(amount),
                    kind: None,
                    recipient: Some(recipient),
                    duration: DamagePreventionDuration::ThisTurn,
                },
            }
        }),
        (arb_damage_amount(), arb_prevention_recipient()).prop_map(|(amount, recipient)| {
            Statement::IfYouDoPreventDamageThisTurn {
                effect: DamagePreventionEffect {
                    amount: DamagePreventionAmount::Next(amount),
                    kind: None,
                    recipient: Some(recipient),
                    duration: DamagePreventionDuration::ThisTurn,
                },
            }
        }),
        (arb_color(), arb_variable()).prop_map(|(color, variable)| {
            Statement::SpendOnlyColorManaOnVariable { color, variable }
        }),
        arb_permanent_type().prop_map(|permanent_type| {
            Statement::AsSourceEntersYouLoseLifeEqualToYourLifeTotal {
                source: SourceObject::This(permanent_type),
            }
        }),
        prop::collection::vec(arb_damage_life_gain_cap(), 2..5)
            .prop_map(|caps| Statement::YouGainLifeEqualToDamageDealtCapped { caps }),
        Just(Statement::IfYouCantYouLoseTheGame),
        (1u32..=10).prop_map(|amount| Statement::IfYouCantSourceDealsDamageToYou {
            source: SourceObject::This(PermanentType::Creature),
            amount: DamageAmount::Number(amount),
        }),
        prop::collection::vec(arb_simple_keyword(), 2..5).prop_map(Statement::KeywordList),
        prop::collection::vec(arb_simple_keyword(), 2..5).prop_map(Statement::SemicolonKeywordList),
        arb_permanent_type().prop_map(|permanent_type| {
            Statement::IfItsPermanentCantBeRegeneratedAndWouldDieExileInsteadThisTurn {
                permanent_type,
            }
        }),
        (arb_permanent_type(), arb_permanent_type()).prop_map(|(a, b)| {
            Statement::DestroyTargetPermanents {
                permanent_types: vec![a, b],
            }
        }),
        arb_permanent_type().prop_map(|permanent_type| {
            Statement::DestroyTargetPermanents {
                permanent_types: vec![permanent_type],
            }
        }),
        prop::collection::vec(arb_permanent_type(), 1..5)
            .prop_map(|permanent_types| { Statement::DestroyAll { permanent_types } }),
        (arb_permanent_type(), arb_permanent_type()).prop_map(|(controller_of, attach_to)| {
            Statement::ThatPermanentsControllerMayAttachThisAuraToPermanentOfTheirChoice {
                controller_of,
                attach_to,
            }
        }),
        arb_basic_land_type()
            .prop_map(|basic_land_type| { Statement::DestroyAllBasicLands { basic_land_type } }),
        (arb_permanent_type(), arb_pt_modifier()).prop_map(|(permanent_type, modifier)| {
            Statement::TargetPermanentGetsUntilEndOfTurn {
                permanent_type,
                modifier,
            }
        }),
        (arb_permanent_type(), arb_mixed_pt_modifier()).prop_map(|(permanent_type, modifier)| {
            Statement::TargetPermanentGetsMixedUntilEndOfTurn {
                permanent_type,
                modifier,
            }
        }),
        (arb_permanent_type(), arb_evasion_keyword()).prop_map(|(permanent_type, keyword)| {
            Statement::TargetPermanentGainsKeywordUntilEndOfTurn {
                permanent_type,
                keyword,
            }
        }),
        arb_permanent_type().prop_map(|permanent_type| {
            Statement::TapAllPermanentsTargetPlayerControlsAndThatPlayerLosesUnspentMana {
                permanent_type,
            }
        }),
        arb_permanent_type().prop_map(|permanent_type| {
            Statement::TargetPlayerActivatesManaAbilityOfEachPermanentTheyControl { permanent_type }
        }),
        arb_card_count().prop_map(|count| Statement::TargetPlayerDiscardsCardsAtRandom { count }),
        arb_card_count().prop_map(|count| {
            Statement::LookAtTopCardsOfTargetPlayersLibraryThenPutThemBackInAnyOrder { count }
        }),
        Just(Statement::YouMayHaveThatPlayerShuffle),
        (1u32..=10).prop_map(|amount| Statement::TargetPlayerGainsLife { amount }),
        Just(Statement::IfYouWouldDrawCardDuringYourDrawStepInsteadYouMaySkipThatDraw),
        Just(Statement::ThenThatPlayerLosesUnspentManaAndYouAddManaLostThisWay),
        Just(Statement::ChangeTextOfTargetSpellOrPermanentReplacingBasicLandType),
        Just(Statement::RegenerateTargetCreature),
        Just(Statement::ActivateOnlyDuringYourTurn),
        Just(Statement::ActivateOnlyDuringCombat),
        Just(Statement::ActivateOnlyDuringYourTurnAndOnlyOnceEachTurn),
        Just(Statement::ActivateOnlyDuringOpponentsTurnBeforeAttackersDeclared),
        Just(Statement::ActivateOnlyAsSorcery),
        Just(Statement::DestroyItAtBeginningOfNextEndStepIfItDidntAttackThisTurn),
        (1u32..=10, 1u32..=10).prop_map(|(power, toughness)| {
            Statement::IfYouDoCastThatCardFaceDownWithoutPayingManaCost { power, toughness }
        }),
        Just(Statement::IfFaceDownSpellCreatureWouldAssignOrDealDamageOrTapTurnFaceUpInstead),
        (1u32..=10).prop_map(|threshold| {
            Statement::IfThisAbilityActivatedAtLeastTimesThisTurnSacrificeSourceAtNextEndStep {
                threshold,
                source: SourceObject::This(PermanentType::Creature),
            }
        }),
        Just(Statement::AntePlayRestriction),
        Just(Statement::EachPlayerPerformsAction {
            action: EachPlayerAction::AnteTopCardOfTheirLibrary
        }),
        Just(Statement::YouOwnTargetCardInZone { zone: Zone::Ante }),
        Just(Statement::ExchangeThatCardWithTopCardOfYourLibrary),
        (prop::collection::vec(arb_spell_type(), 1..3), arb_color()).prop_map(
            |(spell_types, color)| Statement::CopyTargetSpellExceptCopyIsColor {
                spell_types,
                color,
            }
        ),
        Just(Statement::YouMayChooseNewTargetsForTheCopy),
        (1u32..=10).prop_map(|amount| Statement::IfYouDoGainLife { amount }),
        arb_permanent_type().prop_map(|permanent_type| Statement::IfYouDoUntap {
            source: SourceObject::This(permanent_type),
        }),
        prop::collection::vec(arb_evasion_keyword(), 1..3).prop_map(|keywords| {
            Statement::IfYouDoUntilYourNextTurnYouCantBeAttackedExceptByCreaturesWithKeywords {
                keywords,
            }
        }),
        (arb_player_casts_colored_spell_pay_mana_trigger(), 1u32..=10,).prop_map(
            |(trigger, amount)| {
                Statement::Compound(vec![trigger, Statement::IfYouDoGainLife { amount }])
            }
        ),
        arb_player_casts_colored_spell_pay_mana_trigger(),
        arb_player_casts_colored_spell_pay_mana_gain_life_trigger(),
        arb_enchanted_land_has_upkeep_pay_mana_gain_life(),
        arb_target_player_discards_activated_ability(),
        prop::collection::vec(arb_modal_mode(), 1..5)
            .prop_map(|modes| Statement::ModalChoice { modes }),
        prop::collection::vec(arb_imperative_action(), 2..5)
            .prop_map(|actions| Statement::ImperativeActionSequence { actions }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 1000, ..ProptestConfig::default() })]

    /// Lowering is total over every Statement the parser can produce.
    #[test]
    fn lowering_is_total(stmt in arb_statement()) {
        prop_assert!(lower(&stmt).is_ok());
    }

    /// For ManaCost statements, the IR's total() must equal the sum of
    /// pip weights: each colored pip contributes 1, each generic pip
    /// contributes its number. Cross-checks the lowering against an
    /// independently-computed reference.
    #[test]
    fn mana_value_total_matches_pip_sum(stmt in arb_statement()) {
        let Statement::ManaCost(ref mc) = stmt else { return Ok(()); };
        let expected: u32 = mc.symbols.iter().map(|s| match s {
            ManaSymbol::Generic(n) => *n,
            _ => 1,
        }).sum();
        let CardEffect::ManaCost(mv) = lower(&stmt).unwrap() else {
            return Err(TestCaseError::fail("ManaCost statement lowered to non-ManaCost effect"));
        };
        prop_assert_eq!(mv.total(), expected);
    }
}
