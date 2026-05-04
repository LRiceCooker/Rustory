# Rustory

A lightweight, interactive terminal-based RPG campaign manager built in Rust. Manage characters, roll dice, track combat, explore world maps, play ambient sounds, and run custom scripts — all from your terminal.

![Rust](https://img.shields.io/badge/Rust-1.90%2B-orange?logo=rust)
![License](https://img.shields.io/badge/License-Beerware-yellow)
![Status](https://img.shields.io/badge/Status-WIP-blue)

---

## Features

- **Character Management** — Stats, gauges (HP), pools, conditions, inventory, lore, and location tracking for players and NPCs
- **Dice Rolling** — Full dice notation (`2d6+3`, `d20`, `3d8-2`) with implicit detection — just type `2d6` and it rolls
- **Expression Calculator** — Math with inline dice: `calc 2*1d6 + 3`
- **Combat Tracker** — Initiative order, turn management, persistent targeting
- **World Map** — Interactive map viewer with zoom/pan, location database, route finding (supports [Azgaar's Fantasy Map Generator](https://azgaar.github.io/Fantasy-Map-Generator/) JSON export)
- **Encounter Tables** — Weighted random encounter generation by zone
- **Audio Playback** — Ambient sounds and SFX with volume control (MP3, WAV, OGG, FLAC)
- **Custom Scripting** — Extend Rustory with LOLCODE scripts
- **Persistence** — Git-backed campaign versioning with undo/redo
- **Autocomplete** — Context-aware tab completion for commands, characters, files, zones, and locations
- **Campaign Validation** — Schema checking for character sheets and encounter tables

---

## Getting Started

### Requirements

- Rust 1.90+ (use [mise](https://mise.jdx.dev/) or rustup)

### Build & Run

```bash
cargo build --release
cargo run
```

### Load the Sample Campaign

Once inside Rustory:

```
> load sample
```

---

## Campaign Structure

```
my_campaign/
├── rules/
│   ├── system.toml            # Game system definition
│   └── commands/
│       └── *.lol              # LOLCODE custom commands
├── players/
│   └── <character>/
│       └── *.csv              # Player character sheets
├── npc/
│   ├── <npc_template>/
│   │   └── *.csv              # NPC sheets
│   └── encounters/
│       └── *.toml             # Encounter tables
├── map/
│   ├── world.json             # Azgaar map data
│   └── world.png              # Map image
├── sound/
│   ├── ambiance/
│   └── sfx/
└── notes/
    └── *.md                   # Campaign notes
```

---

## Commands

### General

| Command | Alias | Description |
|---------|-------|-------------|
| `help` | `h` | Show all available commands |
| `quit` | `q` | Exit Rustory |
| `clear` | | Clear output history |
| `load <path>` | | Load a campaign folder |
| `new <name> <template>` | | Create a new campaign from a template |
| `validate [path]` | `v` | Validate campaign files against schemas |
| `history [limit]` | `hist` | Show command history |
| `undo` | `u` | Undo last stateful command |
| `redo` | `re` | Redo undone command |

### Dice & Math

| Command | Alias | Description |
|---------|-------|-------------|
| `roll <NdV+M>` | `r` | Roll dice (`r 2d6+3`, `r d20`) |
| `calc <expression>` | | Evaluate math with inline dice (`calc 2*1d6 + 3`) |

Implicit roll detection: typing `2d6+3` directly (without `roll`) works too.

### Characters

| Command | Alias | Description |
|---------|-------|-------------|
| `show <character>` | `s` | Display a character's full sheet |
| `set <character>.<field> <value>` | | Modify a character attribute (`set thorin.hp 35`) |
| `who` | `w` | Player dashboard — HP and status conditions |
| `where` | `wh` | Show all character locations |
| `ls <type>` | `list` | List elements: `players`, `npc`, `sound`, `encounters`, `commands` |
| `damage <character> <amount>` | `dmg` | Deal damage (`dmg thorin 15`) |
| `heal <character> <amount>` | `hp` | Restore HP (`hp thorin 10`) |
| `give <item> <from> <to>` | | Transfer an item between characters |
| `spawn <npc_template> [name]` | `sp` | Create a new NPC instance from a template |
| `cat <file>` | | Display a raw campaign file |

### Combat

| Command | Alias | Description |
|---------|-------|-------------|
| `combat start` | | Enter combat mode, auto-add all players |
| `combat end` | | Exit combat mode |
| `init add <name> <value>` | | Add a combatant with initiative value |
| `init remove <name>` | | Remove a combatant |
| `init roll [modifier]` | | Auto-roll initiative for all characters |
| `next` | `n` | Advance to next combatant |
| `prev` | `p` | Go back to previous combatant |
| `status` | `st` | Show current initiative order |
| `target <name>` | | Set persistent target for damage/heal |

### World Map

| Command | Alias | Description |
|---------|-------|-------------|
| `map` | | Toggle interactive map view (arrow keys to pan, `+`/`-` to zoom) |
| `map list [type]` | | List burgs, states, or cultures |
| `map info <name>` | | Show details about a location |
| `map search <name>` | | Fuzzy search for a location |
| `map near <burg>` | | Find nearby locations |
| `map route <from> <to>` | | Display travel route between locations |
| `map where <character>` | | Show a character's current location |
| `map move <char> <location>` | `mv` | Move a character to a location |

### Encounters

| Command | Alias | Description |
|---------|-------|-------------|
| `encounter ls` | | List all encounter zones |
| `encounter show <zone>` | | Display an encounter table |
| `encounter roll <zone>` | `enc` | Roll a random encounter from a zone |

### Audio

| Command | Alias | Description |
|---------|-------|-------------|
| `sound play <path>` | `play` | Play a sound file (fuzzy matching) |
| `sound loop <path>` | | Loop audio indefinitely |
| `sound pause` | | Pause playback |
| `sound resume` | | Resume playback |
| `sound stop` | | Stop all audio |
| `sound status` | | Show currently playing track |
| `sound volume <0-100>` | | Set volume level |
| `sound search <query>` | | Fuzzy search sound files |
| `sound list [folder]` | | List available sounds |

### Notes & Search

| Command | Alias | Description |
|---------|-------|-------------|
| `note list` | | Show all campaign notes |
| `note history` | | Show recent note changes |
| `search <query>` | | Fuzzy search across PDFs and markdown files |

---

## Quick Alias Reference

| Alias | Command |
|-------|---------|
| `r` | `roll` |
| `s` | `show` |
| `h` | `help` |
| `q` | `quit` |
| `n` | `next` |
| `p` | `prev` |
| `st` | `status` |
| `sp` | `spawn` |
| `enc` | `encounter roll` |
| `dmg` | `damage` |
| `hp` | `heal` |
| `mv` | `map move` |
| `play` | `sound play` |
| `w` | `who` |
| `wh` | `where` |
| `u` | `undo` |
| `re` | `redo` |
| `v` | `validate` |
| `hist` | `history` |
| `list` | `ls` |

---

## Custom Commands (LOLCODE)

Rustory supports custom commands written in LOLCODE. Place `.lol` scripts in your campaign's `rules/commands/` directory — they are loaded automatically and can access game state and audio playback APIs.

```
ls commands
```

---

## Tech Stack

| Crate | Purpose |
|-------|---------|
| [ratatui](https://crates.io/crates/ratatui) | Terminal UI framework |
| [crossterm](https://crates.io/crates/crossterm) | Terminal event handling |
| [rodio](https://crates.io/crates/rodio) | Audio playback |
| [git2](https://crates.io/crates/git2) | Campaign persistence via git |
| [serde](https://crates.io/crates/serde) / [serde_json](https://crates.io/crates/serde_json) | Serialization |
| [toml](https://crates.io/crates/toml) | TOML parsing |
| [csv](https://crates.io/crates/csv) | Character sheet parsing |
| [ratatui-image](https://crates.io/crates/ratatui-image) | Map image rendering |
| [pdf-extract](https://crates.io/crates/pdf-extract) | PDF content search |

---

## License

This project is licensed under the [Beerware License](LICENSE) (Revision 42).

In short: do whatever you want with this. If we meet some day and you think it's worth it, you can buy me a beer.
