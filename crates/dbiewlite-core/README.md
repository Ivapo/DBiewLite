# dbiewlite-core

Shared database operations library for [DBiewLite](https://ivapo.github.io/DBiewLite/) — a friendly, read-only database viewer for SQLite, DuckDB, and Parquet files.

This crate provides the core database logic used by both the GUI and TUI versions of DBiewLite: opening databases, listing tables/views/indexes, querying data with pagination and sorting, schema inspection, and CSV export of either a table or a query's results. File format is detected automatically by extension (with magic-byte fallback for `.db` files).

Note that `export_csv` takes the sort being viewed, so a file comes out in the order it was read in, and writes NULL as an empty field.

## License

[MIT](../../LICENSE)
