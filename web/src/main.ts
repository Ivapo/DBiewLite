import initSqlJs, { type Database as SqlJsDatabase } from "sql.js";
import type { AppState, DbInfo, TableInfo, ColumnInfo, QueryResult, IndexInfo } from "./types";
import { initTheme, cycleTheme } from "./theme";

let db: SqlJsDatabase | null = null;

const state: AppState = {
  dbInfo: null,
  tables: [],
  views: [],
  indexes: [],
  selectedTable: null,
  schema: [],
  data: null,
  page: 0,
  pageSize: 50,
  sort: null,
  queryInput: "",
  queryResult: null,
  queryError: null,
};

function formatSize(bytes: number): string {
  if (bytes < 1024) return `${bytes} B`;
  if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(1)} KB`;
  if (bytes < 1024 * 1024 * 1024) return `${(bytes / (1024 * 1024)).toFixed(1)} MB`;
  return `${(bytes / (1024 * 1024 * 1024)).toFixed(2)} GB`;
}

function escapeId(name: string): string {
  return `"${name.replace(/"/g, '""')}"`;
}

// --- sql.js database layer ---

function getDbInfo(fileName: string, fileSize: number): DbInfo {
  if (!db) throw new Error("No database open");
  const versionRow = db.exec("SELECT sqlite_version()");
  const sqliteVersion = String(versionRow[0]?.values[0]?.[0] ?? "unknown");
  const pageCountRow = db.exec("PRAGMA page_count");
  const pageCount = Number(pageCountRow[0]?.values[0]?.[0] ?? 0);
  const pageSizeRow = db.exec("PRAGMA page_size");
  const pageSize = Number(pageSizeRow[0]?.values[0]?.[0] ?? 0);
  const tableCountRow = db.exec("SELECT COUNT(*) FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%'");
  const tableCount = Number(tableCountRow[0]?.values[0]?.[0] ?? 0);

  return {
    path: fileName,
    file_size: fileSize,
    sqlite_version: sqliteVersion,
    page_count: pageCount,
    page_size: pageSize,
    table_count: tableCount,
  };
}

function listTables(): TableInfo[] {
  if (!db) return [];
  const result = db.exec("SELECT name FROM sqlite_master WHERE type='table' AND name NOT LIKE 'sqlite_%' ORDER BY name");
  if (!result[0]) return [];
  return result[0].values.map(row => {
    const name = String(row[0]);
    const countResult = db!.exec(`SELECT COUNT(*) FROM ${escapeId(name)}`);
    const rowCount = Number(countResult[0]?.values[0]?.[0] ?? 0);
    const colResult = db!.exec(`PRAGMA table_info(${escapeId(name)})`);
    const columnCount = colResult[0]?.values.length ?? 0;
    return { name, row_count: rowCount, column_count: columnCount };
  });
}

function listViews(): string[] {
  if (!db) return [];
  const result = db.exec("SELECT name FROM sqlite_master WHERE type='view' ORDER BY name");
  if (!result[0]) return [];
  return result[0].values.map(row => String(row[0]));
}

function listIndexes(): IndexInfo[] {
  if (!db) return [];
  const result = db.exec("SELECT name, tbl_name FROM sqlite_master WHERE type='index' AND name NOT LIKE 'sqlite_%' ORDER BY name");
  if (!result[0]) return [];
  return result[0].values.map(row => {
    const name = String(row[0]);
    const tableName = String(row[1]);
    const infoResult = db!.exec(`PRAGMA index_info(${escapeId(name)})`);
    const columns = infoResult[0]?.values.map(r => String(r[2])) ?? [];
    const listResult = db!.exec(`PRAGMA index_list(${escapeId(tableName)})`);
    const unique = listResult[0]?.values.find(r => String(r[1]) === name)?.[3] === 1;
    return { name, table_name: tableName, unique: !!unique, columns };
  });
}

function getSchema(table: string): ColumnInfo[] {
  if (!db) return [];
  const result = db.exec(`PRAGMA table_info(${escapeId(table)})`);
  if (!result[0]) return [];
  return result[0].values.map(row => ({
    name: String(row[1]),
    col_type: String(row[2] ?? ""),
    nullable: Number(row[3]) === 0,
    primary_key: Number(row[5]) > 0,
    default_value: row[4] != null ? String(row[4]) : null,
  }));
}

