# dbiewlite-tui

Terminal UI for [DBiewLite](https://ivapo.github.io/DBiewLite/) — a friendly, read-only database viewer for SQLite, DuckDB, and Parquet files.

## Install

```bash
cargo install dbiewlite-tui
```

## Usage

```bash
dbiew path/to/file.sqlite    # or .duckdb, .parquet
dbiew --help
dbiew --version
```

## Features

- Browse tables with row counts — contents load as you move through the list
- Paginated, sortable data grid with horizontal scrolling for wide tables
- Scrollbars on both panels
- Schema inspector (column types, PKs, nullability)
- Details panel (`i`) — size, engine, tables, views, indexes, row totals
- SQL query input with readable, resizable results
- Export a table in the order you are viewing it, or export a query's results
- Vim-style navigation (`j`/`k`, `h`/`l`)
- Mouse and trackpad support

## Keyboard Shortcuts

Press `?` inside the app for the full list.

| Action | Key |
|---|---|
| Help | `?` |
| Database details | `i` |
| Quit | `q` |
| Switch panel | `Tab` |
| Navigate up/down | `↑`/`↓` or `k`/`j` |
| Focus data grid | `Enter` |
| Move column cursor | `←`/`→` or `h`/`l` |
| Pan sideways | `Shift`+`←`/`→` or `H`/`L` |
| Sort by cursor column | `s` |
| First/last column | `Home`/`End` |
| Half screen up/down | `Ctrl+U`/`Ctrl+D` |
| Prev/next page | `PgUp`/`PgDn` or `[`/`]` |
| First/last row | `g`/`G` |
| Enter query mode | `/` or `:` |
| Run query | `Enter` |
| Move through results | `j`/`k`, `g`/`G`, `PgUp`/`PgDn` |
| Grow/restore results | `+`/`-` |
| Clear query and results | `Ctrl+U` |
| Hide results / leave query | `Esc` |
| Toggle tables panel | `Ctrl+B` |
| Export table, or query results | `Ctrl+E` |

## Mouse

| Action | Input |
|---|---|
| Scroll rows | Wheel / two-finger scroll |
| Pan sideways | Horizontal swipe or `Shift`+wheel |
| Select row and column | Click a cell or header |

## License

[MIT](../../LICENSE)
