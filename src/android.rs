use slint::android::AndroidApp;
use tracing_subscriber::prelude::*;

use crate::gui::GuiRepository;

#[unsafe(no_mangle)]
fn android_main(app: AndroidApp) {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::fmt::layer()
                .with_target(false)
                .with_thread_ids(false),
        )
        .with(tracing_subscriber::filter::LevelFilter::INFO)
        .init();

    let data_dir = app.internal_data_path();
    slint::android::init(app).unwrap();

    let db_path = match data_dir {
        Some(dir) => {
            use std::fs;
            let _ = fs::create_dir_all(&dir);
            dir.join("poketrack.sqlite")
        }
        None => std::path::PathBuf::from("poketrack.sqlite"),
    };

    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();

    let result: anyhow::Result<sqlx::SqlitePool> = runtime.block_on(async {
        let pool = crate::db::create_pool(&db_path).await?;
        crate::db::initialize_database(&pool).await?;
        anyhow::Result::Ok(pool)
    });

    let pool = match result {
        Ok(pool) => pool,
        Err(e) => {
            tracing::error!("Failed to open database: {e}");
            return;
        }
    };

    let repo = GuiRepository::new(pool);
    let _app = crate::gui::App::new(repo);
    // GUI window / event loop to be wired to the Slint UI component.
}
