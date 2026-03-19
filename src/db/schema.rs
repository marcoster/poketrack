pub const CREATE_SERIES_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS series (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    logo TEXT,
    symbol TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP
)
"#;

pub const CREATE_SETS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS sets (
    id TEXT PRIMARY KEY,
    name TEXT NOT NULL,
    logo TEXT,
    symbol TEXT,
    serie_id TEXT NOT NULL,
    release_date TEXT NOT NULL,
    tcg_online TEXT,
    total_cards INTEGER NOT NULL DEFAULT 0,
    finished INTEGER NOT NULL DEFAULT 0,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (serie_id) REFERENCES series(id)
)
"#;

pub const CREATE_CARDS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS cards (
    id TEXT PRIMARY KEY,
    set_id TEXT NOT NULL,
    local_id TEXT NOT NULL,
    name TEXT NOT NULL,
    category TEXT NOT NULL,
    hp INTEGER,
    types TEXT,
    dex_id INTEGER,
    rarity TEXT NOT NULL,
    image TEXT,
    stage TEXT,
    evolves_from TEXT,
    illustrator TEXT,
    description TEXT,
    created_at TEXT DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (set_id) REFERENCES sets(id)
)
"#;

pub const CREATE_POKEMON_INDEX_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS pokemon_index (
    card_id TEXT NOT NULL,
    dex_id INTEGER NOT NULL,
    PRIMARY KEY (card_id, dex_id),
    FOREIGN KEY (card_id) REFERENCES cards(id)
)
"#;

pub const CREATE_COLLECTED_POKEMON_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS collected_pokemon (
    dex_id INTEGER PRIMARY KEY,
    collected_at TEXT DEFAULT CURRENT_TIMESTAMP
)
"#;

#[allow(dead_code)]
pub const CREATE_INDEXES: &[(&str, &str)] = &[
    (
        "idx_cards_set_id",
        "CREATE INDEX IF NOT EXISTS idx_cards_set_id ON cards(set_id)",
    ),
    (
        "idx_cards_name",
        "CREATE INDEX IF NOT EXISTS idx_cards_name ON cards(name)",
    ),
    (
        "idx_pokemon_index_dex_id",
        "CREATE INDEX IF NOT EXISTS idx_pokemon_index_dex_id ON pokemon_index(dex_id)",
    ),
    (
        "idx_sets_serie_id",
        "CREATE INDEX IF NOT EXISTS idx_sets_serie_id ON sets(serie_id)",
    ),
];
