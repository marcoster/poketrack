use slint::Model;
use crate::db::models::{CardSetInfo, PokedexCompletion, Set as DbSet, SetMissingCardInfo, SetMissingStats};
use crate::cards_database::{CardData, SerieData, SetData};

#[derive(Model)]
pub struct CardModel {
    pub dex_id: i32,
    pub name: String,
    pub is_collected: bool,
    pub sets: Vec<CardSetInfo>,
}

#[derive(Model)]
pub struct SetModel {
    pub id: String,
    pub name: String,
    pub release_date: String,
    pub missing_count: i64,
    pub language: String,
}

#[derive(Model)]
pub struct SetDetailModel {
    pub set: SetModel,
    pub missing_cards: Vec<CardModel>,
}

#[derive(Model)]
pub struct AppState {
    pub current_view: String,
    pub cards: Vec<CardModel>,
    pub sets: Vec<SetModel>,
    pub set_details: Option<SetDetailModel>,
    pub filter: String,
    pub sort_by: String,
    pub sort_direction: String,
    pub completion: PokedexCompletion,
}