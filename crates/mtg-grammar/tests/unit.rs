// Tier 1 unit tests. Must collectively finish in well under a second.
// Hand-written input → expected AST and AST → expected text.

use mtg_grammar::{
    parse, unparse, DestroyTarget, EnchantObject, InterveningIf, Keyword, ManaCost, ManaSymbol,
    PermanentType, PtModifier, Sign, SignedNumber, Statement, StaticAbility, TriggerEffect,
    TriggerEvent, TriggeredAbility, Zone,
};

fn mc(symbols: Vec<ManaSymbol>) -> Statement {
    Statement::ManaCost(ManaCost { symbols })
}

#[test]
fn parses_single_colored_mana_symbol() {
    assert_eq!(parse("{R}").unwrap(), mc(vec![ManaSymbol::Red]));
}

#[test]
fn parses_generic_mana_symbol() {
    assert_eq!(parse("{2}").unwrap(), mc(vec![ManaSymbol::Generic(2)]));
}

#[test]
fn parses_generic_zero_mana_symbol() {
    // {0} is a real cost (Memnite, Spellbook). The canonical form is "{0}".
    assert_eq!(parse("{0}").unwrap(), mc(vec![ManaSymbol::Generic(0)]));
    assert_eq!(unparse(&mc(vec![ManaSymbol::Generic(0)])), "{0}");
}

#[test]
fn parses_compound_mana_cost() {
    assert_eq!(
        parse("{2}{R}{R}").unwrap(),
        mc(vec![
            ManaSymbol::Generic(2),
            ManaSymbol::Red,
            ManaSymbol::Red,
        ]),
    );
}

#[test]
fn parses_all_five_colors() {
    assert_eq!(
        parse("{W}{U}{B}{R}{G}").unwrap(),
        mc(vec![
            ManaSymbol::White,
            ManaSymbol::Blue,
            ManaSymbol::Black,
            ManaSymbol::Red,
            ManaSymbol::Green,
        ]),
    );
}

#[test]
fn parses_colorless_mana_symbol() {
    assert_eq!(parse("{C}").unwrap(), mc(vec![ManaSymbol::Colorless]));
}

#[test]
fn rejects_internal_whitespace_in_mana_cost() {
    assert!(parse("{2} {R}").is_err());
}

#[test]
fn parses_destroy_target_creature() {
    assert_eq!(
        parse("Destroy target creature.").unwrap(),
        Statement::Destroy {
            target: DestroyTarget::TargetPermanents(vec![PermanentType::Creature]),
        },
    );
}

#[test]
fn destroy_is_case_insensitive() {
    assert_eq!(
        parse("destroy target creature.").unwrap(),
        Statement::Destroy {
            target: DestroyTarget::TargetPermanents(vec![PermanentType::Creature]),
        },
    );
    assert_eq!(
        parse("DESTROY TARGET CREATURE.").unwrap(),
        Statement::Destroy {
            target: DestroyTarget::TargetPermanents(vec![PermanentType::Creature]),
        },
    );
}

#[test]
fn destroy_requires_terminating_period() {
    assert!(parse("Destroy target creature").is_err());
}

#[test]
fn unparses_mana_cost_in_scryfall_form() {
    assert_eq!(
        unparse(&mc(vec![
            ManaSymbol::Generic(2),
            ManaSymbol::Red,
            ManaSymbol::Red,
        ])),
        "{2}{R}{R}",
    );
}

#[test]
fn unparses_destroy_in_canonical_form() {
    assert_eq!(
        unparse(&Statement::Destroy {
            target: DestroyTarget::TargetPermanents(vec![PermanentType::Creature]),
        }),
        "Destroy target creature.",
    );
}

// "Enchanted <type> gets <+P>/<+T>." — Animate Dead's "-1/-0" line and
// its kin (Holy Strength, Unholy Strength, Weakness).

fn enchanted_gets(pt: PermanentType, power: SignedNumber, toughness: SignedNumber) -> Statement {
    Statement::StaticAbility(StaticAbility::EnchantedGets {
        permanent_type: pt,
        modifier: PtModifier { power, toughness },
    })
}

fn sn(sign: Sign, magnitude: u32) -> SignedNumber {
    SignedNumber { sign, magnitude }
}

#[test]
fn parses_enchanted_creature_gets_negative_pt_modifier() {
    assert_eq!(
        parse("Enchanted creature gets -1/-0.").unwrap(),
        enchanted_gets(
            PermanentType::Creature,
            sn(Sign::Minus, 1),
            sn(Sign::Minus, 0),
        ),
    );
}

#[test]
fn parses_enchanted_creature_gets_positive_pt_modifier() {
    assert_eq!(
        parse("Enchanted creature gets +1/+2.").unwrap(),
        enchanted_gets(
            PermanentType::Creature,
            sn(Sign::Plus, 1),
            sn(Sign::Plus, 2),
        ),
    );
}

