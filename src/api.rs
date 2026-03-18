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

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct Serie {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub logo: Option<String>,
    pub sets: Vec<SetListItem>,
}

#[allow(dead_code)]
#[derive(Debug, Clone, Deserialize)]
pub struct SetListItem {
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
    #[serde(deserialize_with = "deserialize_hp")]
    pub hp: Option<i32>,
    #[serde(default)]
    pub types: Option<Vec<String>>,
    pub rarity: String,
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
    pub async fn fetch(card_id: &str, lang: &str) -> anyhow::Result<Self> {
        let url = format!("https://api.tcgdex.net/v2/{}/cards/{}", lang, card_id);
        let response = CLIENT.get(&url).send().await?;
        
        let status = response.status();
        let text = response.text().await?;
        
        if !status.is_success() {
            tracing::warn!("API error for {}: {} - {}", url, status, text);
            anyhow::bail!("API returned status {} for {}", status, url);
        }
        
        serde_json::from_str::<CardDetails>(&text).map_err(|e| {
            tracing::error!("Failed to parse JSON for {}: {}\nRaw response:\n{}", url, e, text);
            anyhow::anyhow!("Failed to parse {}: {}", url, e)
        })
    }
}

impl Serie {
    pub async fn get(serie_id: &str, lang: &str) -> anyhow::Result<Self> {
        let url = format!("https://api.tcgdex.net/v2/{}/series/{}", lang, serie_id);
        let response = CLIENT.get(&url).send().await?;
        
        let status = response.status();
        let text = response.text().await?;
        
        if !status.is_success() {
            tracing::warn!("API error for {}: {} - {}", url, status, text);
            anyhow::bail!("API returned status {} for {}", status, url);
        }
        
        serde_json::from_str::<Serie>(&text).map_err(|e| {
            tracing::error!("Failed to parse JSON for {}: {}\nRaw response:\n{}", url, e, text);
            anyhow::anyhow!("Failed to parse {}: {}", url, e)
        })
    }
}

impl Serie {
    pub async fn list(lang: &str) -> anyhow::Result<Vec<SerieListItem>> {
        let url = format!("https://api.tcgdex.net/v2/{}/series", lang);
        let response = CLIENT.get(&url).send().await?;
        
        let status = response.status();
        let text = response.text().await?;
        
        if !status.is_success() {
            tracing::warn!("API error for {}: {} - {}", url, status, text);
            anyhow::bail!("API returned status {} for {}", status, url);
        }
        
        serde_json::from_str::<Vec<SerieListItem>>(&text).map_err(|e| {
            tracing::error!("Failed to parse JSON for {}: {}\nRaw response:\n{}", url, e, text);
            anyhow::anyhow!("Failed to parse {}: {}", url, e)
        })
    }
}

impl Set {
    pub async fn get(set_id: &str, lang: &str) -> anyhow::Result<Self> {
        let url = format!("https://api.tcgdex.net/v2/{}/sets/{}", lang, set_id);
        let response = CLIENT.get(&url).send().await?;
        
        let status = response.status();
        let text = response.text().await?;
        
        if !status.is_success() {
            tracing::warn!("API error for {}: {} - {}", url, status, text);
            anyhow::bail!("API returned status {} for {}", status, url);
        }
        
        serde_json::from_str::<Set>(&text).map_err(|e| {
            tracing::error!("Failed to parse JSON for {}: {}\nRaw response:\n{}", url, e, text);
            anyhow::anyhow!("Failed to parse {}: {}", url, e)
        })
    }
}
