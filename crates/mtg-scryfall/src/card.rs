use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Card {
    pub name: String,
    pub set_code: String,
    pub collector_number: String,
    #[serde(default)]
    pub oracle_text: String,
    #[serde(default)]
    pub mana_cost: String,
    pub layout: Layout,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Layout {
    Normal,
    Split,
    Flip,
    Transform,
    ModalDfc,
    Meld,
    Leveler,
    Class,
    Saga,
    Adventure,
    Mutate,
    Prototype,
    Battle,
    Case,
    Planar,
    Scheme,
    Vanguard,
    Token,
    DoubleFacedToken,
    Emblem,
    Augment,
    Host,
    ArtSeries,
    ReversibleCard,
    #[serde(other)]
    Other,
}