#[test]
fn unparses_enchanted_gets_preserves_explicit_signs() {
    assert_eq!(
        unparse(&enchanted_gets(
            PermanentType::Creature,
            sn(Sign::Minus, 1),
            sn(Sign::Minus, 0),
        )),
        "Enchanted creature gets -1/-0.",
    );
    assert_eq!(
        unparse(&enchanted_gets(
            PermanentType::Creature,
            sn(Sign::Plus, 1),
            sn(Sign::Plus, 2),
        )),
        "Enchanted creature gets +1/+2.",
    );
}

#[test]
fn enchanted_gets_requires_terminating_period() {
    assert!(parse("Enchanted creature gets -1/-0").is_err());
}

// `Enchant <object>` keyword. The object is a permanent type
// (existing) or a card type in a named zone (Animate Dead's
// "Enchant creature card in a graveyard").

#[test]
fn parses_enchant_permanent_keyword() {
    assert_eq!(
        parse("Enchant artifact").unwrap(),
        Statement::Keyword(Keyword::Enchant(EnchantObject::Permanent(
            PermanentType::Artifact,
        ))),
    );
}

#[test]
fn parses_enchant_card_in_graveyard() {
    assert_eq!(
        parse("Enchant creature card in a graveyard").unwrap(),
        Statement::Keyword(Keyword::Enchant(EnchantObject::CardInZone {
            card_type: PermanentType::Creature,
            zone: Zone::Graveyard,
        })),
    );
}

#[test]
fn unparses_enchant_card_in_zone() {
    assert_eq!(
        unparse(&Statement::Keyword(Keyword::Enchant(
            EnchantObject::CardInZone {
                card_type: PermanentType::Creature,
                zone: Zone::Graveyard,
            },
        ))),
        "Enchant creature card in a graveyard",
    );
}

// "When <event>, <effect>." — single-trigger ability without an
// intervening-if. Animate Dead's second trigger paragraph.

#[test]
fn parses_simple_leaves_battlefield_trigger() {
    assert_eq!(
        parse("When this Aura leaves the battlefield, that creature's controller sacrifices it.")
            .unwrap(),
        Statement::TriggeredAbility(TriggeredAbility {
            event: TriggerEvent::ThisAuraLeavesTheBattlefield,
            intervening_if: None,
            effects: vec![TriggerEffect::ThatCreaturesControllerSacrificesIt],
        }),
    );
}

#[test]
fn unparses_simple_leaves_battlefield_trigger() {
    assert_eq!(
        unparse(&Statement::TriggeredAbility(TriggeredAbility {
            event: TriggerEvent::ThisAuraLeavesTheBattlefield,
            intervening_if: None,
            effects: vec![TriggerEffect::ThatCreaturesControllerSacrificesIt],
        })),
        "When this Aura leaves the battlefield, that creature's controller sacrifices it.",
    );
}

// Pattern 4: trigger with intervening-if and a compound effect body —
// Animate Dead's first trigger.

fn animate_dead_enters_trigger() -> Statement {
    Statement::TriggeredAbility(TriggeredAbility {
        event: TriggerEvent::ThisAuraEnters,
        intervening_if: Some(InterveningIf::ItsOnTheBattlefield),
        effects: vec![
            TriggerEffect::LosesAndGainsKeyword {
                loses: Keyword::Enchant(EnchantObject::CardInZone {
                    card_type: PermanentType::Creature,
                    zone: Zone::Graveyard,
                }),
                gains: Keyword::Enchant(EnchantObject::PutOntoBattlefieldByThisAura {
                    card_type: PermanentType::Creature,
                }),
            },
            TriggerEffect::ReturnEnchantedCardAndAttach {
                card_type: PermanentType::Creature,
            },
        ],
    })
}

const ANIMATE_DEAD_ENTERS_TEXT: &str = "When this Aura enters, if it's on the battlefield, it loses \"enchant creature card in a graveyard\" and gains \"enchant creature put onto the battlefield with this Aura.\" Return enchanted creature card to the battlefield under your control and attach this Aura to it.";

#[test]
fn parses_enters_trigger_with_intervening_if_and_compound_effect() {
    assert_eq!(
        parse(ANIMATE_DEAD_ENTERS_TEXT).unwrap(),
        animate_dead_enters_trigger(),
    );
}

#[test]
fn unparses_enters_trigger_round_trip() {
    assert_eq!(
        unparse(&animate_dead_enters_trigger()),
        ANIMATE_DEAD_ENTERS_TEXT,
    );
}

#[test]
fn parses_two_triggers_chained_on_one_line() {
    let text = "When this Aura enters, if it's on the battlefield, it loses \"enchant creature card in a graveyard\" and gains \"enchant creature put onto the battlefield with this Aura.\" Return enchanted creature card to the battlefield under your control and attach this Aura to it. When this Aura leaves the battlefield, that creature's controller sacrifices it.";
    let leaves = Statement::TriggeredAbility(TriggeredAbility {
        event: TriggerEvent::ThisAuraLeavesTheBattlefield,
        intervening_if: None,
        effects: vec![TriggerEffect::ThatCreaturesControllerSacrificesIt],
    });
    assert_eq!(
        parse(text).unwrap(),
        Statement::Compound(vec![animate_dead_enters_trigger(), leaves]),
    );
}
