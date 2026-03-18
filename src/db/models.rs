use serde::{Deserialize, Serialize};
use sqlx::FromRow;

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Series {
    pub id: String,
    pub name: String,
    pub logo: Option<String>,
    pub symbol: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Set {
    pub id: String,
    pub name: String,
    pub logo: Option<String>,
    pub symbol: Option<String>,
    pub serie_id: String,
    pub release_date: String,
    pub tcg_online: Option<String>,
    pub total_cards: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct Card {
    pub id: String,
    pub set_id: String,
    pub local_id: String,
    pub name: String,
    pub category: String,
    pub hp: Option<i32>,
    pub types: Option<String>,
    pub rarity: String,
    pub image: Option<String>,
    pub stage: Option<String>,
    pub evolves_from: Option<String>,
    pub illustrator: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CollectedCard {
    pub card_id: String,
    pub language: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PokemonIndex {
    pub card_id: String,
    pub dex_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CardWithPokemon {
    pub card: Card,
    pub dex_ids: Vec<i32>,
    pub is_collected: bool,
    pub language: Option<String>,
}
