use anyhow::{Context, Result};
use serde_json::Value;
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
    json_base: PathBuf,
}

impl CardsDatabase {
    pub fn new() -> Self {
        Self {
            json_base: PathBuf::from("cards-database-json"),
        }
    }

    pub fn load_series(&self, lang_code: &str) -> Result<Vec<SerieData>> {
        let dir = &self.json_base.join(lang_code);
        if !dir.exists() {
            anyhow::bail!("Directory not found: {:?}", dir);
        }

        let mut series = Vec::new();
        
        let entries = fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {:?}", dir))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                if path.parent() == Some(dir.as_path()) {
                    match self.parse_serie_file(&path) {
                        Ok(serie) => series.push(serie),
                        Err(e) => tracing::warn!("Failed to parse serie file {:?}: {}", path, e),
                    }
                }
            }
        }

        Ok(series)
    }

    pub fn load_sets_for_series(&self, serie_id: &str, lang_code: &str) -> Result<Vec<SetData>> {
        let lang_dir = self.json_base.join(lang_code);
        let mut sets = Vec::new();
        self.find_sets_in_dir(&lang_dir, serie_id, &mut sets)?;
        Ok(sets)
    }

    fn find_sets_in_dir(&self, dir: &Path, serie_id: &str, sets: &mut Vec<SetData>) -> Result<()> {
        let entries = fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {:?}", dir))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.find_sets_in_dir(&path, serie_id, sets)?;
            } else if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(json) = serde_json::from_str::<Value>(&content) {
                        if json["serie"]["id"].as_str() == Some(serie_id) {
                            match self.parse_set_file(&path, serie_id) {
                                Ok(set) => sets.push(set),
                                Err(e) => tracing::warn!("Failed to parse set file {:?}: {}", path, e),
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    pub fn load_cards(&self, set_id: &str, lang_code: &str) -> Result<Vec<CardData>> {
        let lang_dir = self.json_base.join(lang_code);
        let mut cards = Vec::new();
        self.find_cards_in_dir(&lang_dir, set_id, lang_code, &mut cards)?;
        Ok(cards)
    }

    fn find_cards_in_dir(&self, dir: &Path, set_id: &str, lang_code: &str, cards: &mut Vec<CardData>) -> Result<()> {
        let entries = fs::read_dir(dir)
            .with_context(|| format!("Failed to read directory: {:?}", dir))?;

        for entry in entries {
            let entry = entry?;
            let path = entry.path();
            if path.is_dir() {
                self.find_cards_in_dir(&path, set_id, lang_code, cards)?;
            } else if path.is_file() && path.extension().is_some_and(|ext| ext == "json") {
                if let Ok(content) = fs::read_to_string(&path) {
                    if let Ok(json) = serde_json::from_str::<Value>(&content) {
                        if json["set"]["id"].as_str() == Some(set_id) {
                            match self.parse_card_file(&path, lang_code) {
                                Ok(card) => cards.push(card),
                                Err(e) => tracing::warn!("Failed to parse card file {:?}: {}", path, e),
                            }
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn parse_serie_file(&self, path: &Path) -> Result<SerieData> {
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

        Ok(SerieData {
            id,
            name_en,
            name_ja,
            logo: None,
        })
    }

    fn parse_set_file(&self, path: &Path, serie_id: &str) -> Result<SetData> {
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

        let release_date = json["releaseDate"]
            .as_str()
            .map(|s| s.to_string());

        let tcg_online = json["tcgOnline"]
            .as_str()
            .map(|s| s.to_string());

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

    fn parse_card_file(&self, path: &Path, lang_code: &str) -> Result<CardData> {
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

        let category = json["category"]
            .as_str()
            .unwrap_or("Pokemon")
            .to_string();

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

        let rarity = json["rarity"]
            .as_str()
            .map(|s| s.to_string());

        let stage = json["stage"]
            .as_str()
            .map(|s| s.to_string());

        let evolves_from = json["evolveFrom"].get(lang_code)
            .and_then(|v| v.as_str())
            .or_else(|| json["evolveFrom"]["en"].as_str())
            .map(|s| s.to_string());

        let illustrator = json["illustrator"]
            .as_str()
            .map(|s| s.to_string());

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
