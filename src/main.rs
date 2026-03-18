mod db;

use anyhow::Result;
use clap::{Parser, Subcommand};
use db::{create_pool, initialize_database, repository::Repository};
use std::path::PathBuf;
use tcgdex_sdk::{Language, TCGdex};
use tracing_subscriber::prelude::*;

#[derive(Parser)]
#[command(name = "poketrack")]
#[command(about = "Pokemon Card Tracker - Track your Pokemon TCG collection", long_about = None)]
struct Cli {
    #[arg(short, long, default_value = "poketrack.sqlite")]
    db: PathBuf,
    
    #[arg(short, long)]
    update_tcgdex: bool,
    
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand)]
enum Commands {
    Add {
        card_id: String,
        #[arg(short, long, default_value = "EN")]
        language: String,
    },
    Remove {
        card_id: String,
    },
    List {
        #[arg(short, long)]
        set: Option<String>,
        #[arg(short, long)]
        dex: Option<i32>,
        #[arg(short, long, default_value = "EN")]
        language: String,
    },
    Missing {
        #[arg(short, long, default_value = "EN")]
        language: String,
    },
    Stats,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::registry()
        .with(tracing_subscriber::fmt::layer())
        .init();

    let cli = Cli::parse();

    if !cli.db.exists() {
        tracing::info!("Database not found, creating new database at {:?}", cli.db);
    }

    let pool = create_pool(&cli.db).await?;
    initialize_database(&pool).await?;

    let repo = Repository::new(pool);

    if cli.update_tcgdex {
        update_tcgdex_cache(&repo).await?;
    }

    if let Some(command) = cli.command {
        match command {
            Commands::Add { card_id, language } => {
                let lang = parse_language(&language)?;
                repo.mark_card_collected(&card_id, lang).await?;
                println!("Added card {} to collection", card_id);
            }
            Commands::Remove { card_id } => {
                repo.unmark_card_collected(&card_id).await?;
                println!("Removed card {} from collection", card_id);
            }
            Commands::List { set, dex, language: _ } => {
                if let Some(set_id) = set {
                    let cards = repo.get_cards_by_set(&set_id).await?;
                    for card in cards {
                        println!("{} - {}", card.id, card.name);
                    }
                } else if let Some(dex_id) = dex {
                    let cards = repo.get_cards_by_dex_id(dex_id).await?;
                    for card in cards {
                        println!("{} - {} ({})", card.id, card.name, card.set_id);
                    }
                }
            }
            Commands::Missing { language } => {
                let missing = repo.get_missing_pokemon_by_dex(Some(&language)).await?;
                if missing.is_empty() {
                    println!("No missing Pokemon! You have them all!");
                } else {
                    println!("Missing Pokemon by National Dex:");
                    for (dex_id, count) in missing.iter().take(20) {
                        println!("  #{:04}: {} cards missing", dex_id, count);
                    }
                    if missing.len() > 20 {
                        println!("  ... and {} more", missing.len() - 20);
                    }
                }
            }
            Commands::Stats => {
                let stats = repo.get_set_completion_stats(None).await?;
                let mut collected_total = 0i64;
                let mut total_cards = 0i32;
                println!("Set Completion Stats:");
                for stat in stats.iter().take(20) {
                    let pct = if stat.total_cards > 0 {
                        (stat.collected_cards as f64 / stat.total_cards as f64 * 100.0).round()
                    } else {
                        0.0
                    };
                    println!("  {}: {}/{} ({:.0}%)", stat.set_name, stat.collected_cards, stat.total_cards, pct);
                    collected_total += stat.collected_cards;
                    total_cards += stat.total_cards;
                }
                if stats.len() > 20 {
                    println!("  ... and {} more sets", stats.len() - 20);
                }
                let overall_pct = if total_cards > 0 {
                    (collected_total as f64 / total_cards as f64 * 100.0).round()
                } else {
                    0.0
                };
                println!("\nOverall: {}/{} ({:.0}%)", collected_total, total_cards, overall_pct);
            }
        }
    }

    Ok(())
}

fn parse_language(lang: &str) -> Result<Language> {
    match lang.to_uppercase().as_str() {
        "EN" => Ok(Language::EN),
        "FR" => Ok(Language::FR),
        "DE" => Ok(Language::DE),
        "ES" => Ok(Language::ES),
        "ES_MX" => Ok(Language::ES_MX),
        "IT" => Ok(Language::IT),
        "PT_BR" => Ok(Language::PT_BR),
        "PT_PT" => Ok(Language::PT_PT),
        "NL" => Ok(Language::NL),
        "PL" => Ok(Language::PL),
        "RU" => Ok(Language::RU),
        "JA" => Ok(Language::JA),
        "KO" => Ok(Language::KO),
        "ZH_TW" => Ok(Language::ZH_TW),
        "ZH_CN" => Ok(Language::ZH_CN),
        "ID" => Ok(Language::ID),
        "TH" => Ok(Language::TH),
        _ => anyhow::bail!("Unsupported language: {}", lang),
    }
}

async fn update_tcgdex_cache(repo: &Repository) -> Result<()> {
    tracing::info!("Starting TCGdex cache update (full refresh)...");

    repo.clear_cache().await?;

    let tcgdex_en = TCGdex::new(Language::EN);

    let series_list = tcgdex_en.serie.list(None).await?;
    let total_series = series_list.len();
    tracing::info!("Found {} series", total_series);

    let mut cards_inserted: u64 = 0;
    let mut cards_skipped: u64 = 0;
    let mut errors: u64 = 0;

    for (series_idx, series_resume) in series_list.iter().enumerate() {
        tracing::info!(
            "Processing series {}/{}: {}",
            series_idx + 1,
            total_series,
            series_resume.name
        );

        let series = match tcgdex_en.serie.get(&series_resume.id).await {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("Failed to fetch series {}: {}", series_resume.id, e);
                errors += 1;
                continue;
            }
        };

        if let Err(e) = repo.upsert_series(&series).await {
            tracing::error!("Failed to save series {}: {}", series.id, e);
            errors += 1;
            continue;
        }

        let total_sets = series.sets.len();

        for (set_idx, set_resume) in series.sets.iter().enumerate() {
            tracing::debug!(
                "Processing set {}/{} in series {}",
                set_idx + 1,
                total_sets,
                series.name
            );

            let set = match tcgdex_en.set.get(&set_resume.id).await {
                Ok(s) => s,
                Err(e) => {
                    tracing::error!("Failed to fetch set {}: {}", set_resume.id, e);
                    errors += 1;
                    continue;
                }
            };

            if let Err(e) = repo.upsert_set(&set).await {
                tracing::error!("Failed to save set {}: {}", set.id, e);
                errors += 1;
                continue;
            }

            for card_resume in &set.cards {
                match tcgdex_en.card.get(&card_resume.id).await {
                    Ok(card) => match repo.upsert_card(&card).await {
                        Ok(_) => cards_inserted += 1,
                        Err(e) => {
                            tracing::warn!("Failed to save card {}: {}", card.id, e);
                            cards_skipped += 1;
                        }
                    },
                    Err(e) => {
                        tracing::warn!("Failed to fetch card {}: {}", card_resume.id, e);
                        cards_skipped += 1;
                    }
                }
            }

            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }

        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    tracing::info!(
        "TCGdex cache update complete! Cards inserted: {}, Skipped: {}, Errors: {}",
        cards_inserted,
        cards_skipped,
        errors
    );
    Ok(())
}
