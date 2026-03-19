use once_cell::sync::Lazy;
use reqwest::Client;
use serde::Deserialize;
use serde::de::{self, Visitor};
use std::fmt;

static CLIENT: Lazy<Client> = Lazy::new(|| {
    Client::builder()
        .user_agent("poketrack/0.1.0")
        .build()
        .unwrap()
});

pub fn get_client() -> &'static Client {
    &CLIENT
}

#[allow(dead_code)]
pub fn create_client() -> Client {
    Client::builder()
        .user_agent("poketrack/0.1.0")
        .build()
        .unwrap()
}

fn deserialize_hp<'de, D>(deserializer: D) -> Result<Option<i32>, D::Error>
where
    D: de::Deserializer<'de>,
{
    struct HpVisitor;

    impl<'de> Visitor<'de> for HpVisitor {
        type Value = Option<i32>;

        fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
            formatter.write_str("i64, string, or null")
        }

        fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value >= i32::MIN as i64 && value <= i32::MAX as i64 {
                Ok(Some(value as i32))
            } else {
                Err(E::custom(format!("integer out of range: {}", value)))
            }
        }

        fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            if value <= i32::MAX as u64 {
                Ok(Some(value as i32))
            } else {
                Err(E::custom(format!("integer out of range: {}", value)))
            }
        }

        fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            match value.parse::<i32>() {
                Ok(val) => Ok(Some(val)),
                Err(_) => Ok(None),
            }
        }

        fn visit_none<E>(self) -> Result<Self::Value, E>
        where
            E: de::Error,
        {
            Ok(None)
        }

        fn visit_some<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
        where
            D: de::Deserializer<'de>,
        {
            deserializer.deserialize_i64(self)
        }
    }

    deserializer.deserialize_any(HpVisitor)
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct SetResume {
    pub id: String,
    pub name: String,
    #[serde(rename = "cardCount")]
    pub card_count: CardCountResume,
    #[serde(rename = "tcgOnline", default)]
    pub tcg_online: Option<String>,
    #[serde(rename = "releaseDate", default)]
    pub release_date: Option<String>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct CardCountResume {
    pub total: u16,
    pub official: u16,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct SerieResume {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub logo: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct SerieListItem {
    pub id: String,
    pub name: String,
}

impl SerieListItem {
    pub fn with_lang(&self, _lang: &str) -> SerieListItemWithLang {
        SerieListItemWithLang {
            id: self.id.clone(),
            name: self.name.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SerieListItemWithLang {
    pub id: String,
    pub name: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Serie {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub logo: Option<String>,
    pub sets: Vec<SerieListItem>,
}

impl Serie {
    pub fn with_lang(&self, lang: &str) -> SerieWithLang {
        SerieWithLang {
            id: format!("{}-{}", lang, self.id),
            name: self.name.clone(),
            logo: self.logo.clone(),
            sets: self.sets.iter().map(|s| s.with_lang(lang)).collect(),
            language: lang.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SerieWithLang {
    pub id: String,
    pub name: String,
    pub logo: Option<String>,
    pub sets: Vec<SerieListItemWithLang>,
    pub language: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct SetListItem {
    pub id: String,
    pub name: String,
}

impl SetListItem {
    pub fn with_lang(&self, lang: &str) -> SetListItemWithLang {
        SetListItemWithLang {
            id: format!("{}-{}", lang, self.id),
            name: self.name.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SetListItemWithLang {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Set {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub logo: Option<String>,
    #[serde(default)]
    pub symbol: Option<String>,
    #[serde(rename = "cardCount")]
    pub card_count: CardCountResume,
    pub serie: SerieResume,
    #[serde(rename = "tcgOnline", default)]
    pub tcg_online: Option<String>,
    #[serde(rename = "releaseDate")]
    pub release_date: String,
    #[serde(default)]
    pub cards: Vec<CardResume>,
}

impl Set {
    pub fn with_lang(&self, lang: &str) -> SetWithLang {
        SetWithLang {
            id: format!("{}-{}", lang, self.id),
            name: self.name.clone(),
            logo: self.logo.clone(),
            symbol: self.symbol.clone(),
            card_count: self.card_count.clone(),
            serie_id: format!("{}-{}", lang, self.serie.id),
            serie_name: self.serie.name.clone(),
            tcg_online: self.tcg_online.clone(),
            release_date: self.release_date.clone(),
            cards: self.cards.iter().map(|c| c.with_lang(lang)).collect(),
            language: lang.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SetWithLang {
    pub id: String,
    pub name: String,
    pub logo: Option<String>,
    pub symbol: Option<String>,
    pub card_count: CardCountResume,
    pub serie_id: String,
    pub serie_name: String,
    pub tcg_online: Option<String>,
    pub release_date: String,
    pub cards: Vec<CardResumeWithLang>,
    pub language: String,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct CardResume {
    pub id: String,
    pub name: String,
    #[serde(rename = "localId")]
    pub local_id: String,
    #[serde(default)]
    pub image: Option<String>,
}

impl CardResume {
    pub fn with_lang(&self, lang: &str) -> CardResumeWithLang {
        CardResumeWithLang {
            raw_id: self.id.clone(),
            id: format!("{}-{}", lang, self.id),
            name: self.name.clone(),
            local_id: self.local_id.clone(),
            image: self.image.clone(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CardResumeWithLang {
    pub raw_id: String,
    pub id: String,
    pub name: String,
    pub local_id: String,
    pub image: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct CardDetails {
    pub id: String,
    #[serde(rename = "localId")]
    pub local_id: String,
    pub name: String,
    #[serde(default)]
    pub category: Option<String>,
    #[serde(rename = "dexId", default)]
    pub dex_ids: Option<Vec<i32>>,
    #[serde(default)]
    pub hp: Option<i32>,
    #[serde(default)]
    pub types: Option<Vec<String>>,
    #[serde(default)]
    pub rarity: Option<String>,
    #[serde(default)]
    pub image: Option<String>,
    #[serde(default)]
    pub stage: Option<String>,
    #[serde(rename = "evolvesFrom", default)]
    pub evolves_from: Option<String>,
    #[serde(default)]
    pub illustrator: Option<String>,
    #[serde(default)]
    pub description: Option<String>,
    pub set: SetResume,
}

impl CardDetails {
    pub fn with_lang(self, lang: &str) -> CardDetailsWithLang {
        CardDetailsWithLang {
            id: format!("{}-{}", lang, self.id),
            local_id: self.local_id,
            name: self.name,
            category: self.category,
            dex_ids: self.dex_ids,
            hp: self.hp,
            types: self.types,
            rarity: self.rarity,
            image: self.image,
            stage: self.stage,
            evolves_from: self.evolves_from,
            illustrator: self.illustrator,
            description: self.description,
            set_id: format!("{}-{}", lang, self.set.id),
            set_name: self.set.name,
            language: lang.to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct CardDetailsWithLang {
    pub id: String,
    pub local_id: String,
    pub name: String,
    pub category: Option<String>,
    pub dex_ids: Option<Vec<i32>>,
    pub hp: Option<i32>,
    pub types: Option<Vec<String>>,
    pub rarity: Option<String>,
    pub image: Option<String>,
    pub stage: Option<String>,
    pub evolves_from: Option<String>,
    pub illustrator: Option<String>,
    pub description: Option<String>,
    pub set_id: String,
    pub set_name: String,
    pub language: String,
}

impl CardDetailsWithLang {
    pub async fn fetch(card_id: &str, lang: &str) -> anyhow::Result<Self> {
        let url = format!("https://api.tcgdex.net/v2/{}/cards/{}", lang, card_id);
        let response = CLIENT.get(&url).send().await?;

        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            tracing::warn!("API error for {}: {} - {}", url, status, text);
            anyhow::bail!("API returned status {} for {}", status, url);
        }

        let card: CardDetails = serde_json::from_str(&text).map_err(|e| {
            tracing::error!(
                "Failed to parse JSON for {}: {}\nRaw response:\n{}",
                url,
                e,
                text
            );
            anyhow::anyhow!("Failed to parse {}: {}", url, e)
        })?;

        Ok(card.with_lang(lang))
    }
}

impl SerieWithLang {
    pub async fn get(serie_id: &str, lang: &str) -> anyhow::Result<Self> {
        let url = format!("https://api.tcgdex.net/v2/{}/series/{}", lang, serie_id);
        let response = CLIENT.get(&url).send().await?;

        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            tracing::warn!("API error for {}: {} - {}", url, status, text);
            anyhow::bail!("API returned status {} for {}", status, url);
        }

        let serie: Serie = serde_json::from_str(&text).map_err(|e| {
            tracing::error!(
                "Failed to parse JSON for {}: {}\nRaw response:\n{}",
                url,
                e,
                text
            );
            anyhow::anyhow!("Failed to parse {}: {}", url, e)
        })?;

        Ok(serie.with_lang(lang))
    }
}

impl SerieWithLang {
    pub async fn list(lang: &str) -> anyhow::Result<Vec<SerieListItemWithLang>> {
        let url = format!("https://api.tcgdex.net/v2/{}/series", lang);
        let response = CLIENT.get(&url).send().await?;

        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            tracing::warn!("API error for {}: {} - {}", url, status, text);
            anyhow::bail!("API returned status {} for {}", status, url);
        }

        let series: Vec<SerieListItem> = serde_json::from_str(&text).map_err(|e| {
            tracing::error!(
                "Failed to parse JSON for {}: {}\nRaw response:\n{}",
                url,
                e,
                text
            );
            anyhow::anyhow!("Failed to parse {}: {}", url, e)
        })?;

        Ok(series.iter().map(|s| s.with_lang(lang)).collect::<Vec<_>>())
    }
}

impl SetWithLang {
    pub async fn get(set_id: &str, lang: &str) -> anyhow::Result<Self> {
        let url = format!("https://api.tcgdex.net/v2/{}/sets/{}", lang, set_id);
        let response = CLIENT.get(&url).send().await?;

        let status = response.status();
        let text = response.text().await?;

        if !status.is_success() {
            tracing::warn!("API error for {}: {} - {}", url, status, text);
            anyhow::bail!("API returned status {} for {}", status, url);
        }

        let set: Set = serde_json::from_str(&text).map_err(|e| {
            tracing::error!(
                "Failed to parse JSON for {}: {}\nRaw response:\n{}",
                url,
                e,
                text
            );
            anyhow::anyhow!("Failed to parse {}: {}", url, e)
        })?;

        Ok(set.with_lang(lang))
    }
}
