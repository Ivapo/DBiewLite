# DBiewLite

A friendly, read-only SQLite database viewer for data analysis. Browse tables, inspect schemas, run SQL queries, and export to CSV — in a desktop GUI or your terminal.

Built with Tauri v2 + Rust + TypeScript.

## Features

- Open `.sqlite` / `.db` files and browse tables with row counts
- Paginated, sortable data grid
- Schema inspector showing column types
- SQL query panel with results grid
- Export tables to CSV
- Terminal UI (TUI) with the same feature set
- Four themes: dark, light, 3.1, tui

## Getting Started

### Prerequisites

- [Rust](https://rustup.rs/)
- [Bun](https://bun.sh/)

### GUI (Desktop App)

```bash
bun install
bun run tauri dev
```

### TUI (Terminal)

```bash
cargo run -p dbiewlite-tui -- path/to/database.sqlite
```

### Build for Production

```bash
# Desktop app
bun run tauri build

# TUI binary
cargo build -p dbiewlite-tui --release
```

## Tech Stack

- **Backend:** Rust (Tauri v2, rusqlite with bundled SQLite)
- **Frontend:** TypeScript + HTML/CSS (no framework)
- **TUI:** ratatui + crossterm

## License

[MIT](LICENSE)
