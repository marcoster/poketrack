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
    pub dex_id: Option<i32>,
    pub rarity: String,
    pub image: Option<String>,
    pub stage: Option<String>,
    pub evolves_from: Option<String>,
    pub illustrator: Option<String>,
    pub description: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct PokemonIndex {
    pub card_id: String,
    pub dex_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, FromRow)]
pub struct CollectedPokemon {
    pub dex_id: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PokedexCompletion {
    pub collected: i64,
    pub total: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct CardSetInfo {
    pub card_id: String,
    pub set_id: String,
    pub set_name: String,
    pub local_id: String,
    pub rarity: String,
}

#[derive(Debug, Clone, FromRow)]
pub struct SetMissingStats {
    pub set_id: String,
    pub set_name: String,
    pub missing: i64,
}
