# dbiewlite-core

Shared database operations library for [DBiewLite](https://ivapo.github.io/DBiewLite/) — a friendly, read-only database viewer for SQLite, DuckDB, and Parquet files.

This crate provides the core database logic used by both the GUI and TUI versions of DBiewLite: opening databases, listing tables/views/indexes, querying data with pagination and sorting, schema inspection, and CSV export. File format is detected automatically by extension (with magic-byte fallback for `.db` files).

## License

[MIT](../../LICENSE)
