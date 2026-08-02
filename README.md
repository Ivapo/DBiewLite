# DBiewLite

**[Website](https://ivapo.github.io/DBiewLite/)**

A friendly, read-only database viewer for data analysis. Open SQLite, DuckDB, and Parquet files — browse tables, inspect schemas, run SQL queries, and export to CSV — in a desktop GUI, your terminal, or the browser.

Built with Tauri v2 + Rust + TypeScript.

## Features

- Open `.sqlite` / `.db` / `.duckdb` / `.parquet` files and browse tables with row counts
- Paginated, sortable data grid
- Schema inspector showing column types, primary keys, nullability
- SQL query panel with results grid
- Export tables to CSV
- Terminal UI (TUI) with the same feature set
- Web demo — runs entirely in the browser via WASM
- Four themes: dark, light, 3.1, tui
- Read-only — your data is never modified

## Download

Download the latest release from the [website](https://ivapo.github.io/DBiewLite/) or the [releases page](https://github.com/Ivapo/DBiewLite/releases/latest).

**macOS:** If you see "app is damaged", open Terminal and run:
```bash
xattr -cr /Applications/DBiewLite.app
```

## Terminal UI

Install via Cargo:

```bash
cargo install dbiewlite-tui
```

Then run:

```bash
dbiew path/to/database.sqlite    # or .duckdb, .parquet
```

See the [TUI README](crates/dbiewlite-tui/README.md) for details.

## Keyboard Shortcuts

| Action | GUI (Mac) | GUI (Win/Linux) | TUI |
|---|---|---|---|
| Open database | `Cmd+O` | `Ctrl+O` | CLI arg |
| Toggle sidebar | `Cmd+B` | `Ctrl+B` | `Ctrl+B` |
| Switch panel | Click | Click | `Tab` |
| Navigate up/down | — | — | `↑`/`↓` or `k`/`j` |
| Move column cursor | — | — | `←`/`→` or `h`/`l` |
| Pan sideways | — | — | `Shift`+`←`/`→` or `H`/`L` |
| Sort column | Click header | Click header | `s` |
| Half screen up/down | — | — | `Ctrl+U`/`Ctrl+D` |
| Prev/next page | Click `◀`/`▶` | Click `◀`/`▶` | `PgUp`/`PgDn` or `[`/`]` |
| Enter query mode | Click textarea | Click textarea | `/` or `:` |
| Run query | `Cmd+Enter` | `Ctrl+Enter` | `Enter` |
| Export to CSV | `Cmd+E` | `Ctrl+E` | `Ctrl+E` |
| Database details | — | — | `i` |
| Show help | — | — | `?` |
| Cycle theme | `Cmd+T` | `Ctrl+T` | — |
| Quit | — | — | `q` |

## Tech Stack

- **Backend:** Rust (Tauri v2, rusqlite with bundled SQLite, duckdb with bundled DuckDB)
- **Frontend:** TypeScript + HTML/CSS (no framework)
- **TUI:** ratatui + crossterm
- **Web:** sql.js (SQLite compiled to WASM)

## Building from Source

### Prerequisites

- [Rust](https://rustup.rs/)
- [Bun](https://bun.sh/)

### GUI (Desktop App)

```bash
bun install
bun run tauri dev      # development
bun run tauri build    # production
```

### TUI (Terminal)

```bash
cargo run -p dbiewlite-tui -- path/to/database.sqlite  # or .duckdb, .parquet
```

Need something to browse? This generates a demo database that exercises every
feature — 28 tables, a 27-column table, 12k rows, NULLs, BLOBs and wide
characters. The file itself is not tracked:

```bash
python3 scripts/make_sample_db.py
cargo run -p dbiewlite-tui -- samples/sample.db
```

## Support

If you find DBiewLite useful, consider [supporting development on Ko-fi](https://ko-fi.com/ivapo).

## License

[MIT](LICENSE)
