use anyhow::{Context, Result};
use serde_json::Value;
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct SerieData {
    pub id: String,
    pub name_en: Option<String>,
    pub name_ja: Option<String>,
    pub logo: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SetData {
    pub id: String,
    pub name_en: Option<String>,
    pub name_ja: Option<String>,
    pub serie_id: String,
    pub release_date: Option<String>,
    pub tcg_online: Option<String>,
    pub total_cards: i32,
}

#[derive(Debug, Clone)]
pub struct CardData {
    pub local_id: String,
    pub name: String,
    pub category: String,
    pub hp: Option<i32>,
    pub types: Option<String>,
    pub dex_id: Option<i32>,
    pub rarity: Option<String>,
    pub stage: Option<String>,
    pub evolves_from: Option<String>,
    pub illustrator: Option<String>,
    pub description: Option<String>,
    pub image: Option<String>,
}

pub struct CardsDatabase {
    series: HashMap<String, Vec<SerieData>>,
    sets_by_serie: HashMap<String, HashMap<String, Vec<SetData>>>,
    cards_by_set: HashMap<String, HashMap<String, Vec<CardData>>>,
}

impl CardsDatabase {
    pub fn new() -> Result<Self> {
        let json_base = PathBuf::from("cards-database-json");
        let mut db = Self {
            series: HashMap::new(),
            sets_by_serie: HashMap::new(),
            cards_by_set: HashMap::new(),
        };
        for lang in &["en", "ja"] {
            tracing::info!("Loading {} card database into memory...", lang);
            db.load_language(&json_base, lang)
                .with_context(|| format!("Failed to load language: {}", lang))?;
            let n_sets: usize = db.sets_by_serie.get(*lang)
                .map(|m| m.values().map(|v| v.len()).sum())
                .unwrap_or(0);
            let n_cards: usize = db.cards_by_set.get(*lang)
                .map(|m| m.values().map(|v| v.len()).sum())
                .unwrap_or(0);
            tracing::info!("Loaded {} series, {} sets, {} cards for {}",
                db.series.get(*lang).map_or(0, |v| v.len()),
                n_sets, n_cards, lang);
        }
        Ok(db)
    }

    pub fn load_series(&self, lang_code: &str) -> Result<Vec<SerieData>> {
        self.series.get(lang_code)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("Language not loaded: {}", lang_code))
    }

    pub fn load_sets_for_series(&self, serie_id: &str, lang_code: &str) -> Result<Vec<SetData>> {
        self.sets_by_serie.get(lang_code)
            .and_then(|m| m.get(serie_id))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No sets found for serie {} in {}", serie_id, lang_code))
    }

    pub fn load_cards(&self, set_id: &str, lang_code: &str) -> Result<Vec<CardData>> {
        self.cards_by_set.get(lang_code)
            .and_then(|m| m.get(set_id))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("No cards found for set {} in {}", set_id, lang_code))
    }

    fn load_language(&mut self, json_base: &Path, lang_code: &str) -> Result<()> {
        let dir = json_base.join(lang_code);
        if !dir.exists() {
            anyhow::bail!("Directory not found: {:?}", dir);
        }

        let mut series = Vec::new();
        let mut sets_by_serie = HashMap::new();
        let mut cards_by_set = HashMap::new();

        let entries = fs::read_dir(&dir)
            .with_context(|| format!("Failed to read directory: {:?}", dir))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if !path.is_file() || path.extension().map_or(true, |e| e != "json") {
                continue;
            }

            let serie = Self::parse_serie_file(&path).with_context(|| {
                format!("Failed to parse serie file: {:?}", path)
            })?;
            let serie_id = serie.id.clone();

            let serie_dir_name = path.file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or(&serie_id);
            let serie_dir = dir.join(serie_dir_name);
            let mut set_list = Vec::new();

            if serie_dir.is_dir() {
                let set_entries = fs::read_dir(&serie_dir)
                    .with_context(|| format!("Failed to read serie directory: {:?}", serie_dir))?;

                for set_entry in set_entries {
                    let set_entry = set_entry?;
                    let set_path = set_entry.path();
                    if !set_path.is_file() || set_path.extension().map_or(true, |e| e != "json") {
                        continue;
                    }

                    let set = Self::parse_set_file(&set_path, &serie_id).with_context(|| {
                        format!("Failed to parse set file: {:?}", set_path)
                    })?;
                    let set_id = set.id.clone();

                    let set_dir_name = set_path.file_stem()
                        .and_then(|s| s.to_str())
                        .unwrap_or(&set_id);
                    let card_dir = serie_dir.join(set_dir_name);
                    let mut card_list = Vec::new();
                    if card_dir.is_dir() {
                        let card_entries = fs::read_dir(&card_dir)
                            .with_context(|| format!("Failed to read card directory: {:?}", card_dir))?;

                        for card_entry in card_entries {
                            let card_entry = card_entry?;
                            let card_path = card_entry.path();
                            if !card_path.is_file() || card_path.extension().map_or(true, |e| e != "json") {
                                continue;
                            }

                            if let Ok(card) = Self::parse_card_file(&card_path, lang_code) {
                                card_list.push(card);
                            }
                        }
                    }

                    cards_by_set.insert(set_id, card_list);
                    set_list.push(set);
                }
            }

            sets_by_serie.insert(serie_id, set_list);
            series.push(serie);
        }

        self.series.insert(lang_code.to_string(), series);
        self.sets_by_serie.insert(lang_code.to_string(), sets_by_serie);
        self.cards_by_set.insert(lang_code.to_string(), cards_by_set);

        Ok(())
    }

    fn parse_serie_file(path: &Path) -> Result<SerieData> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {:?}", path))?;

        let json: Value = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON: {:?}", path))?;

        let id = json["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'id' in serie"))?
            .to_string();

        let name_en = json["name"]["en"].as_str().map(|s| s.to_string());
        let name_ja = json["name"]["ja"].as_str().map(|s| s.to_string());

        Ok(SerieData { id, name_en, name_ja, logo: None })
    }

    fn parse_set_file(path: &Path, serie_id: &str) -> Result<SetData> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {:?}", path))?;

        let json: Value = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON: {:?}", path))?;

        let id = json["id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("Missing 'id' in set"))?
            .to_string();

        let name_en = json["name"]["en"].as_str().map(|s| s.to_string());
        let name_ja = json["name"]["ja"].as_str().map(|s| s.to_string());

        let total_cards = json["cardCount"]["official"]
            .as_i64()
            .unwrap_or(0) as i32;

        let release_date = json["releaseDate"].as_str().map(|s| s.to_string());
        let tcg_online = json["tcgOnline"].as_str().map(|s| s.to_string());

        Ok(SetData {
            id,
            name_en,
            name_ja,
            serie_id: serie_id.to_string(),
            release_date,
            tcg_online,
            total_cards,
        })
    }

    fn parse_card_file(path: &Path, lang_code: &str) -> Result<CardData> {
        let content = fs::read_to_string(path)
            .with_context(|| format!("Failed to read file: {:?}", path))?;

        let json: Value = serde_json::from_str(&content)
            .with_context(|| format!("Failed to parse JSON: {:?}", path))?;

        let local_id = path.file_stem()
            .and_then(|s| s.to_str())
            .ok_or_else(|| anyhow::anyhow!("Invalid file name"))?
            .to_string();

        let name = json["name"].get(lang_code)
            .and_then(|v| v.as_str())
            .or_else(|| json["name"]["en"].as_str())
            .unwrap_or("Unknown")
            .to_string();

        let category = json["category"].as_str().unwrap_or("Pokemon").to_string();
        let hp = json["hp"].as_i64().map(|v| v as i32);

        let types = json["types"]
            .as_array()
            .map(|arr| {
                let types: Vec<String> = arr.iter()
                    .filter_map(|v| v.as_str())
                    .map(|s| s.to_string())
                    .collect();
                serde_json::to_string(&types).ok()
            })
            .flatten();

        let dex_id = json["dexId"]
            .as_array()
            .and_then(|arr| arr.first())
            .and_then(|v| v.as_i64())
            .map(|v| v as i32);

        let rarity = json["rarity"].as_str().map(|s| s.to_string());
        let stage = json["stage"].as_str().map(|s| s.to_string());

        let evolves_from = json["evolveFrom"].get(lang_code)
            .and_then(|v| v.as_str())
            .or_else(|| json["evolveFrom"]["en"].as_str())
            .map(|s| s.to_string());

        let illustrator = json["illustrator"].as_str().map(|s| s.to_string());

        let description = json["description"].get(lang_code)
            .and_then(|v| v.as_str())
            .or_else(|| json["description"]["en"].as_str())
            .map(|s| s.to_string());

        Ok(CardData {
            local_id,
            name,
            category,
            hp,
            types,
            dex_id,
            rarity,
            stage,
            evolves_from,
            illustrator,
            description,
            image: None,
        })
    }
}
