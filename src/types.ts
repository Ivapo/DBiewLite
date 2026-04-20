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
  queryResult: QueryResult | null;
  queryError: string | null;
}