function queryTable(table: string, limit: number, offset: number, sortColumn: string | null, sortAscending: boolean | null): QueryResult {
  if (!db) throw new Error("No database open");

  const countResult = db.exec(`SELECT COUNT(*) FROM ${escapeId(table)}`);
  const totalRows = Number(countResult[0]?.values[0]?.[0] ?? 0);

  let sql = `SELECT * FROM ${escapeId(table)}`;
  if (sortColumn) {
    sql += ` ORDER BY ${escapeId(sortColumn)} ${sortAscending === false ? "DESC" : "ASC"}`;
  }
  sql += ` LIMIT ${limit} OFFSET ${offset}`;

  const result = db.exec(sql);
  if (!result[0]) return { columns: [], rows: [], total_rows: totalRows };

  const columns = result[0].columns;
  const rows = result[0].values.map(row =>
    row.map(val => {
      if (val === null) return null;
      if (val instanceof Uint8Array) return Array.from(val);
      return val as string | number;
    })
  );

  return { columns, rows, total_rows: totalRows };
}

function runSqlQuery(sql: string): QueryResult {
  if (!db) throw new Error("No database open");
  const result = db.exec(sql);
  if (!result[0]) return { columns: [], rows: [], total_rows: null };

  const columns = result[0].columns;
  const rows = result[0].values.map(row =>
    row.map(val => {
      if (val === null) return null;
      if (val instanceof Uint8Array) return Array.from(val);
      return val as string | number;
    })
  );

  return { columns, rows, total_rows: null };
}

// --- App logic (mirrors the Tauri GUI) ---

async function openDatabase(file: File): Promise<void> {
  try {
    const SQL = await initSqlJs({
      locateFile: () => `./sql-wasm.wasm`,
    });

    const buffer = await file.arrayBuffer();
    db = new SQL.Database(new Uint8Array(buffer));

    state.dbInfo = getDbInfo(file.name, file.size);
    state.tables = listTables();
    state.views = listViews();
    state.indexes = listIndexes();
    state.selectedTable = null;
    state.data = null;
    state.schema = [];
    state.page = 0;
    state.sort = null;
    state.queryResult = null;
    state.queryError = null;

    if (state.tables.length > 0) {
      selectTable(state.tables[0]!.name);
    }

    render();
  } catch (e) {
    console.error("Failed to open database:", e);
    alert(`Failed to open database: ${e}`);
  }
}

function selectTable(name: string): void {
  state.selectedTable = name;
  state.page = 0;
  state.sort = null;
  state.schema = getSchema(name);
  loadTableData();
}

function loadTableData(): void {
  if (!state.selectedTable) return;
  state.data = queryTable(
    state.selectedTable,
    state.pageSize,
    state.page * state.pageSize,
    state.sort?.column ?? null,
    state.sort?.ascending ?? null,
  );
}

function toggleSort(column: string): void {
  if (state.sort?.column === column) {
    state.sort = { column, ascending: !state.sort.ascending };
  } else {
    state.sort = { column, ascending: true };
  }
  state.page = 0;
  loadTableData();
  render();
}

function nextPage(): void {
  if (!state.data?.total_rows) return;
  const maxPage = Math.floor((state.data.total_rows - 1) / state.pageSize);
  if (state.page < maxPage) {
    state.page++;
    loadTableData();
    render();
  }
}

function prevPage(): void {
  if (state.page > 0) {
    state.page--;
    loadTableData();
    render();
  }
}

function runQuery(): void {
  const sql = state.queryInput.trim();
  if (!sql) return;
  try {
    state.queryResult = runSqlQuery(sql);
    state.queryError = null;
  } catch (e) {
    state.queryResult = null;
    state.queryError = String(e);
  }
  render();
}

function exportCsv(): void {
  if (!state.selectedTable || !db) return;
  try {
    const result = queryTable(state.selectedTable, 999999999, 0, null, null);
    const csvRows = [result.columns.join(",")];
    for (const row of result.rows) {
      csvRows.push(row.map(val => {
        if (val === null) return "";
        const s = String(val);
        return s.includes(",") || s.includes('"') || s.includes("\n")
          ? `"${s.replace(/"/g, '""')}"`
          : s;
      }).join(","));
    }
    const blob = new Blob([csvRows.join("\n")], { type: "text/csv" });
    const url = URL.createObjectURL(blob);
    const a = document.createElement("a");
    a.href = url;
    a.download = `${state.selectedTable}.csv`;
    a.click();
    URL.revokeObjectURL(url);
    showStatus(`Exported ${state.selectedTable}.csv`);
  } catch (e) {
    showStatus(`Export failed: ${e}`);
  }
}

function showStatus(msg: string): void {
  const toast = document.getElementById("status-toast");
  if (!toast) return;
  toast.textContent = msg;
  toast.classList.remove("hidden");
  setTimeout(() => toast.classList.add("hidden"), 3000);
}

