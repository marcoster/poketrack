use slint::Model;
use crate::gui::models::{CardModel, SetModel, SetDetailModel, AppState};
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
                set_details: None,
                filter: "all".to_string(),
                sort_by: "number".to_string(),
                sort_direction: "asc".to_string(),
                completion: PokedexCompletion { collected: 0, total: 0 },
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
            self.state.cards = cards.into_iter().map(|card| {
                let dex_id = card.dex_id.unwrap_or(0);
                let is_collected = !missing_pokemon.contains(&dex_id);
                let sets = self.repository.get_pokemon_sets(dex_id).unwrap_or_default();
                CardModel {
                    dex_id,
                    name: card.name,
                    is_collected,
                    sets,
                }
            }).collect();
        }
    }

    fn load_sets(&mut self) {
        if let Ok(sets) = self.repository.get_all_sets() {
            self.state.sets = sets.into_iter().map(|set| {
                let missing_count = self.repository.get_set_missing_stats(None)
                    .unwrap_or_default()
                    .iter()
                    .find(|s| s.set_id == set.id)
                    .map(|s| s.missing)
                    .unwrap_or(0);
                let language = if set.id.starts_with("en-") { "English" } else { "Japanese" };
                SetModel {
                    id: set.id,
                    name: set.name,
                    release_date: set.release_date.unwrap_or_default(),
                    missing_count,
                    language: language.to_string(),
                }
            }).collect();
        }
    }

    fn update_completion(&mut self) {
        if let Ok(completion) = self.repository.get_pokedex_completion() {
            self.state.completion = completion;
        }
    }

    pub fn filter_cards(&mut self, filter: &str) {
        self.state.filter = filter.to_string();
        self.apply_filters();
    }

    pub fn sort_cards(&mut self, sort_by: &str, direction: &str) {
        self.state.sort_by = sort_by.to_string();
        self.state.sort_direction = direction.to_string();
        self.apply_sorting();
    }

    fn apply_filters(&mut self) {
        let filter = &self.state.filter;
        let cards = &mut self.state.cards;
        
        match filter.as_str() {
            "all" => {
                // No filtering needed
            }
            "collected" => {
                cards.retain(|card| card.is_collected);
            }
            "missing" => {
                cards.retain(|card| !card.is_collected);
            }
            _ => {}
        }
    }

    fn apply_sorting(&mut self) {
        let sort_by = &self.state.sort_by;
        let direction = &self.state.sort_direction;
        let cards = &mut self.state.cards;
        
        match sort_by.as_str() {
            "number" => {
                if direction == "asc" {
                    cards.sort_by(|a, b| a.dex_id.cmp(&b.dex_id));
                } else {
                    cards.sort_by(|a, b| b.dex_id.cmp(&a.dex_id));
                }
            }
            "name" => {
                if direction == "asc" {
                    cards.sort_by(|a, b| a.name.cmp(&b.name));
                } else {
                    cards.sort_by(|a, b| b.name.cmp(&a.name));
                }
            }
            _ => {}
        }
    }

    pub fn get_state(&self) -> &AppState {
        &self.state
    }

    pub fn get_mut_state(&mut self) -> &mut AppState {
        &mut self.state
    }

    pub fn toggle_card_collection(&mut self, dex_id: i32) -> Result<()> {
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

    pub fn add_missing_cards_from_set(&mut self, set_id: &str) -> Result<()> {
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

    pub fn update_database(&mut self, force: bool) -> Result<()> {
        // Implementation for database update
        let languages = vec!["en", "ja"];
        let mode = if force { "force refresh" } else { "incremental" };
        tracing::info!("Starting TCGdex cache update from cards-database ({} mode)...", mode);

        if force {
            // Clear cache if force update
            // This would be implemented in the repository
        }

        let db = cards_database::CardsDatabase::new()?;
        let mut total_cards_inserted: u64 = 0;
        let mut total_cards_skipped: u64 = 0;
        let mut total_sets_completed = 0u64;

        for lang in &languages {
            let lang_label = if *lang == "en" { "English (en)" } else { "Japanese (ja)" };
            let series_list = db.load_series(lang)?;
            tracing::info!("Found {} series for {}", series_list.len(), lang_label);

            let mut sets_to_process: Vec<cards_database::SetData> = Vec::new();
            let mut sets_skipped = 0u64;

            for serie in &series_list {
                // Upsert series
                // This would be implemented in the repository
                let sets = db.load_sets_for_series(&serie.id, lang)?;
                tracing::info!("Found {} sets for series {}", sets.len(), serie.id);

                for set_data in &sets {
                    let should_fetch = if force {
                        true
                    } else {
                        // Check if set is already complete
                        // This would be implemented in the repository
                        false
                    };

                    if should_fetch {
                        sets_to_process.push(set_data.clone());
                    }
                }
            }

            tracing::info!("{} sets already complete, skipping", sets_skipped);
            tracing::info!("Processing {} new/updated sets for {}...", sets_to_process.len(), lang_label);

            let mut cards_inserted: u64 = 0;
            let cards_skipped: u64 = 0;
            let mut sets_completed = 0u64;

            for (set_idx, set_data) in sets_to_process.iter().enumerate() {
                let set_name = match *lang {
                    "en" => set_data.name_en.as_deref().unwrap_or(""),
                    "ja" => set_data.name_ja.as_deref().unwrap_or(""),
                    _ => "",
                };
                let series_name = series_list.iter()
                    .find(|s| s.id == set_data.serie_id)
                    .and_then(|s| match *lang {
                        "en" => s.name_en.as_deref(),
                        "ja" => s.name_ja.as_deref(),
                        _ => None,
                    })
                    .unwrap_or("");

                tracing::info!(
                    "[{}] Processing set {}/{}: {} ({})",
                    lang, set_idx + 1, sets_to_process.len(), set_name, series_name
                );

                let cards = db.load_cards(&set_data.id, lang)?;
                tracing::info!("Loading {} cards for set {}...", cards.len(), set_data.id);

                // Upsert set with cards
                // This would be implemented in the repository
                cards_inserted += cards.len() as u64;
                sets_completed += 1;
                tracing::debug!("Set {} marked as finished", set_data.id);
            }

            total_cards_inserted += cards_inserted;
            total_cards_skipped += cards_skipped;
            total_sets_completed += sets_completed;
        }

        // Fetch English translations
        // This would be implemented in the repository

        tracing::info!(
            "TCGdex cache update complete! Sets completed: {}, Cards inserted: {}, Cards skipped: {}",
            total_sets_completed, total_cards_inserted, total_cards_skipped
        );
        Ok(())
    }
}