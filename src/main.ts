import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { AppState, DbInfo, TableInfo, ColumnInfo, QueryResult, IndexInfo } from "./types";
import { initTheme, cycleTheme } from "./theme";

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

async function openDatabase(path: string): Promise<void> {
  try {
    state.dbInfo = await invoke<DbInfo>("open_database", { path });
    state.tables = await invoke<TableInfo[]>("list_tables");
    state.views = await invoke<string[]>("list_views");
    state.indexes = await invoke<IndexInfo[]>("list_indexes");
    state.selectedTable = null;
    state.data = null;
    state.schema = [];
    state.page = 0;
    state.sort = null;

    if (state.tables.length > 0) {
      await selectTable(state.tables[0]!.name);
    }

    render();
  } catch (e) {
    console.error("Failed to open database:", e);
  }
}

async function selectTable(name: string): Promise<void> {
  state.selectedTable = name;
  state.page = 0;
  state.sort = null;
  state.schema = await invoke<ColumnInfo[]>("get_schema", { table: name });
  await loadTableData();
}

async function loadTableData(): Promise<void> {
  if (!state.selectedTable) return;
  state.data = await invoke<QueryResult>("query_table", {
    table: state.selectedTable,
    limit: state.pageSize,
    offset: state.page * state.pageSize,
    sortColumn: state.sort?.column ?? null,
    sortAscending: state.sort?.ascending ?? null,
  });
}

async function toggleSort(column: string): Promise<void> {
  if (state.sort?.column === column) {
    state.sort = { column, ascending: !state.sort.ascending };
  } else {
    state.sort = { column, ascending: true };
  }
  state.page = 0;
  await loadTableData();
  render();
}

async function nextPage(): Promise<void> {
  if (!state.data?.total_rows) return;
  const maxPage = Math.floor((state.data.total_rows - 1) / state.pageSize);
  if (state.page < maxPage) {
    state.page++;
    await loadTableData();
    render();
  }
}

async function prevPage(): Promise<void> {
  if (state.page > 0) {
    state.page--;
    await loadTableData();
    render();
  }
}

async function runQuery(): Promise<void> {
  const sql = state.queryInput.trim();
  if (!sql) return;
  try {
    state.queryResult = await invoke<QueryResult>("run_query", { sql });
    state.queryError = null;
  } catch (e) {
    state.queryResult = null;
    state.queryError = String(e);
  }
  render();
}

async function exportCsv(): Promise<void> {
  if (!state.selectedTable) return;
  try {
    const outputPath = `${state.selectedTable}.csv`;
    await invoke("export_csv", { table: state.selectedTable, outputPath });
    showStatus(`Exported to ${outputPath}`);
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

function render(): void {
  const app = document.getElementById("app")!;

  if (!state.dbInfo) {
    app.innerHTML = `
      <div class="welcome">
        <h1>DBiewLite</h1>
        <p>Open a SQLite database to get started</p>
        <button id="open-btn" class="btn">Open Database</button>
      </div>
    `;
    document.getElementById("open-btn")?.addEventListener("click", handleOpenFile);
    return;
  }

  const info = state.dbInfo;
  const fileName = info.path.split("/").pop() ?? info.path;

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

  // Bind events
  document.getElementById("theme-btn")?.addEventListener("click", () => { cycleTheme(); });
  document.getElementById("open-new-btn")?.addEventListener("click", handleOpenFile);

  document.querySelectorAll(".sidebar-item[data-table]").forEach(el => {
    el.addEventListener("click", () => {
      const name = el.getAttribute("data-table");
      if (name) selectTable(name);
    });
  });

  document.querySelectorAll(".col-header[data-col]").forEach(el => {
    el.addEventListener("click", () => {
      const col = el.getAttribute("data-col");
      if (col) toggleSort(col);
    });
  });

  document.getElementById("prev-page")?.addEventListener("click", prevPage);
  document.getElementById("next-page")?.addEventListener("click", nextPage);
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

async function handleOpenFile(): Promise<void> {
  const path = await open({
    multiple: false,
    filters: [{ name: "SQLite", extensions: ["sqlite", "db", "sqlite3"] }],
  });
  if (path) {
    await openDatabase(path);
  }
}

function setupKeyboardShortcuts(): void {
  document.addEventListener("keydown", (e) => {
    // Cmd+O: open file
    if ((e.metaKey || e.ctrlKey) && e.key === "o") {
      e.preventDefault();
      handleOpenFile();
    }
    // Cmd+T: cycle theme
    if ((e.metaKey || e.ctrlKey) && e.key === "t") {
      e.preventDefault();
      cycleTheme();
    }
    // Cmd+B: toggle sidebar
    if ((e.metaKey || e.ctrlKey) && e.key === "b") {
      e.preventDefault();
      const sidebar = document.querySelector(".sidebar") as HTMLElement | null;
      if (sidebar) {
        sidebar.classList.toggle("collapsed");
      }
    }
    // Cmd+E: export
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
