#[derive(Debug, Clone)]
pub struct CardModel {
    pub dex_id: i32,
    pub name: String,
    pub is_collected: bool,
}

#[derive(Debug, Clone)]
pub struct SetModel {
    pub id: String,
    pub name: String,
    pub release_date: String,
    pub missing_count: i64,
    pub language: String,
}

#[derive(Debug, Clone)]
pub struct AppState {
    pub current_view: String,
    pub cards: Vec<CardModel>,
    pub sets: Vec<SetModel>,
    pub filter: String,
    pub sort_by: String,
    pub sort_direction: String,
    pub collected_count: i64,
    pub total_count: i64,
    pub status: String,
    pub selected_set: Option<String>,
    pub selected_set_name: String,
    pub set_cards: Vec<CardModel>,
}
