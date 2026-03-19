pub mod models;
pub mod repository;
pub mod schema;

use sqlx::sqlite::{SqlitePool, SqlitePoolOptions};
use std::path::Path;
use anyhow::Result;

pub async fn create_pool(db_path: &Path) -> Result<SqlitePool> {
    let url = format!("sqlite:{}?mode=rwc", db_path.display());
    
    let pool = SqlitePoolOptions::new()
        .max_connections(1)
        .connect(&url)
        .await?;
    
    Ok(pool)
}

pub async fn initialize_database(pool: &SqlitePool) -> Result<()> {
    sqlx::query(schema::CREATE_SERIES_TABLE)
        .execute(pool)
        .await?;
    
    sqlx::query(schema::CREATE_SETS_TABLE)
        .execute(pool)
        .await?;
    
    sqlx::query(schema::CREATE_CARDS_TABLE)
        .execute(pool)
        .await?;
    
    sqlx::query(schema::CREATE_POKEMON_INDEX_TABLE)
        .execute(pool)
        .await?;
    
    sqlx::query(schema::CREATE_COLLECTED_POKEMON_TABLE)
        .execute(pool)
        .await?;

    sqlx::query(schema::CREATE_TRANSLATIONS_TABLE)
        .execute(pool)
        .await?;

    tracing::info!("Database schema initialized successfully");
    Ok(())
}
