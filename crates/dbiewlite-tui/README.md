# dbiewlite-tui

Terminal UI for [DBiewLite](https://ivapo.github.io/DBiewLite/) — a friendly, read-only SQLite database viewer.

## Install

```bash
cargo install dbiewlite-tui
```

## Usage

```bash
dbiew path/to/database.sqlite
```

## Features

- Browse tables with row counts
- Paginated, sortable data grid
- Schema inspector (column types, PKs, nullability)
- SQL query input with results
- Export tables to CSV
- Vim-style navigation (`j`/`k`, `h`/`l`)

## Keyboard Shortcuts

| Action | Key |
|---|---|
| Quit | `q` |
| Switch panel | `Tab` |
| Navigate up/down | `↑`/`↓` or `k`/`j` |
| Select table | `Enter` |
| Prev/next page | `←`/`→` or `h`/`l` |
| Sort by column | `1`–`9` |
| Enter query mode | `/` or `:` |
| Run query | `Enter` |
| Exit query mode | `Esc` or `Tab` |
| Toggle sidebar | `Ctrl+B` |
| Export CSV | `Ctrl+E` |

## License

[MIT](../../LICENSE)