function formatCellValue(val: unknown): string {
  if (val === null || val === undefined) return "NULL";
  if (Array.isArray(val)) return `<blob ${val.length} B>`;
  return String(val);
}

function cellClass(val: unknown): string {
  if (val === null || val === undefined) return "cell-null";
  if (typeof val === "number") return "cell-number";
  if (Array.isArray(val)) return "cell-blob";
  return "cell-text";
}

function handleOpenFile(): void {
  const input = document.createElement("input");
  input.type = "file";
  input.accept = ".sqlite,.db,.sqlite3";
  input.addEventListener("change", () => {
    const file = input.files?.[0];
    if (file) openDatabase(file);
  });
  input.click();
}

function render(): void {
  const app = document.getElementById("app")!;

  if (!state.dbInfo) {
    app.innerHTML = `
      <div class="welcome" id="drop-zone">
        <h1>DBiewLite</h1>
        <p>Drop a SQLite database here, or click to open</p>
        <button id="open-btn" class="btn">Open Database</button>
        <p class="welcome-hint">Supports .sqlite, .db, .sqlite3 files</p>
      </div>
    `;
    document.getElementById("open-btn")?.addEventListener("click", handleOpenFile);
    setupDropZone();
    return;
  }

  const info = state.dbInfo;
  const fileName = info.path;

  app.innerHTML = `
    <div class="layout">
      <div class="title-bar">
        <span class="title-text">DBiewLite</span>
        <span class="title-info">SQLite ${info.sqlite_version} \u2502 ${info.table_count} tables \u2502 <span class="title-filename">${fileName}</span> (${formatSize(info.file_size)})</span>
        <div class="title-actions">
          <button id="theme-btn" class="btn btn-sm">Theme</button>
          <button id="open-new-btn" class="btn btn-sm">Open</button>
        </div>
      </div>
      <div id="status-toast" class="status-toast hidden"></div>
      <div class="main-area">
        <div class="sidebar">
          <div class="sidebar-section">
            <div class="sidebar-header">Tables</div>
            ${state.tables.map(t => `
              <div class="sidebar-item ${t.name === state.selectedTable ? "active" : ""}" data-table="${t.name}">
                <span class="sidebar-icon">\u{f0ce}</span>
                <span class="sidebar-name">${t.name}</span>
                <span class="sidebar-count">${t.row_count}</span>
              </div>
            `).join("")}
          </div>
          ${state.views.length > 0 ? `
            <div class="sidebar-section">
              <div class="sidebar-header">Views</div>
              ${state.views.map(v => `
                <div class="sidebar-item sidebar-view">
                  <span class="sidebar-icon">\u{f06e}</span>
                  <span class="sidebar-name">${v}</span>
                </div>
              `).join("")}
            </div>
          ` : ""}
          ${state.indexes.length > 0 ? `
            <div class="sidebar-section">
              <div class="sidebar-header">Indexes</div>
              ${state.indexes.map(idx => `
                <div class="sidebar-item sidebar-index">
                  <span class="sidebar-icon">\u{f0cb}</span>
                  <span class="sidebar-name">${idx.name}</span>
                </div>
              `).join("")}
            </div>
          ` : ""}
        </div>
        <div class="content">
          ${renderDataTable()}
          ${renderQueryPanel()}
        </div>
      </div>
    </div>
  `;

  document.getElementById("theme-btn")?.addEventListener("click", () => { cycleTheme(); });
  document.getElementById("open-new-btn")?.addEventListener("click", handleOpenFile);

  document.querySelectorAll(".sidebar-item[data-table]").forEach(el => {
    el.addEventListener("click", () => {
      const name = el.getAttribute("data-table");
      if (name) { selectTable(name); render(); }
    });
  });

  document.querySelectorAll(".col-header[data-col]").forEach(el => {
    el.addEventListener("click", () => {
      const col = el.getAttribute("data-col");
      if (col) toggleSort(col);
    });
  });

  document.getElementById("prev-page")?.addEventListener("click", () => { prevPage(); });
  document.getElementById("next-page")?.addEventListener("click", () => { nextPage(); });
  document.getElementById("export-btn")?.addEventListener("click", exportCsv);

  const queryInput = document.getElementById("query-input") as HTMLTextAreaElement | null;
  if (queryInput) {
    queryInput.value = state.queryInput;
    queryInput.addEventListener("input", (e) => {
      state.queryInput = (e.target as HTMLTextAreaElement).value;
    });
    queryInput.addEventListener("keydown", (e) => {
      if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
        e.preventDefault();
        runQuery();
      }
    });
  }

  document.getElementById("run-query-btn")?.addEventListener("click", runQuery);
}

