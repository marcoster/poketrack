use anyhow::Result;
use sqlx::SqlitePool;
use tcgdex_sdk::{Card as TcgdexCard, Serie as TcgdexSerie, Set as TcgdexSet, Language};

use super::models::{Card, CollectedCard, Series, Set as DbSet};

pub struct Repository {
    pool: SqlitePool,
}

    #[allow(dead_code)]
    impl Repository {
        pub fn new(pool: SqlitePool) -> Self {
            Self { pool }
        }

        pub async fn clear_cache(&self) -> Result<()> {
            sqlx::query("DELETE FROM pokemon_index")
                .execute(&self.pool)
                .await?;
            sqlx::query("DELETE FROM cards")
                .execute(&self.pool)
                .await?;
            sqlx::query("DELETE FROM sets")
                .execute(&self.pool)
                .await?;
            sqlx::query("DELETE FROM series")
                .execute(&self.pool)
                .await?;
            tracing::info!("Cache cleared successfully");
            Ok(())
        }

        pub async fn upsert_series(&self, series: &TcgdexSerie) -> Result<()> {
        sqlx::query(
            r#"
            INSERT INTO series (id, name, logo, updated_at)
            VALUES (?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                logo = excluded.logo,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&series.id)
        .bind(&series.name)
        .bind(&series.logo)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn upsert_set(&self, set: &TcgdexSet) -> Result<()> {
        let total_cards = set.card_count.total;
        
        sqlx::query(
            r#"
            INSERT INTO sets (id, name, logo, symbol, serie_id, release_date, tcg_online, total_cards, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(id) DO UPDATE SET
                name = excluded.name,
                logo = excluded.logo,
                symbol = excluded.symbol,
                serie_id = excluded.serie_id,
                release_date = excluded.release_date,
                tcg_online = excluded.tcg_online,
                total_cards = excluded.total_cards,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&set.id)
        .bind(&set.name)
        .bind(&set.logo)
        .bind(&set.symbol)
        .bind(&set.serie.id)
        .bind(&set.release_date)
        .bind(&set.tcg_online)
        .bind(total_cards)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn upsert_card(&self, card: &TcgdexCard) -> Result<()> {
        let types_json = card.types.as_ref().map(|t| serde_json::to_string(t).ok()).flatten();

        sqlx::query(
            r#"
            INSERT INTO cards (id, set_id, local_id, name, category, hp, types, rarity, image, stage, evolves_from, illustrator, description, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(id) DO UPDATE SET
                set_id = excluded.set_id,
                local_id = excluded.local_id,
                name = excluded.name,
                category = excluded.category,
                hp = excluded.hp,
                types = excluded.types,
                rarity = excluded.rarity,
                image = excluded.image,
                stage = excluded.stage,
                evolves_from = excluded.evolves_from,
                illustrator = excluded.illustrator,
                description = excluded.description,
                updated_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(&card.id)
        .bind(&card.set.id)
        .bind(&card.local_id)
        .bind(&card.name)
        .bind(&card.category)
        .bind(card.hp)
        .bind(&types_json)
        .bind(&card.rarity)
        .bind(&card.image)
        .bind(&card.stage)
        .bind(&card.evolves_from)
        .bind(&card.illustrator)
        .bind(&card.description)
        .execute(&self.pool)
        .await?;

        if let Some(dex_ids) = &card.dex_ids {
            for dex_id in dex_ids {
                sqlx::query(
                    r#"
                    INSERT INTO pokemon_index (card_id, dex_id)
                    VALUES (?, ?)
                    ON CONFLICT(card_id, dex_id) DO NOTHING
                    "#,
                )
                .bind(&card.id)
                .bind(dex_id)
                .execute(&self.pool)
                .await?;
            }
        }

        Ok(())
    }

    pub async fn mark_card_collected(&self, card_id: &str, language: Language) -> Result<()> {
        let lang_str = match language {
            Language::EN => "EN",
            Language::FR => "FR",
            Language::DE => "DE",
            Language::ES => "ES",
            Language::ES_MX => "ES_MX",
            Language::IT => "IT",
            Language::PT_BR => "PT_BR",
            Language::PT_PT => "PT_PT",
            Language::NL => "NL",
            Language::PL => "PL",
            Language::RU => "RU",
            Language::JA => "JA",
            Language::KO => "KO",
            Language::ZH_TW => "ZH_TW",
            Language::ZH_CN => "ZH_CN",
            Language::ID => "ID",
            Language::TH => "TH",
        };

        sqlx::query(
            r#"
            INSERT INTO collected_cards (card_id, language)
            VALUES (?, ?)
            ON CONFLICT(card_id) DO UPDATE SET
                language = excluded.language,
                collected_at = CURRENT_TIMESTAMP
            "#,
        )
        .bind(card_id)
        .bind(lang_str)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn unmark_card_collected(&self, card_id: &str) -> Result<()> {
        sqlx::query("DELETE FROM collected_cards WHERE card_id = ?")
            .bind(card_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_collected_cards(&self, language: Option<&str>) -> Result<Vec<CollectedCard>> {
        let lang_filter = language.unwrap_or("EN");
        
        let cards = sqlx::query_as::<_, CollectedCard>(
            "SELECT card_id, language FROM collected_cards WHERE language = ?"
        )
        .bind(lang_filter)
        .fetch_all(&self.pool)
        .await?;

        Ok(cards)
    }

    pub async fn get_missing_pokemon_by_dex(
        &self, 
        language: Option<&str>
    ) -> Result<Vec<(i32, i64)>> {
        let lang_filter = language.unwrap_or("EN");
        
        let results: Vec<MissingPokemonRow> = sqlx::query_as(
            r#"
            SELECT 
                pi.dex_id,
                COUNT(DISTINCT pi.card_id) as total_cards,
                COUNT(cc.card_id) as collected_count
            FROM pokemon_index pi
            LEFT JOIN cards c ON pi.card_id = c.id
            LEFT JOIN collected_cards cc ON pi.card_id = cc.card_id AND cc.language = ?
            GROUP BY pi.dex_id
            HAVING collected_count < total_cards
            ORDER BY pi.dex_id
            "#,
        )
        .bind(lang_filter)
        .fetch_all(&self.pool)
        .await?;

        let missing: Vec<(i32, i64)> = results
            .into_iter()
            .map(|row| (row.dex_id, row.total_cards - row.collected_count))
            .collect();

        Ok(missing)
    }

    pub async fn get_all_series(&self) -> Result<Vec<Series>> {
        let series = sqlx::query_as::<_, Series>("SELECT id, name, logo, symbol FROM series ORDER BY name")
            .fetch_all(&self.pool)
            .await?;

        Ok(series)
    }

    pub async fn get_all_sets(&self) -> Result<Vec<DbSet>> {
        let sets = sqlx::query_as::<_, DbSet>(
            "SELECT id, name, logo, symbol, serie_id, release_date, tcg_online, total_cards FROM sets ORDER BY release_date DESC"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(sets)
    }

    pub async fn get_cards_by_set(&self, set_id: &str) -> Result<Vec<Card>> {
        let cards = sqlx::query_as::<_, Card>(
            "SELECT id, set_id, local_id, name, category, hp, types, rarity, image, stage, evolves_from, illustrator, description FROM cards WHERE set_id = ? ORDER BY local_id"
        )
        .bind(set_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(cards)
    }

    pub async fn get_cards_by_dex_id(&self, dex_id: i32) -> Result<Vec<Card>> {
        let cards = sqlx::query_as::<_, Card>(
            r#"
            SELECT c.id, c.set_id, c.local_id, c.name, c.category, c.hp, c.types, c.rarity, c.image, c.stage, c.evolves_from, c.illustrator, c.description 
            FROM cards c
            JOIN pokemon_index pi ON c.id = pi.card_id
            WHERE pi.dex_id = ?
            ORDER BY c.set_id, c.local_id
            "#
        )
        .bind(dex_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(cards)
    }

    pub async fn get_set_completion_stats(
        &self,
        language: Option<&str>
    ) -> Result<Vec<SetCompletionStats>> {
        let lang_filter = language.unwrap_or("EN");
        
        let stats = sqlx::query_as::<_, SetCompletionStats>(
            r#"
            SELECT 
                s.id as set_id,
                s.name as set_name,
                s.total_cards,
                COUNT(cc.card_id) as collected_cards
            FROM sets s
            LEFT JOIN collected_cards cc ON s.id = substr(cc.card_id, 1, length(s.id)) AND cc.language = ?
            GROUP BY s.id
            ORDER BY s.release_date DESC
            "#,
        )
        .bind(lang_filter)
        .fetch_all(&self.pool)
        .await?;

        Ok(stats)
    }

    pub async fn series_exists(&self, series_id: &str) -> Result<bool> {
        let result: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM series WHERE id = ?"
        )
        .bind(series_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|r| r.0 > 0).unwrap_or(false))
    }

    pub async fn set_exists(&self, set_id: &str) -> Result<bool> {
        let result: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM sets WHERE id = ?"
        )
        .bind(set_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|r| r.0 > 0).unwrap_or(false))
    }

    pub async fn card_exists(&self, card_id: &str) -> Result<bool> {
        let result: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM cards WHERE id = ?"
        )
        .bind(card_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|r| r.0 > 0).unwrap_or(false))
    }
}

#[derive(Debug, Clone, sqlx::FromRow)]
pub struct SetCompletionStats {
    #[allow(dead_code)]
    pub set_id: String,
    pub set_name: String,
    pub total_cards: i32,
    pub collected_cards: i64,
}

#[derive(Debug, Clone, sqlx::FromRow)]
struct MissingPokemonRow {
    dex_id: i32,
    total_cards: i64,
    collected_count: i64,
}
