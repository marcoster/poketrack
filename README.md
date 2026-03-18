# Pokemon Card / Set Tracker

This should get to be a Pokemon card Pokedex tracker with the ability to find the sets with the most missing cards.

This functionality helps a lot to find the best booster packs or boxes to open for missing cards.

## Functionality
  - Pokedex tracker
    - be able to track all pokemon by the national index
    - easy function to add or remove a pokemon by national index
  - Interact with tcgdex
    - fetch available sets and pokemon in those sets
    - generate a report of the missing pokemon and in which set how many of those are found
    - search for this in english and japanese sets

## Technical details
  - command line utility (for now)
    - maybe extend to a GUI using iced-rs in the future
  - store data in a local sqlite database
  - if there is no database available, ask the user if a new one should be created
    - the database file should also be an optional start parameter (--db) but default to poketrack.sqlite
  - data to be stored in the database
    - table of already collected cards
    - it should also cache the data fetched from tcgdex
      - sets with cards contained
      - updating these tables should be done with the start parameter --update-tcgdex
      - collected cards should persist such an update





