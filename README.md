# Pokemon Card Tracker

Track your Pokemon TCG collection by national dex number. Fetches card data from the TCGdex API and supports both English (EN) and Japanese (JA) language sets.

The real highlight for collectors that like to rip packs is the `stats --sets` command, which lists how many of your missing cards can be found in each set.

## Build

```bash
cargo build --release
```

The executable will be at `target/release/poketrack` (or `target/debug/poketrack` for debug builds).

## Setup

On first run, the database is created automatically. Fetch card data with:

```bash
./poketrack --force --update-tcgdex
```

This fetches all sets and cards from both EN and JA APIs. Use `--force` for a full refresh or run without it for incremental updates.

## Usage

```bash
# Add Pokemon to collection (supports ranges)
./poketrack add 25          # Add Pikachu
./poketrack add 1-20        # Add Bulbasaur through Squirtle
./poketrack add 1,4,7       # Add Bulbasaur, Charmander, Squirtle

# Remove Pokemon from collection
./poketrack remove 25

# List all cards for a Pokemon
./poketrack list --dex 25

# Show Pokedex completion
./poketrack stats

# Show missing Pokemon by set
./poketrack stats --sets

# Show all missing Pokemon
./poketrack missing
```

## Database

By default, data is stored in `poketrack.sqlite`. Override with:

```bash
./poketrack --db /path/to/database.sqlite [command]
```
