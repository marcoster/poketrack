use crate::gui::models::{CardModel, SetModel, AppState};
use crate::gui::db::GuiRepository;

pub struct App {
    repository: GuiRepository,
    state: AppState,
}

impl App {
    pub fn new(repository: GuiRepository) -> Self {
        let mut app = Self {
            repository,
            state: AppState {
                current_view: "cards".to_string(),
                cards: Vec::new(),
                sets: Vec::new(),
                filter: "all".to_string(),
                sort_by: "number".to_string(),
                sort_direction: "asc".to_string(),
                collected_count: 0,
                total_count: 0,
                status: "Ready".to_string(),
                selected_set: None,
                selected_set_name: String::new(),
                set_cards: Vec::new(),
            },
        };
        app.load_initial_data();
        app
    }

    fn load_initial_data(&mut self) {
        self.load_cards();
        self.load_sets();
        self.update_completion();
    }

    fn load_cards(&mut self) {
        if let Ok(cards) = self.repository.get_all_cards() {
            let missing_pokemon = self.repository.get_missing_pokemon().unwrap_or_default();
            self.state.cards = cards.into_iter().map(|(_id, name, dex_id)| {
                let dex = dex_id.unwrap_or(0);
                let is_collected = !missing_pokemon.contains(&dex);
                CardModel {
                    dex_id: dex,
                    name,
                    is_collected,
                }
            }).collect();
        }
    }

    fn load_sets(&mut self) {
        if let Ok(sets) = self.repository.get_all_sets() {
            let missing_stats = self.repository.get_set_missing_stats(None).unwrap_or_default();
            self.state.sets = sets.into_iter().map(|set| {
                let missing_count = missing_stats
                    .iter()
                    .find(|s| s.set_id == set.id)
                    .map(|s| s.missing)
                    .unwrap_or(0);
                let language = if set.id.starts_with("en-") { "English" } else { "Japanese" };
                SetModel {
                    id: set.id,
                    name: set.name,
                    release_date: set.release_date,
                    missing_count,
                    language: language.to_string(),
                }
            }).collect();
        }
    }

    fn update_completion(&mut self) {
        if let Ok(completion) = self.repository.get_pokedex_completion() {
            self.state.collected_count = completion.collected;
            self.state.total_count = completion.total;
        }
    }

    pub fn filter_cards(&mut self, filter: &str) {
        self.state.filter = filter.to_string();
    }

    pub fn sort_cards(&mut self, sort_by: &str, direction: &str) {
        self.state.sort_by = sort_by.to_string();
        self.state.sort_direction = direction.to_string();
    }

    pub fn get_filtered_cards(&self) -> Vec<&CardModel> {
        let mut filtered: Vec<&CardModel> = self.state.cards.iter().filter(|card| {
            match self.state.filter.as_str() {
                "collected" => card.is_collected,
                "missing" => !card.is_collected,
                _ => true,
            }
        }).collect();

        match self.state.sort_by.as_str() {
            "number" => {
                if self.state.sort_direction == "asc" {
                    filtered.sort_by(|a, b| a.dex_id.cmp(&b.dex_id));
                } else {
                    filtered.sort_by(|a, b| b.dex_id.cmp(&a.dex_id));
                }
            }
            "name" => {
                if self.state.sort_direction == "asc" {
                    filtered.sort_by(|a, b| a.name.cmp(&b.name));
                } else {
                    filtered.sort_by(|a, b| b.name.cmp(&a.name));
                }
            }
            _ => {}
        }

        filtered
    }

    pub fn get_state(&self) -> &AppState {
        &self.state
    }

    pub fn get_mut_state(&mut self) -> &mut AppState {
        &mut self.state
    }

    pub fn toggle_card_collection(&mut self, dex_id: i32) -> anyhow::Result<()> {
        let card = self.state.cards.iter_mut().find(|c| c.dex_id == dex_id);
        if let Some(card) = card {
            card.is_collected = !card.is_collected;
            if card.is_collected {
                self.repository.mark_pokemon_collected(dex_id)?;
            } else {
                self.repository.unmark_pokemon_collected(dex_id)?;
            }
            self.update_completion();
            Ok(())
        } else {
            Err(anyhow::anyhow!("Card not found"))
        }
    }

    pub fn add_missing_cards_from_set(&mut self, set_id: &str) -> anyhow::Result<()> {
        let missing_cards = self.repository.get_set_missing_pokemon_details(set_id)?;
        for card in missing_cards {
            self.repository.mark_pokemon_collected(card.dex_id)?;
            if let Some(card_model) = self.state.cards.iter_mut().find(|c| c.dex_id == card.dex_id) {
                card_model.is_collected = true;
            }
        }
        self.update_completion();
        Ok(())
    }

    pub fn get_cards(&self) -> &[CardModel] {
        &self.state.cards
    }

    pub fn get_sets(&self) -> &[SetModel] {
        &self.state.sets
    }

    pub fn get_status(&self) -> &str {
        &self.state.status
    }

    pub fn set_status(&mut self, status: &str) {
        self.state.status = status.to_string();
    }

    pub fn reload(&mut self) {
        self.load_cards();
        self.load_sets();
        self.update_completion();
    }

    pub fn select_set(&mut self, set_id: &str) {
        self.state.selected_set = Some(set_id.to_string());
        self.state.selected_set_name = self.state.sets.iter()
            .find(|s| s.id == set_id)
            .map(|s| s.name.clone())
            .unwrap_or_default();
        self.state.set_cards = self.repository
            .get_cards_for_set(set_id)
            .map(|cards| cards.into_iter().map(|(name, dex_id, collected)| CardModel {
                dex_id,
                name,
                is_collected: collected,
            }).collect())
            .unwrap_or_default();
    }

    pub fn clear_selected_set(&mut self) {
        self.state.selected_set = None;
        self.state.selected_set_name.clear();
        self.state.set_cards.clear();
    }

    pub fn get_selected_set_cards(&self) -> &[CardModel] {
        &self.state.set_cards
    }

    pub fn get_selected_set_name(&self) -> &str {
        &self.state.selected_set_name
    }

    pub fn refresh_set_cards(&mut self) {
        if let Some(set_id) = self.state.selected_set.clone() {
            self.select_set(&set_id);
        }
    }
}
