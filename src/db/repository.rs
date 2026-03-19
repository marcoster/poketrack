use anyhow::Result;
use sqlx::SqlitePool;

use crate::api::{CardDetails, Serie, Set};
use super::models::{Card, CardSetInfo, PokedexCompletion, Series, Set as DbSet};

pub struct Repository {
    pool: SqlitePool,
}

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

    pub async fn upsert_series(&self, series: &Serie) -> Result<()> {
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

    pub async fn upsert_set(&self, set: &Set) -> Result<()> {
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

    pub async fn upsert_card(&self, card: &CardDetails) -> Result<()> {
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
        .bind(&card.set.id)
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
            "SELECT id, name, logo, symbol, serie_id, release_date, tcg_online, total_cards FROM sets ORDER BY release_date DESC",
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
            SELECT c.id as card_id, c.set_id, s.name as set_name, c.local_id, c.rarity
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
}
