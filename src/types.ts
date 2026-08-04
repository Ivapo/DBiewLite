export interface DbInfo {
  path: string;
  file_size: number;
  engine: string;
  engine_version: string;
  page_count: number | null;
  page_size: number | null;
  table_count: number;
}

export interface TableInfo {
  name: string;
  row_count: number;
  column_count: number;
}

export interface ColumnInfo {
  name: string;
  col_type: string;
  nullable: boolean;
  primary_key: boolean;
  default_value: string | null;
}

export interface IndexInfo {
  name: string;
  table_name: string;
  unique: boolean;
  columns: string[];
}

export interface QueryResult {
  columns: string[];
  rows: CellValue[][];
  total_rows: number | null;
}

export type CellValue = null | number | string | number[];

export interface Sort {
  column: string;
  ascending: boolean;
}

export interface AppState {
  dbInfo: DbInfo | null;
  tables: TableInfo[];
  views: string[];
  selectedTable: string | null;
  schema: ColumnInfo[];
  data: QueryResult | null;
  page: number;
  pageSize: number;
  sort: Sort | null;
  queryInput: string;
  queryOpen: boolean;
  /// Explicit panel height once dragged. Null means the panel still sizes
  /// itself to its contents, which is the better default for a one-row result.
  queryHeight: number | null;
  queryResult: QueryResult | null;
  queryError: string | null;
  sidebarCollapsed: boolean;
  detailsOpen: boolean;
  /// Whether the column details show above the grid. Kept across tables, so it
  /// reads as a preference rather than something to re-open constantly.
  schemaOpen: boolean;
  helpOpen: boolean;
  /// Row the cursor is on, numbered across the whole table rather than the
  /// current page, so crossing a page edge is ordinary arithmetic.
  cursorRow: number;
  cursorCol: number;
}
