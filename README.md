# DBiewLite

**[Website](https://ivapo.github.io/DBiewLite/)**

A friendly, read-only database viewer for data analysis. Open SQLite, DuckDB, and Parquet files — browse tables, inspect schemas, run SQL queries, and export to CSV — in a desktop GUI, your terminal, or the browser.

Built with Tauri v2 + Rust + TypeScript.

## Features

- Open `.sqlite` / `.db` / `.duckdb` / `.parquet` files and browse tables with row counts
- Paginated, sortable data grid with a cell cursor — scrolling past an edge turns the page
- Column details and database details, each a keystroke away
- SQL query panel, opened when wanted, with readable and resizable results
- Export a table in the order you are viewing it, or export a query's results
- Terminal UI (TUI) with the same feature set
- Web demo — runs entirely in the browser via WASM
- Three themes: dark, light, 3.1
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

Press `?` in either the GUI or the TUI for the full list, which is generated
from the bindings themselves.

| Action | GUI (Mac) | GUI (Win/Linux) | TUI |
|---|---|---|---|
| Open database | `Cmd+O` | `Ctrl+O` | CLI arg |
| Toggle sidebar | `Cmd+B` | `Ctrl+B` | `Ctrl+B` |
| Focus the table list | `Tab` | `Tab` | `Tab` |
| Leave the list for the grid | `Enter` | `Enter` | `Enter` |
| Navigate up/down | `↑`/`↓` or `k`/`j` | `↑`/`↓` or `k`/`j` | `↑`/`↓` or `k`/`j` |
| Move column cursor | `←`/`→` or `h`/`l` | `←`/`→` or `h`/`l` | `←`/`→` or `h`/`l` |
| Pan sideways | `Shift`+`←`/`→` or `H`/`L` | `Shift`+`←`/`→` or `H`/`L` | `Shift`+`←`/`→` or `H`/`L` |
| First/last row | `g`/`G` | `g`/`G` | `g`/`G` |
| Sort column | `s` or click header | `s` or click header | `s` |
| Half screen up/down | `Ctrl+U`/`Ctrl+D` | `Ctrl+U`/`Ctrl+D` | `Ctrl+U`/`Ctrl+D` |
| Prev/next page | `[`/`]`, `PgUp`/`PgDn`, or scroll to an edge | same | `PgUp`/`PgDn` or `[`/`]` |
| Open the query panel | `/` or `:` | `/` or `:` | `/` or `:` |
| Run query | `Enter` | `Enter` | `Enter` |
| New line in a query | `Shift+Enter` | `Shift+Enter` | — |
| Clear query and results | Clear button | Clear button | `Ctrl+U` |
| Resize results | drag the divider | drag the divider | `+`/`-` |
| Close the query panel | `Esc` | `Esc` | `Esc` |
| Export to CSV | `Cmd+E` | `Ctrl+E` | `Ctrl+E` |
| Database details | `i` | `i` | `i` |
| Column details | `c` | `c` | — |
| Show help | `?` | `?` | `?` |
| Cycle theme | `Cmd+T` | `Ctrl+T` | — |
| Quit | `Cmd+Q` | `Ctrl+Q` | `q` |

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
