import { invoke } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import type { AppState, DbInfo, TableInfo, ColumnInfo, QueryResult } from "./types";
import { initTheme, cycleTheme } from "./theme";

const state: AppState = {
  dbInfo: null,
  tables: [],
  views: [],
  selectedTable: null,
  schema: [],
  data: null,
  page: 0,
  pageSize: 50,
  sort: null,
  queryInput: "",
  queryResult: null,
  queryError: null,
  detailsOpen: false,
};

function escapeHtml(s: string): string {
  return s.replace(/&/g, "&amp;").replace(/</g, "&lt;").replace(/>/g, "&gt;");
}

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
  pendingScroll = "top";
  render();
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
  pendingScroll = "top";
  render();
}

/// Returns whether the page actually advanced.
async function nextPage(): Promise<boolean> {
  if (!state.data?.total_rows) return false;
  const maxPage = Math.floor((state.data.total_rows - 1) / state.pageSize);
  if (state.page >= maxPage) return false;
  state.page++;
  await loadTableData();
  render();
  return true;
}

/// Returns whether the page actually moved back.
async function prevPage(): Promise<boolean> {
  if (state.page === 0) return false;
  state.page--;
  await loadTableData();
  render();
  return true;
}

// --- scroll-to-turn ----------------------------------------------------
//
// The grid scrolls within a page and rolls over at the edges, the same model
// the TUI uses. render() rebuilds the table wholesale, so the landing position
// is stashed here and applied once the new DOM exists.

/// How close to an edge counts as reaching it.
const EDGE_SLACK_PX = 24;
/// Fallback row height, for when the page has no rows to measure.
const FALLBACK_ROW_PX = 29;

let pageTurning = false;
let pendingScroll: "top" | "bottom" | null = null;
/// The edge already acted on. Turning a page lands against the opposite edge,
/// which would otherwise read as a fresh arrival and turn again immediately —
/// so a turn only fires when the grid *enters* an edge it was not already at.
let settledEdge: "top" | "bottom" | null = null;

function tableWrapper(): HTMLElement | null {
  return document.querySelector(".table-wrapper");
}

function applyPendingScroll(carried: number): void {
  const wrapper = tableWrapper();
  if (!wrapper) {
    pendingScroll = null;
    return;
  }
  // Renders that are not page turns — toggling the details strip, running a
  // query — should leave the reader where they were.
  wrapper.scrollTop = pendingScroll === "bottom"
    ? Math.max(0, wrapper.scrollHeight - wrapper.clientHeight)
    : pendingScroll === "top" ? 0 : carried;
  pendingScroll = null;
  // Whatever edge the rebuilt grid is resting against counts as already
  // reached, so the first scroll event afterwards is not read as an arrival.
  const atTop = wrapper.scrollTop <= EDGE_SLACK_PX;
  const atBottom = wrapper.scrollTop + wrapper.clientHeight >= wrapper.scrollHeight - EDGE_SLACK_PX;
  settledEdge = atBottom ? "bottom" : atTop ? "top" : null;
}

async function turnPage(direction: 1 | -1): Promise<void> {
  if (pageTurning) return;
  pageTurning = true;
  // Set before the fetch: nextPage/prevPage render as they return, and the
  // render is what applies it.
  pendingScroll = direction === 1 ? "top" : "bottom";
  const moved = direction === 1 ? await nextPage() : await prevPage();
  if (!moved) pendingScroll = null;
  pageTurning = false;
}

function onTableScroll(wrapper: HTMLElement): void {
  if (pageTurning) return;
  // A page too short to scroll has no edges to arrive at; without this the
  // grid would turn straight through every remaining page.
  if (wrapper.scrollHeight <= wrapper.clientHeight) return;

  const atTop = wrapper.scrollTop <= EDGE_SLACK_PX;
  const atBottom = wrapper.scrollTop + wrapper.clientHeight >= wrapper.scrollHeight - EDGE_SLACK_PX;
  const edge = atBottom ? "bottom" : atTop ? "top" : null;

  if (edge === settledEdge) return;
  settledEdge = edge;
  if (edge === "bottom") void turnPage(1);
  else if (edge === "top") void turnPage(-1);
}

/// Scrolls the grid by whole rows, rolling over the page edge like the TUI's
/// cursor does.
function scrollByRows(rows: number): void {
  const wrapper = tableWrapper();
  if (!wrapper) return;
  const firstRow = wrapper.querySelector("tbody tr") as HTMLElement | null;
  const rowHeight = firstRow?.offsetHeight || FALLBACK_ROW_PX;
  const before = wrapper.scrollTop;
  wrapper.scrollTop += rows * rowHeight;
  // Already pinned against the edge, so no scroll event will fire to notice it.
  if (wrapper.scrollTop === before) onTableScroll(wrapper);
}

