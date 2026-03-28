# DBiewLite

A friendly SQLite database viewer for data analysis, built with Tauri v2 + Rust + TypeScript.

## What This Is

A desktop + terminal app that opens `.sqlite`/`.db` files and lets you visually browse tables, inspect schemas, run SQL queries, and export data to CSV. Read-only.

## Tech Stack

- **Backend:** Rust (Tauri v2, `rusqlite` with bundled SQLite)
- **Frontend:** TypeScript + HTML/CSS (no framework)
- **TUI:** `ratatui` + `crossterm` (terminal UI, same pattern as PanEx)
- **Package managers:** Cargo (Rust), Bun (frontend)
- **Target platforms:** macOS first, then Windows and Linux

## Architecture

- `dbiewlite-core`: Shared Rust library for all SQLite operations (used by both GUI and TUI)
- `src-tauri`: Tauri v2 app — thin command wrappers around core
- `src/`: Frontend — vanilla TS, renders sidebar + data grid + query panel
- `dbiewlite-tui`: Terminal UI with ratatui — same features as GUI
- Themes shared with PanEx (dark, light, 3.1, tui) — see `~/.claude/themes/panex-themes.md`

## Project Structure

```
DBiewLite/
├── Cargo.toml              # Workspace root
├── crates/
│   ├── dbiewlite-core/     # Shared SQLite logic
│   │   └── src/lib.rs      # Database, TableInfo, QueryResult, export_csv, etc.
│   └── dbiewlite-tui/      # Terminal UI
│       └── src/
│           ├── main.rs      # Terminal setup, event loop
│           ├── app.rs       # App state, table loading, pagination, sort
│           ├── ui.rs        # Ratatui rendering
│           └── input.rs     # Key bindings
├── src-tauri/               # Tauri v2 GUI backend
│   └── src/
│       ├── main.rs          # Entry point
│       ├── lib.rs           # Tauri builder, command registration
│       └── commands.rs      # Tauri commands (wraps core)
├── src/                     # GUI Frontend
│   ├── index.html
│   ├── style.css            # All 4 themes (dark/light/3.1/tui)
│   ├── main.ts              # State management, rendering, events
│   ├── types.ts             # TypeScript interfaces mirroring Rust structs
│   └── theme.ts             # Theme cycling and persistence
├── package.json
└── vite.config.ts
```

## MVP Features (build order)

1. ~~Open a database file~~ (done — CLI arg for TUI, prompt for GUI)
2. ~~Sidebar: table list~~ (done — with row counts)
3. ~~Table data grid~~ (done — paginated, sortable columns)
4. ~~Schema inspector~~ (done — schema bar showing column types)
5. ~~SQL query panel~~ (done — textarea + results grid)
6. ~~Export to CSV~~ (done — per-table export)
7. Copy to clipboard (select rows/cells, Cmd+C as TSV)
8. File dialog for opening databases (replace prompt with native dialog)

## Conventions

- Use `snake_case` for Rust, `camelCase` for TypeScript
- All SQLite access through Rust via `dbiewlite-core` — never from TS directly
- Tauri commands are thin wrappers around core functions
- Type all `invoke` responses — interfaces in `types.ts` mirror Rust structs
- Vanilla TS — no React/Vue/Svelte
- CSS custom properties for theming (shared palette with PanEx)
- `strict: true` in tsconfig.json

## Useful Commands

```bash
# Install frontend dependencies
bun install

# Dev mode with hot reload (GUI)
bun run tauri dev

# Build for production (GUI)
bun run tauri build

# Run TUI
cargo run -p dbiewlite-tui -- path/to/database.sqlite

# Build TUI binary
cargo build -p dbiewlite-tui --release
```