function renderDataTable(): string {
  if (!state.data || !state.selectedTable) {
    return `<div class="data-panel empty"><p>Select a table from the sidebar</p></div>`;
  }

  const total = state.data.total_rows ?? 0;
  const start = state.page * state.pageSize + 1;
  const end = Math.min(start + state.data.rows.length - 1, total);

  const headers = state.data.columns.map(col => {
    const indicator = state.sort?.column === col
      ? (state.sort.ascending ? " \u25b2" : " \u25bc")
      : "";
    return `<th class="col-header" data-col="${col}">${col}${indicator}</th>`;
  }).join("");

  const rows = state.data.rows.map(row => {
    const cells = row.map(val =>
      `<td class="${cellClass(val)}">${formatCellValue(val)}</td>`
    ).join("");
    return `<tr>${cells}</tr>`;
  }).join("");

  const schemaInfo = state.schema.map(c => {
    const pk = c.primary_key ? " PK" : "";
    const nullable = c.nullable ? "" : " NOT NULL";
    return `<span class="schema-chip">${c.name}: ${c.col_type || "ANY"}${pk}${nullable}</span>`;
  }).join("");

  return `
    <div class="data-panel">
      <div class="data-header">
        <span class="data-title">${state.selectedTable}</span>
        <span class="data-info">${start}\u2013${end} of ${total}</span>
        <div class="data-actions">
          <button id="prev-page" class="btn btn-sm" ${state.page === 0 ? "disabled" : ""}>\u25c0</button>
          <button id="next-page" class="btn btn-sm" ${end >= total ? "disabled" : ""}>\u25b6</button>
          <button id="export-btn" class="btn btn-sm">Export .csv</button>
        </div>
      </div>
      <div class="schema-bar">${schemaInfo}</div>
      <div class="table-wrapper">
        <table class="data-table">
          <thead><tr>${headers}</tr></thead>
          <tbody>${rows}</tbody>
        </table>
      </div>
    </div>
  `;
}

function renderQueryPanel(): string {
  let resultHtml = "";
  if (state.queryError) {
    resultHtml = `<div class="query-error">${state.queryError}</div>`;
  } else if (state.queryResult) {
    const qHeaders = state.queryResult.columns.map(c => `<th>${c}</th>`).join("");
    const qRows = state.queryResult.rows.map(row => {
      const cells = row.map(val => `<td class="${cellClass(val)}">${formatCellValue(val)}</td>`).join("");
      return `<tr>${cells}</tr>`;
    }).join("");
    resultHtml = `
      <div class="query-result-info">${state.queryResult.rows.length} rows returned</div>
      <div class="table-wrapper">
        <table class="data-table">
          <thead><tr>${qHeaders}</tr></thead>
          <tbody>${qRows}</tbody>
        </table>
      </div>
    `;
  }

  return `
    <div class="query-panel">
      <div class="query-input-area">
        <textarea id="query-input" placeholder="Enter SQL query... (Cmd+Enter to run)" rows="2"></textarea>
        <button id="run-query-btn" class="btn">Run</button>
      </div>
      <div class="query-results">${resultHtml}</div>
    </div>
  `;
}

function setupDropZone(): void {
  const zone = document.getElementById("drop-zone");
  if (!zone) return;

  zone.addEventListener("dragover", (e) => {
    e.preventDefault();
    zone.classList.add("drag-over");
  });
  zone.addEventListener("dragleave", () => {
    zone.classList.remove("drag-over");
  });
  zone.addEventListener("drop", (e) => {
    e.preventDefault();
    zone.classList.remove("drag-over");
    const file = e.dataTransfer?.files[0];
    if (file) openDatabase(file);
  });
}

function setupKeyboardShortcuts(): void {
  document.addEventListener("keydown", (e) => {
    if ((e.metaKey || e.ctrlKey) && e.key === "o") {
      e.preventDefault();
      handleOpenFile();
    }
    if ((e.metaKey || e.ctrlKey) && e.key === "t") {
      e.preventDefault();
      cycleTheme();
    }
    if ((e.metaKey || e.ctrlKey) && e.key === "b") {
      e.preventDefault();
      const sidebar = document.querySelector(".sidebar") as HTMLElement | null;
      if (sidebar) sidebar.classList.toggle("collapsed");
    }
    if ((e.metaKey || e.ctrlKey) && e.key === "e") {
      e.preventDefault();
      exportCsv();
    }
  });
}

function init(): void {
  initTheme();
  setupKeyboardShortcuts();
  render();
}

init();
