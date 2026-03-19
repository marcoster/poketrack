use anyhow::Result;
use sqlx::SqlitePool;
use std::collections::HashSet;

use crate::api::{CardDetailsWithLang, SerieWithLang, SetWithLang};
use super::models::{Card, CardSetInfo, PokedexCompletion, Series, Set as DbSet, SetMissingCardInfo, SetMissingStats};

pub struct Repository {
    pool: SqlitePool,
}

impl Repository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }

    pub async fn ensure_finished_column(&self) -> Result<()> {
        let result = sqlx::query_scalar::<_, i64>(
            "SELECT COUNT(*) FROM pragma_table_info('sets') WHERE name = 'finished'"
        )
        .fetch_one(&self.pool)
        .await?;

        if result == 0 {
            tracing::info!("Adding 'finished' column to sets table...");
            sqlx::query("ALTER TABLE sets ADD COLUMN finished INTEGER NOT NULL DEFAULT 0")
                .execute(&self.pool)
                .await?;
            tracing::info!("'finished' column added successfully");
        }
        Ok(())
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

    pub async fn upsert_series(&self, series: &SerieWithLang) -> Result<()> {
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

    pub async fn upsert_set(&self, set: &SetWithLang) -> Result<()> {
        let total_cards = set.card_count.total;

        sqlx::query(
            r#"
            INSERT INTO sets (id, name, logo, symbol, serie_id, release_date, tcg_online, total_cards, finished, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, 0, CURRENT_TIMESTAMP)
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
        .bind(&set.serie_id)
        .bind(&set.release_date)
        .bind(&set.tcg_online)
        .bind(total_cards)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_set_finished(&self, set_id: &str, lang: &str) -> Result<()> {
        let full_id = format!("{}-{}", lang, set_id);
        sqlx::query("UPDATE sets SET finished = 1 WHERE id = ?")
            .bind(&full_id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    pub async fn get_set_total_cards(&self, set_id: &str, lang: &str) -> Result<Option<i32>> {
        let full_id = format!("{}-{}", lang, set_id);
        let result: Option<(i32,)> = sqlx::query_as(
            "SELECT total_cards FROM sets WHERE id = ?"
        )
        .bind(&full_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|r| r.0))
    }

    pub async fn is_set_finished(&self, set_id: &str, lang: &str) -> Result<bool> {
        let full_id = format!("{}-{}", lang, set_id);
        let result: Option<(i64,)> = sqlx::query_as(
            "SELECT finished FROM sets WHERE id = ?"
        )
        .bind(&full_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|r| r.0 != 0).unwrap_or(false))
    }

    pub async fn upsert_card(&self, card: &CardDetailsWithLang) -> Result<()> {
        let types_json = card.types.as_ref().map(|t| serde_json::to_string(t).ok()).flatten();
        let dex_id = card.dex_ids.as_ref().and_then(|ids| ids.first().copied());
        let category = card.category.clone().unwrap_or_else(|| "Unknown".to_string());

        sqlx::query(
            r#"
            INSERT INTO cards (id, set_id, local_id, name, category, hp, types, dex_id, rarity, image, stage, evolves_from, illustrator, description, updated_at)
            VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, CURRENT_TIMESTAMP)
            ON CONFLICT(id) DO UPDATE SET
                set_id = excluded.set_id,
                local_id = excluded.local_id,
                name = excluded.name,
                category = excluded.category,
                hp = excluded.hp,
                types = excluded.types,
                dex_id = excluded.dex_id,
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
        .bind(&card.set_id)
        .bind(&card.local_id)
        .bind(&card.name)
        .bind(&category)
        .bind(card.hp)
        .bind(&types_json)
        .bind(dex_id)
        .bind(&card.rarity)
        .bind(&card.image)
        .bind(&card.stage)
        .bind(&card.evolves_from)
        .bind(&card.illustrator)
        .bind(&card.description)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn mark_pokemon_collected(&self, dex_id: i32) -> Result<()> {
        sqlx::query(
            r#"
            INSERT OR IGNORE INTO collected_pokemon (dex_id)
            VALUES (?)
            "#,
        )
        .bind(dex_id)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn unmark_pokemon_collected(&self, dex_id: i32) -> Result<()> {
        sqlx::query("DELETE FROM collected_pokemon WHERE dex_id = ?")
            .bind(dex_id)
            .execute(&self.pool)
            .await?;

        Ok(())
    }

    pub async fn get_existing_dex_ids(&self, dex_ids: &[i32]) -> Result<HashSet<i32>> {
        if dex_ids.is_empty() {
            return Ok(HashSet::new());
        }

        let placeholders: Vec<String> = dex_ids.iter().map(|_| "?".to_string()).collect();
        let query = format!(
            "SELECT DISTINCT dex_id FROM cards WHERE dex_id IS NOT NULL AND dex_id IN ({})",
            placeholders.join(", ")
        );

        let mut query_builder = sqlx::query_scalar::<_, i32>(&query);
        for dex_id in dex_ids {
            query_builder = query_builder.bind(dex_id);
        }

        let existing: Vec<i32> = query_builder.fetch_all(&self.pool).await?;
        Ok(existing.into_iter().collect())
    }

    pub async fn get_missing_pokemon(&self) -> Result<Vec<i32>> {
        let missing: Vec<i32> = sqlx::query_scalar(
            r#"
            SELECT DISTINCT c.dex_id
            FROM cards c
            LEFT JOIN collected_pokemon cp ON c.dex_id = cp.dex_id
            WHERE c.dex_id IS NOT NULL AND cp.dex_id IS NULL
            ORDER BY c.dex_id
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(missing)
    }

    pub async fn get_pokedex_completion(&self) -> Result<PokedexCompletion> {
        let result = sqlx::query_as::<_, (i64, i64)>(
            r#"
            SELECT 
                COUNT(DISTINCT cp.dex_id) as collected,
                COUNT(DISTINCT c.dex_id) as total
            FROM cards c
            LEFT JOIN collected_pokemon cp ON c.dex_id = cp.dex_id
            WHERE c.dex_id IS NOT NULL
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(PokedexCompletion {
            collected: result.0,
            total: result.1,
        })
    }

    #[allow(dead_code)]
    pub async fn get_all_series(&self) -> Result<Vec<Series>> {
        let series = sqlx::query_as::<_, Series>(
            "SELECT id, name, logo, symbol FROM series ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(series)
    }

    #[allow(dead_code)]
    pub async fn get_all_sets(&self) -> Result<Vec<DbSet>> {
        let sets = sqlx::query_as::<_, DbSet>(
            "SELECT id, name, logo, symbol, serie_id, release_date, tcg_online, total_cards, finished FROM sets ORDER BY release_date DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(sets)
    }

    #[allow(dead_code)]
    pub async fn get_cards_by_set(&self, set_id: &str) -> Result<Vec<Card>> {
        let cards = sqlx::query_as::<_, Card>(
            "SELECT id, set_id, local_id, name, category, hp, types, dex_id, rarity, image, stage, evolves_from, illustrator, description FROM cards WHERE set_id = ? ORDER BY local_id",
        )
        .bind(set_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(cards)
    }

    #[allow(dead_code)]
    pub async fn get_cards_by_dex_id(&self, dex_id: i32) -> Result<Vec<Card>> {
        let cards = sqlx::query_as::<_, Card>(
            r#"
            SELECT id, set_id, local_id, name, category, hp, types, dex_id, rarity, image, stage, evolves_from, illustrator, description 
            FROM cards
            WHERE dex_id = ?
            ORDER BY set_id, local_id
            "#,
        )
        .bind(dex_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(cards)
    }

    pub async fn get_pokemon_sets(&self, dex_id: i32) -> Result<Vec<CardSetInfo>> {
        let cards = sqlx::query_as::<_, CardSetInfo>(
            r#"
            SELECT c.id as card_id, c.set_id, s.name as set_name, c.local_id, c.rarity, c.dex_id
            FROM cards c
            JOIN sets s ON c.set_id = s.id
            WHERE c.dex_id = ?
            ORDER BY s.release_date DESC, c.local_id
            "#,
        )
        .bind(dex_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(cards)
    }

    pub async fn get_set_missing_stats(&self) -> Result<Vec<SetMissingStats>> {
        let stats = sqlx::query_as::<_, SetMissingStats>(
            r#"
            SELECT 
                s.id as set_id,
                s.name as set_name,
                COUNT(DISTINCT c.dex_id) - COUNT(DISTINCT cp.dex_id) as missing
            FROM sets s
            JOIN cards c ON c.set_id = s.id AND c.dex_id IS NOT NULL
            LEFT JOIN collected_pokemon cp ON c.dex_id = cp.dex_id
            GROUP BY s.id
            HAVING missing > 0
            ORDER BY missing DESC
            "#,
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(stats)
    }

    pub async fn get_set_missing_pokemon_details(&self, set_id: &str) -> Result<Vec<SetMissingCardInfo>> {
        let cards: Vec<SetMissingCardInfo> = sqlx::query_as(
            r#"
            SELECT DISTINCT c.dex_id, t.en_name
            FROM cards c
            LEFT JOIN translations t ON c.dex_id = t.dex_id
            LEFT JOIN collected_pokemon cp ON c.dex_id = cp.dex_id
            WHERE c.set_id = ? AND c.dex_id IS NOT NULL AND cp.dex_id IS NULL
            ORDER BY c.dex_id
            "#
        )
        .bind(set_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(cards)
    }

    pub async fn upsert_translation(&self, dex_id: i32, en_name: &str) -> Result<bool> {
        let result = sqlx::query(
            r#"
            INSERT OR IGNORE INTO translations (dex_id, en_name)
            VALUES (?, ?)
            "#,
        )
        .bind(dex_id)
        .bind(en_name)
        .execute(&self.pool)
        .await?;

        Ok(result.rows_affected() > 0)
    }

    pub async fn get_translation(&self, dex_id: i32) -> Result<Option<String>> {
        let result: Option<(String,)> = sqlx::query_as(
            "SELECT en_name FROM translations WHERE dex_id = ?"
        )
        .bind(dex_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|r| r.0))
    }

    pub async fn get_all_translations(&self) -> Result<std::collections::HashMap<i32, String>> {
        let translations: Vec<(i32, String)> = sqlx::query_as(
            "SELECT dex_id, en_name FROM translations"
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(translations.into_iter().collect())
    }

    pub async fn get_english_pokemon_names(&self) -> Result<Vec<(i32, String)>> {
        let names: Vec<(i32, String)> = sqlx::query_as(
            r#"
            SELECT DISTINCT dex_id, name 
            FROM cards 
            WHERE id LIKE 'en-%' AND dex_id IS NOT NULL
            ORDER BY dex_id
            "#
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(names)
    }

    pub async fn clear_translations(&self) -> Result<()> {
        sqlx::query("DELETE FROM translations")
            .execute(&self.pool)
            .await?;
        Ok(())
    }
}