/// Jumps to the first or last row of the whole table.
async function goToEdgeRow(which: "first" | "last"): Promise<void> {
  if (!state.data?.total_rows) return;
  const maxPage = Math.floor((state.data.total_rows - 1) / state.pageSize);
  const target = which === "first" ? 0 : maxPage;
  pendingScroll = which === "first" ? "top" : "bottom";
  if (state.page !== target) {
    state.page = target;
    await loadTableData();
  }
  render();
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

function renderDbDetails(info: DbInfo): string {
  const rows: [string, string][] = [
    ["Path", escapeHtml(info.path)],
    ["Engine", `${info.engine} ${info.engine_version}`],
    ["Size", formatSize(info.file_size)],
    ["Tables", `${info.table_count}${state.views.length > 0 ? ` │ ${state.views.length} views` : ""}`],
  ];
  if (info.page_count !== null) rows.push(["Pages", String(info.page_count)]);
  if (info.page_size !== null) rows.push(["Page size", `${info.page_size} B`]);

  return `
    <div id="db-details" class="db-details">
      <dl class="db-details-grid">
        ${rows.map(([label, value]) => `
          <dt>${label}</dt>
          <dd>${value}</dd>
        `).join("")}
      </dl>
      <div class="db-details-hint">⌘O open another database</div>
    </div>
  `;
}

function render(): void {
  const app = document.getElementById("app")!;
  const carriedScroll = tableWrapper()?.scrollTop ?? 0;

  if (!state.dbInfo) {
    app.innerHTML = `
      <div class="welcome">
        <h1>DBiewLite</h1>
        <p>Open a database file to get started</p>
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
        <button id="db-details-toggle" class="db-title" aria-expanded="${state.detailsOpen}" aria-controls="db-details">
          <span class="db-title-name">${escapeHtml(fileName)}</span>
          <span class="db-chevron ${state.detailsOpen ? "open" : ""}">\u203a</span>
        </button>
        <div class="title-actions">
          <button id="theme-btn" class="btn btn-sm">Theme</button>
        </div>
      </div>
      ${state.detailsOpen ? renderDbDetails(info) : ""}
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
                <div class="sidebar-item sidebar-view" data-table="${v}">
                  <span class="sidebar-icon">\u{f06e}</span>
                  <span class="sidebar-name">${v}</span>
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
  document.getElementById("db-details-toggle")?.addEventListener("click", () => {
    state.detailsOpen = !state.detailsOpen;
    render();
  });

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

  document.getElementById("prev-page")?.addEventListener("click", () => { void turnPage(-1); });
  document.getElementById("next-page")?.addEventListener("click", () => { void turnPage(1); });
  document.getElementById("export-btn")?.addEventListener("click", exportCsv);

  const wrapper = tableWrapper();
  if (wrapper) {
    wrapper.addEventListener("scroll", () => { onTableScroll(wrapper); });
  }
  applyPendingScroll(carriedScroll);

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
    filters: [
      { name: "All Databases", extensions: ["sqlite", "db", "sqlite3", "duckdb", "parquet", "pq"] },
      { name: "SQLite", extensions: ["sqlite", "db", "sqlite3"] },
      { name: "DuckDB", extensions: ["duckdb"] },
      { name: "Parquet", extensions: ["parquet", "pq"] },
    ],
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

    // Everything below is unmodified keys acting on the grid, so they only
    // apply when the query box does not have the keystroke.
    if (e.metaKey || e.ctrlKey || e.altKey || e.isComposing) return;
    if (isTypingTarget(document.activeElement)) return;
    if (!state.dbInfo) return;

    switch (e.key) {
      case "ArrowDown":
      case "j":
        e.preventDefault();
        scrollByRows(1);
        break;
      case "ArrowUp":
      case "k":
        e.preventDefault();
        scrollByRows(-1);
        break;
      case "PageDown":
      case "]":
        e.preventDefault();
        void turnPage(1);
        break;
      case "PageUp":
      case "[":
        e.preventDefault();
        void turnPage(-1);
        break;
      case "g":
        e.preventDefault();
        void goToEdgeRow("first");
        break;
      case "G":
        e.preventDefault();
        void goToEdgeRow("last");
        break;
      case "/":
      case ":":
        e.preventDefault();
        document.getElementById("query-input")?.focus();
        break;
      case "i":
        e.preventDefault();
        state.detailsOpen = !state.detailsOpen;
        render();
        break;
    }
  });
}

function isTypingTarget(el: Element | null): boolean {
  return el instanceof HTMLTextAreaElement || el instanceof HTMLInputElement;
}

function init(): void {
  initTheme();
  setupKeyboardShortcuts();
  render();
}

init();
