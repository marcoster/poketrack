use poketrack::db::{create_pool, initialize_database};
use poketrack::gui::{App, GuiRepository, UiOptions, run_gui};
use std::cell::RefCell;
use std::path::PathBuf;
use std::rc::Rc;
use tracing_subscriber::prelude::*;

fn main() -> anyhow::Result<()> {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(false),
        )
        .with(tracing_subscriber::filter::LevelFilter::INFO)
        .init();

    let db_path = std::env::args().nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| PathBuf::from("poketrack.sqlite"));

    let runtime = tokio::runtime::Runtime::new()?;
    let pool_result: anyhow::Result<sqlx::SqlitePool> = runtime.block_on(async {
        let p = create_pool(&db_path).await?;
        initialize_database(&p).await?;
        Ok(p)
    });
    let pool = pool_result?;

    let repo = GuiRepository::new(pool.clone());
    let app = Rc::new(RefCell::new(App::new(repo)));

    run_gui(app, pool, UiOptions { show_update_button: true, compact_top_bar: false })
}