use anyhow::Result;
use sqlx::SqlitePool;
use tcgdex_sdk::{Card as TcgdexCard, Serie as TcgdexSerie, Set as TcgdexSet};

use super::models::{Card, PokedexCompletion, Series, Set as DbSet};

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
            SELECT DISTINCT pi.dex_id
            FROM pokemon_index pi
            LEFT JOIN collected_pokemon cp ON pi.dex_id = cp.dex_id
            WHERE cp.dex_id IS NULL
            ORDER BY pi.dex_id
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
                COUNT(DISTINCT pi.dex_id) as total
            FROM collected_pokemon cp
            RIGHT JOIN pokemon_index pi ON cp.dex_id = pi.dex_id
            "#,
        )
        .fetch_one(&self.pool)
        .await?;

        Ok(PokedexCompletion {
            collected: result.0,
            total: result.1,
        })
    }

    pub async fn get_all_series(&self) -> Result<Vec<Series>> {
        let series = sqlx::query_as::<_, Series>(
            "SELECT id, name, logo, symbol FROM series ORDER BY name",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(series)
    }

    pub async fn get_all_sets(&self) -> Result<Vec<DbSet>> {
        let sets = sqlx::query_as::<_, DbSet>(
            "SELECT id, name, logo, symbol, serie_id, release_date, tcg_online, total_cards FROM sets ORDER BY release_date DESC",
        )
        .fetch_all(&self.pool)
        .await?;

        Ok(sets)
    }

    pub async fn get_cards_by_set(&self, set_id: &str) -> Result<Vec<Card>> {
        let cards = sqlx::query_as::<_, Card>(
            "SELECT id, set_id, local_id, name, category, hp, types, rarity, image, stage, evolves_from, illustrator, description FROM cards WHERE set_id = ? ORDER BY local_id",
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
            "#,
        )
        .bind(dex_id)
        .fetch_all(&self.pool)
        .await?;

        Ok(cards)
    }

    pub async fn is_pokemon_collected(&self, dex_id: i32) -> Result<bool> {
        let result: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(*) FROM collected_pokemon WHERE dex_id = ?",
        )
        .bind(dex_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(result.map(|r| r.0 > 0).unwrap_or(false))
    }

    pub async fn get_total_pokemon_count(&self) -> Result<i64> {
        let count: Option<(i64,)> = sqlx::query_as(
            "SELECT COUNT(DISTINCT dex_id) FROM pokemon_index",
        )
        .fetch_optional(&self.pool)
        .await?;

        Ok(count.map(|r| r.0).unwrap_or(0))
    }
}
