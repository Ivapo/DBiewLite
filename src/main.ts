import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { open, save } from "@tauri-apps/plugin-dialog";
import type { AppState, DbInfo, TableInfo, ColumnInfo, QueryResult } from "./types";
import { initTheme, cycleTheme } from "./theme";

// --- query panel sizing ------------------------------------------------
//
// Declared ahead of `state`, which reads the stored height while building
// itself: a const below that point is still in its dead zone by then, and
// touching it throws before a single listener is bound.

/// Enough to keep the input and a line of output usable.
const QUERY_PANEL_MIN_PX = 110;
/// Rows the grid keeps no matter how far the divider is dragged.
const GRID_MIN_PX = 120;
const QUERY_HEIGHT_KEY = "dbiewlite_query_height";

function loadQueryHeight(): number | null {
  const stored = Number(localStorage.getItem(QUERY_HEIGHT_KEY));
  return Number.isFinite(stored) && stored > 0 ? stored : null;
}

function saveQueryHeight(): void {
  if (state.queryHeight === null) localStorage.removeItem(QUERY_HEIGHT_KEY);
  else localStorage.setItem(QUERY_HEIGHT_KEY, String(Math.round(state.queryHeight)));
}

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
  queryOpen: false,
  queryHeight: loadQueryHeight(),
  queryResult: null,
  queryError: null,
  detailsOpen: false,
  schemaOpen: false,
  helpOpen: false,
  cursorRow: 0,
  cursorCol: 0,
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
  state.cursorRow = 0;
  state.cursorCol = 0;
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
  // Row order changed underneath it, so the old row number means nothing now.
  state.cursorRow = 0;
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
  // Scoped to the data panel: the query results render a .table-wrapper too,
  // and with no table selected that one would otherwise be picked up here and
  // wired for cursor movement and page turning.
  return document.querySelector(".data-panel .table-wrapper");
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
  if (moved) {
    // The cursor is numbered across the whole table, so a page turn it did not
    // drive would strand it off-screen. Bring it to the edge being read into.
    state.cursorRow = direction === 1
      ? state.page * state.pageSize
      : state.page * state.pageSize + state.pageSize - 1;
    paintCursor();
  } else {
    pendingScroll = null;
  }
  pageTurning = false;
}

function onTableScroll(wrapper: HTMLElement): void {
  if (pageTurning) return;
  if (performance.now() - scrolledProgrammaticallyAt < SCROLL_SETTLE_MS) return;
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

// --- cell cursor -------------------------------------------------------

/// Scrolling the cursor into view can land against an edge, which would
/// otherwise read as the user arriving there and turn a page on its own. Only
/// scrolling the user drives should turn pages.
///
/// Recorded as a time rather than a flag cleared on the next frame: a window
/// that is hidden or occluded runs no frames, and a flag left standing there
/// would disable page turning for the rest of the session.
const SCROLL_SETTLE_MS = 150;
let scrolledProgrammaticallyAt = -Infinity;

function scrollProgrammatically(wrapper: HTMLElement, top: number, left: number): void {
  scrolledProgrammaticallyAt = performance.now();
  wrapper.scrollTop = top;
  wrapper.scrollLeft = left;
}

/// Rows that fit below the sticky header — the step Ctrl+D/Ctrl+U move by.
function visibleRowCount(): number {
  const wrapper = tableWrapper();
  const row = wrapper?.querySelector("tbody tr") as HTMLElement | null;
  if (!wrapper || !row) return 1;
  const headHeight = (wrapper.querySelector("thead") as HTMLElement | null)?.offsetHeight ?? 0;
  return Math.max(1, Math.floor((wrapper.clientHeight - headHeight) / (row.offsetHeight || FALLBACK_ROW_PX)));
}

function scrollCursorIntoView(wrapper: HTMLElement, row: HTMLElement, cell: HTMLElement | null): void {
  const wrapRect = wrapper.getBoundingClientRect();
  // The header floats over the top of the scroll area, so a row is only
  // really visible once it clears it.
  const headHeight = (wrapper.querySelector("thead") as HTMLElement | null)?.offsetHeight ?? 0;
  const rowRect = row.getBoundingClientRect();

  let dy = 0;
  if (rowRect.top < wrapRect.top + headHeight) dy = rowRect.top - (wrapRect.top + headHeight);
  else if (rowRect.bottom > wrapRect.bottom) dy = rowRect.bottom - wrapRect.bottom;

  let dx = 0;
  if (cell) {
    const cellRect = cell.getBoundingClientRect();
    if (cellRect.left < wrapRect.left) dx = cellRect.left - wrapRect.left;
    else if (cellRect.right > wrapRect.right) dx = cellRect.right - wrapRect.right;
  }

  // Clamp before comparing: the first row sits under the sticky header, so it
  // asks to scroll above zero every time. Stamping the suppression window for
  // a scroll that cannot move would deafen the grid to real ones.
  const top = clamp(wrapper.scrollTop + dy, 0, Math.max(0, wrapper.scrollHeight - wrapper.clientHeight));
  const left = clamp(wrapper.scrollLeft + dx, 0, Math.max(0, wrapper.scrollWidth - wrapper.clientWidth));
  if (top === wrapper.scrollTop && left === wrapper.scrollLeft) return;
  scrollProgrammatically(wrapper, top, left);
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(Math.max(value, min), max);
}

/// Moves the cursor highlight without rebuilding the grid — a full render per
/// keystroke would rebind every listener and fight the scroll position.
function paintCursor(): void {
  const wrapper = tableWrapper();
  if (!wrapper) return;
  wrapper.querySelector(".row-cursor")?.classList.remove("row-cursor");
  wrapper.querySelector(".cell-cursor")?.classList.remove("cell-cursor");

  const pageRow = state.cursorRow - state.page * state.pageSize;
  const row = wrapper.querySelectorAll("tbody tr")[pageRow] as HTMLElement | undefined;
  if (!row) return;
  row.classList.add("row-cursor");
  const cell = row.children[state.cursorCol] as HTMLElement | undefined;
  cell?.classList.add("cell-cursor");
  scrollCursorIntoView(wrapper, row, cell ?? null);
}

async function setCursorRow(target: number): Promise<void> {
  state.cursorRow = target;
  const page = Math.floor(target / state.pageSize);
  if (page === state.page) {
    paintCursor();
    return;
  }
  state.page = page;
  await loadTableData();
  render();
}

async function moveCursorRow(delta: number): Promise<void> {
  const total = state.data?.total_rows ?? 0;
  if (total === 0) return;
  const target = Math.min(Math.max(state.cursorRow + delta, 0), total - 1);
  if (target !== state.cursorRow) await setCursorRow(target);
}

function moveCursorCol(delta: number): void {
  const columns = state.data?.columns.length ?? 0;
  if (columns === 0) return;
  state.cursorCol = Math.min(Math.max(state.cursorCol + delta, 0), columns - 1);
  paintCursor();
}

/// Pans the grid sideways a whole column at a time, the way Shift+arrows do in
/// the TUI, rather than by some arbitrary pixel step.
function panColumns(direction: -1 | 1): void {
  const wrapper = tableWrapper();
  if (!wrapper) return;
  const headers = Array.from(wrapper.querySelectorAll("thead th")) as HTMLElement[];
  if (headers.length === 0) return;

  const wrapLeft = wrapper.getBoundingClientRect().left;
  const edges = headers.map(th => th.getBoundingClientRect().left - wrapLeft + wrapper.scrollLeft);
  const current = wrapper.scrollLeft;
  const next = direction === 1
    ? edges.find(edge => edge > current + 1)
    : [...edges].reverse().find(edge => edge < current - 1);

  const fallback = direction === 1 ? wrapper.scrollWidth - wrapper.clientWidth : 0;
  scrollProgrammatically(wrapper, wrapper.scrollTop, next ?? fallback);
}

function sortCursorColumn(): void {
  const column = state.data?.columns[state.cursorCol];
  if (column) void toggleSort(column);
}

/// Jumps to the first or last row of the whole table.
async function goToEdgeRow(which: "first" | "last"): Promise<void> {
  const total = state.data?.total_rows ?? 0;
  if (total === 0) return;
  const targetRow = which === "first" ? 0 : total - 1;
  const targetPage = Math.floor(targetRow / state.pageSize);
  state.cursorRow = targetRow;
  pendingScroll = which === "first" ? "top" : "bottom";
  if (state.page !== targetPage) {
    state.page = targetPage;
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
  // Ask where it goes. A relative path would resolve against the working
  // directory, which is src-tauri under `tauri dev` and / for a bundled app —
  // where the write fails outright.
  const outputPath = await save({
    defaultPath: `${state.selectedTable}.csv`,
    filters: [{ name: "CSV", extensions: ["csv"] }],
  });
  if (!outputPath) return;
  try {
    // Sorted as the grid is. The page window is not applied: the whole table
    // goes out, not the fifty rows currently on screen.
    await invoke("export_csv", {
      table: state.selectedTable,
      outputPath,
      sortColumn: state.sort?.column ?? null,
      sortAscending: state.sort?.ascending ?? null,
    });
    showStatus(`Exported to ${outputPath}`);
  } catch (e) {
    showStatus(`Export failed: ${e}`);
  }
}

async function exportQueryCsv(): Promise<void> {
  const sql = state.queryInput.trim();
  if (!sql || !state.queryResult) return;
  const outputPath = await save({
    defaultPath: "query-results.csv",
    filters: [{ name: "CSV", extensions: ["csv"] }],
  });
  if (!outputPath) return;
  try {
    await invoke("export_query_csv", { sql, outputPath });
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

/// Plain text for a cell. The angle-bracket markers are stand-ins for values
/// with nothing to show, and are escaped along with everything else on the way
/// into the DOM — unescaped, `<blob 12 B>` is parsed as a start tag and the
/// cell draws empty.
function formatCellValue(val: unknown): string {
  if (val === null || val === undefined) return "NULL";
  if (Array.isArray(val)) return `<blob ${val.length} B>`;
  // Otherwise indistinguishable from a NULL, or from a cell that failed to draw.
  if (val === "") return "<empty>";
  return String(val);
}

function cellClass(val: unknown): string {
  if (val === null || val === undefined) return "cell-null";
  if (typeof val === "number") return "cell-number";
  if (Array.isArray(val)) return "cell-blob";
  if (val === "") return "cell-empty";
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

/// Every binding the app answers to, in one place so the panel cannot drift
/// from what setupKeyboardShortcuts actually does.
const SHORTCUTS: { title: string; keys: [string, string][] }[] = [
  {
    title: "Move",
    keys: [
      ["j  ↓", "Down a row"],
      ["k  ↑", "Up a row"],
      ["h  ←", "Left a column"],
      ["l  →", "Right a column"],
      ["⌃D  ⌃U", "Half a screen"],
      ["g  G", "First / last row"],
      ["Home  End", "First / last column"],
    ],
  },
  {
    title: "Pages",
    keys: [
      ["]  ⇟", "Next 50 rows"],
      ["[  ⇞", "Previous 50 rows"],
      ["scroll", "Reaching an edge turns the page"],
    ],
  },
  {
    title: "Grid",
    keys: [
      ["s", "Sort by the cursor's column"],
      ["H  L", "Pan sideways a column"],
      ["⇧←  ⇧→", "Pan sideways a column"],
      ["click", "Put the cursor on a cell"],
    ],
  },
  {
    title: "App",
    keys: [
      ["⇥", "Focus the table list"],
      ["⏎", "Leave the list for the grid"],
      ["/  :", "Open the SQL panel"],
      ["⏎", "Run the query"],
      ["⇧⏎", "New line in the query"],
      ["esc", "Close the SQL panel"],
      ["i", "Database details"],
      ["c", "Column details"],
      ["⌘T", "Next theme"],
      ["⌘B", "Show or hide the sidebar"],
      ["⌘O", "Open a database"],
      ["⌘E", "Export CSV"],
      ["?", "This panel"],
    ],
  },
];

function renderHelpOverlay(): string {
  const sections = SHORTCUTS.map(section => `
    <section class="help-section">
      <h3>${section.title}</h3>
      <dl>
        ${section.keys.map(([key, description]) => `
          <dt><kbd>${escapeHtml(key)}</kbd></dt>
          <dd>${description}</dd>
        `).join("")}
      </dl>
    </section>
  `).join("");

  return `
    <div id="help-overlay" class="help-overlay">
      <div class="help-panel" role="dialog" aria-label="Keyboard shortcuts">
        <div class="help-header">
          <span class="help-title">Keyboard shortcuts</span>
          <span class="help-dismiss">esc to close</span>
        </div>
        <div class="help-body">${sections}</div>
      </div>
    </div>
  `;
}

function sidebarItems(): HTMLElement[] {
  return Array.from(document.querySelectorAll(".sidebar-item[data-table]"));
}

/// How long the sidebar sits still before its contents load. Walking the list
/// with a key held down should cost one query at the end, not one per table.
const SIDEBAR_PREVIEW_MS = 120;
let sidebarPreviewTimer: number | undefined;

/// Loads whatever the sidebar has settled on, the way the TUI drains its
/// pending load once an input burst ends.
function previewSidebarSelection(name: string): void {
  window.clearTimeout(sidebarPreviewTimer);
  sidebarPreviewTimer = window.setTimeout(() => {
    // Already on screen — re-querying would only throw away the scroll.
    if (name !== state.selectedTable) void selectTable(name);
  }, SIDEBAR_PREVIEW_MS);
}

function commitSidebarSelection(): void {
  window.clearTimeout(sidebarPreviewTimer);
  const active = document.activeElement as HTMLElement | null;
  const name = active?.getAttribute("data-table");
  if (name && name !== state.selectedTable) void selectTable(name);
  // Contents are already showing, so Enter only hands the keys to the grid.
  active?.blur();
}

function moveSidebarFocus(delta: number): void {
  const items = sidebarItems();
  if (items.length === 0) return;
  const current = items.indexOf(document.activeElement as HTMLElement);
  const target = items[current === -1 ? 0 : clamp(current + delta, 0, items.length - 1)];
  if (!target) return;
  // Carry the single tab stop to wherever the cursor now is, so tabbing away
  // and back returns here rather than to whichever table is loaded.
  items.forEach(item => { item.tabIndex = -1; });
  target.tabIndex = 0;
  target.focus();

  const name = target.getAttribute("data-table");
  if (name) previewSidebarSelection(name);
}

function bindQueryResize(): void {
  const handle = document.getElementById("query-resize");
  const panel = document.querySelector(".query-panel") as HTMLElement | null;
  const content = document.querySelector(".content") as HTMLElement | null;
  if (!handle || !panel || !content) return;

  // Applied straight to the element rather than through render(): a rebuild
  // per pointer move would drop focus and fight the grid's scroll position.
  const resizeTo = (height: number): void => {
    const available = content.getBoundingClientRect().height;
    const max = Math.max(QUERY_PANEL_MIN_PX, available - GRID_MIN_PX);
    const clamped = clamp(height, QUERY_PANEL_MIN_PX, max);
    state.queryHeight = clamped;
    panel.style.height = `${clamped}px`;
    panel.style.maxHeight = "none";
  };

  handle.addEventListener("pointerdown", (e) => {
    e.preventDefault();
    handle.setPointerCapture(e.pointerId);

    const onMove = (move: PointerEvent) => {
      resizeTo(content.getBoundingClientRect().bottom - move.clientY);
    };
    const onUp = (up: PointerEvent) => {
      handle.releasePointerCapture(up.pointerId);
      handle.removeEventListener("pointermove", onMove);
      handle.removeEventListener("pointerup", onUp);
      saveQueryHeight();
    };

    handle.addEventListener("pointermove", onMove);
    handle.addEventListener("pointerup", onUp);
  });

  // Back to sizing itself to its contents.
  handle.addEventListener("dblclick", () => {
    state.queryHeight = null;
    saveQueryHeight();
    render();
  });

  handle.addEventListener("keydown", (e) => {
    if (e.key !== "ArrowUp" && e.key !== "ArrowDown") return;
    e.preventDefault();
    resizeTo(panel.getBoundingClientRect().height + (e.key === "ArrowUp" ? 24 : -24));
    saveQueryHeight();
  });
}

function openQuery(): void {
  state.queryOpen = true;
  render();
  document.getElementById("query-input")?.focus();
}

/// Closing discards the results too — leaving them behind would mean the panel
/// reopened with stale output next time.
function closeQuery(): void {
  state.queryOpen = false;
  state.queryResult = null;
  state.queryError = null;
  render();
}

/// Empties the box and the output together: results left standing under a
/// cleared query read as belonging to it.
function clearQuery(): void {
  state.queryInput = "";
  state.queryResult = null;
  state.queryError = null;
  render();
  document.getElementById("query-input")?.focus();
}

function toggleQuery(): void {
  if (state.queryOpen) closeQuery();
  else openQuery();
}

function toggleSchema(): void {
  state.schemaOpen = !state.schemaOpen;
  render();
}

function toggleHelp(): void {
  state.helpOpen = !state.helpOpen;
  render();
}

function bindHelpOverlay(): void {
  const overlay = document.getElementById("help-overlay");
  // Clicking the backdrop dismisses; clicking the panel itself must not.
  overlay?.addEventListener("click", (e) => {
    if (e.target === overlay) toggleHelp();
  });
}

function render(): void {
  const app = document.getElementById("app")!;
  const carriedScroll = tableWrapper()?.scrollTop ?? 0;
  // Rebuilding the DOM drops focus, which would throw the user out of the
  // sidebar every time picking a table re-rendered it.
  const focusedTable = document.activeElement
    ?.closest(".sidebar-item")
    ?.getAttribute("data-table") ?? null;

  if (!state.dbInfo) {
    app.innerHTML = `
      <div class="welcome">
        <h1>DBiewLite</h1>
        <p>Open a database file to get started</p>
        <button id="open-btn" class="btn">Open Database</button>
      </div>
      ${state.helpOpen ? renderHelpOverlay() : ""}
    `;
    document.getElementById("open-btn")?.addEventListener("click", handleOpenFile);
    bindHelpOverlay();
    return;
  }

  const info = state.dbInfo;
  const fileName = info.path.split("/").pop() ?? info.path;

  // A roving tabindex: one item in the whole list is a tab stop, so Tab steps
  // into the sidebar and then out to the grid rather than walking every table.
  // j/k move within, and hand the tab stop along as they go.
  const sidebarNames = [...state.tables.map(t => t.name), ...state.views];
  const tabStop = state.selectedTable !== null && sidebarNames.includes(state.selectedTable)
    ? state.selectedTable
    : sidebarNames[0] ?? null;

  app.innerHTML = `
    <div class="layout">
      <div class="title-bar">
        <button id="db-details-toggle" class="disclosure db-title" aria-expanded="${state.detailsOpen}" aria-controls="db-details">
          <span class="db-title-name">${escapeHtml(fileName)}</span>
          <span class="disclosure-chevron ${state.detailsOpen ? "open" : ""}">\u203a</span>
        </button>
        <div class="title-actions">
          <button id="theme-btn" class="btn btn-sm">Theme</button>
        </div>
      </div>
      ${state.detailsOpen ? renderDbDetails(info) : ""}
      <div id="status-toast" class="status-toast hidden"></div>
      <div class="main-area">
        <div class="sidebar" role="listbox" aria-label="Tables and views">
          <div class="sidebar-section">
            <div class="sidebar-header">Tables</div>
            ${state.tables.map(t => `
              <div class="sidebar-item ${t.name === state.selectedTable ? "active" : ""}" data-table="${t.name}"
                   role="option" tabindex="${t.name === tabStop ? 0 : -1}" aria-selected="${t.name === state.selectedTable}">
                <span class="sidebar-icon">\u{f0ce}</span>
                <span class="sidebar-name">${escapeHtml(t.name)}</span>
                <span class="sidebar-count">${t.row_count}</span>
              </div>
            `).join("")}
          </div>
          ${state.views.length > 0 ? `
            <div class="sidebar-section">
              <div class="sidebar-header">Views</div>
              ${state.views.map(v => `
                <div class="sidebar-item sidebar-view ${v === state.selectedTable ? "active" : ""}" data-table="${v}"
                     role="option" tabindex="${v === tabStop ? 0 : -1}" aria-selected="${v === state.selectedTable}">
                  <span class="sidebar-icon">\u{f06e}</span>
                  <span class="sidebar-name">${escapeHtml(v)}</span>
                </div>
              `).join("")}
            </div>
          ` : ""}
        </div>
        <div class="content">
          ${renderDataTable()}
          ${state.queryOpen ? renderQueryPanel() : ""}
        </div>
      </div>
    </div>
    ${state.helpOpen ? renderHelpOverlay() : ""}
  `;

  // Bind events
  bindHelpOverlay();
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

  document.getElementById("schema-toggle")?.addEventListener("click", toggleSchema);
  document.getElementById("prev-page")?.addEventListener("click", () => { void turnPage(-1); });
  document.getElementById("next-page")?.addEventListener("click", () => { void turnPage(1); });
  document.getElementById("export-btn")?.addEventListener("click", exportCsv);

  const wrapper = tableWrapper();
  if (wrapper) {
    wrapper.addEventListener("scroll", () => { onTableScroll(wrapper); });
    wrapper.querySelectorAll("td[data-cell]").forEach(el => {
      el.addEventListener("click", () => {
        const row = Number(el.getAttribute("data-row"));
        const col = Number(el.getAttribute("data-cell"));
        state.cursorRow = state.page * state.pageSize + row;
        state.cursorCol = col;
        paintCursor();
      });
    });
  }
  applyPendingScroll(carriedScroll);
  paintCursor();

  if (focusedTable !== null) {
    sidebarItems().find(el => el.getAttribute("data-table") === focusedTable)?.focus();
  }

  const queryInput = document.getElementById("query-input") as HTMLTextAreaElement | null;
  if (queryInput) {
    queryInput.value = state.queryInput;
    queryInput.addEventListener("input", (e) => {
      state.queryInput = (e.target as HTMLTextAreaElement).value;
    });
    queryInput.addEventListener("keydown", (e) => {
      // Enter runs, as it does in the TUI, where the input is a single line.
      // Shift+Enter is the way to a second line here.
      if (e.key === "Enter" && !e.shiftKey) {
        e.preventDefault();
        void runQuery();
        return;
      }
      if (e.key === "Escape") {
        e.preventDefault();
        queryInput.blur();
        // Nothing has been read yet, so the panel has no reason to stay.
        if (!state.queryResult && !state.queryError) closeQuery();
      }
    });
  }

  document.getElementById("query-btn")?.addEventListener("click", toggleQuery);
  document.getElementById("run-query-btn")?.addEventListener("click", runQuery);
  document.getElementById("clear-query-btn")?.addEventListener("click", clearQuery);
  document.getElementById("export-query-btn")?.addEventListener("click", () => { void exportQueryCsv(); });
  bindQueryResize();
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

  const cursorPageRow = state.cursorRow - state.page * state.pageSize;
  const rows = state.data.rows.map((row, rowIndex) => {
    const onCursorRow = rowIndex === cursorPageRow;
    const cells = row.map((val, colIndex) => {
      const cursor = onCursorRow && colIndex === state.cursorCol ? " cell-cursor" : "";
      return `<td class="${cellClass(val)}${cursor}" data-row="${rowIndex}" data-cell="${colIndex}">${escapeHtml(formatCellValue(val))}</td>`;
    }).join("");
    return `<tr class="${onCursorRow ? "row-cursor" : ""}">${cells}</tr>`;
  }).join("");

  const schemaInfo = state.schema.map(c => {
    const pk = c.primary_key ? " PK" : "";
    const nullable = c.nullable ? "" : " NOT NULL";
    return `<span class="schema-chip">${c.name}: ${c.col_type || "ANY"}${pk}${nullable}</span>`;
  }).join("");

  return `
    <div class="data-panel">
      <div class="data-header">
        <button id="schema-toggle" class="disclosure data-title" aria-expanded="${state.schemaOpen}" aria-controls="schema-bar"
                title="Column details">
          <span>${escapeHtml(state.selectedTable)}</span>
          <span class="disclosure-chevron ${state.schemaOpen ? "open" : ""}">\u203a</span>
        </button>
        <span class="data-info">${start}\u2013${end} of ${total}</span>
        <div class="data-actions">
          <button id="prev-page" class="btn btn-sm" ${state.page === 0 ? "disabled" : ""}>\u25c0</button>
          <button id="next-page" class="btn btn-sm" ${end >= total ? "disabled" : ""}>\u25b6</button>
          <button id="query-btn" class="btn btn-sm ${state.queryOpen ? "active" : ""}">Query</button>
          <button id="export-btn" class="btn btn-sm">Export .csv</button>
        </div>
      </div>
      ${state.schemaOpen ? `<div id="schema-bar" class="schema-bar">${schemaInfo}</div>` : ""}
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
      const cells = row.map(val => `<td class="${cellClass(val)}">${escapeHtml(formatCellValue(val))}</td>`).join("");
      return `<tr>${cells}</tr>`;
    }).join("");
    resultHtml = `
      <div class="query-result-info">
        <span>${state.queryResult.rows.length} rows returned</span>
        <button id="export-query-btn" class="btn btn-sm">Export .csv</button>
      </div>
      <div class="table-wrapper">
        <table class="data-table">
          <thead><tr>${qHeaders}</tr></thead>
          <tbody>${qRows}</tbody>
        </table>
      </div>
    `;
  }

  const nothingToClear = state.queryInput === "" && !state.queryResult && !state.queryError;

  // Sized by contents until the divider is dragged, then pinned.
  const sized = state.queryHeight === null
    ? ""
    : ` style="height:${Math.round(state.queryHeight)}px;max-height:none"`;

  return `
    <div class="query-panel"${sized}>
      <div id="query-resize" class="query-resize" role="separator" aria-orientation="horizontal"
           tabindex="0" aria-label="Resize the query panel" title="Drag to resize, double-click to reset"></div>
      <div class="query-input-area">
        <textarea id="query-input" placeholder="SELECT … — Enter runs, Shift+Enter for a new line, Esc closes" rows="2"></textarea>
        <button id="run-query-btn" class="btn">Run</button>
        <button id="clear-query-btn" class="btn" ${nothingToClear ? "disabled" : ""}>Clear</button>
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
    // Cmd+O and Cmd+E are accelerators on the File menu now. The menu consumes
    // them before the webview sees them, so handling them here as well would
    // either do nothing or fire twice.
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
    // Everything below drives the grid, so it only applies when the query box
    // is not the one being typed into.
    if (e.isComposing || isTypingTarget(document.activeElement)) return;

    // The overlay swallows every key, the way the TUI's does: the key that
    // opened it closes it again, and nothing reaches the grid meanwhile.
    if (state.helpOpen) {
      e.preventDefault();
      if (e.key === "Escape" || e.key === "Enter" || e.key === "?") toggleHelp();
      return;
    }
    if (e.key === "?") {
      e.preventDefault();
      toggleHelp();
      return;
    }
    if (!state.dbInfo) return;

    // The sidebar answers to the same movement keys when it holds focus.
    // Which panel is active is just where the browser's focus already is,
    // rather than a second notion of it kept in step by hand.
    if (document.activeElement?.closest(".sidebar")) {
      switch (e.key) {
        case "ArrowDown":
        case "j":
          e.preventDefault();
          moveSidebarFocus(1);
          return;
        case "ArrowUp":
        case "k":
          e.preventDefault();
          moveSidebarFocus(-1);
          return;
        case "g":
          e.preventDefault();
          sidebarItems()[0]?.focus();
          return;
        case "G":
          e.preventDefault();
          sidebarItems().at(-1)?.focus();
          return;
        case "Enter":
        case " ":
          e.preventDefault();
          commitSidebarSelection();
          return;
        case "Escape":
          e.preventDefault();
          window.clearTimeout(sidebarPreviewTimer);
          (document.activeElement as HTMLElement).blur();
          return;
      }
      // App-wide keys still work from here; the grid's do not, since the
      // cursor is not what the arrow keys are pointing at right now.
      if (e.key !== "/" && e.key !== ":" && e.key !== "i") return;
    }

    // Half-viewport jumps. Ctrl rather than Cmd, matching the TUI — and they
    // are free outside a text field, where macOS would read them as
    // forward-delete and delete-to-line-start.
    if (e.ctrlKey && !e.metaKey && (e.key === "d" || e.key === "u")) {
      e.preventDefault();
      const step = Math.max(1, Math.floor(visibleRowCount() / 2));
      void moveCursorRow(e.key === "d" ? step : -step);
      return;
    }
    if (e.metaKey || e.ctrlKey || e.altKey) return;

    switch (e.key) {
      case "ArrowDown":
      case "j":
        e.preventDefault();
        void moveCursorRow(1);
        break;
      case "ArrowUp":
      case "k":
        e.preventDefault();
        void moveCursorRow(-1);
        break;
      // Shift pans the grid sideways; unshifted moves the cursor and lets the
      // grid follow. H/L are the same keys under a different name.
      case "H":
        e.preventDefault();
        panColumns(-1);
        break;
      case "L":
        e.preventDefault();
        panColumns(1);
        break;
      case "ArrowLeft":
      case "h":
        e.preventDefault();
        if (e.shiftKey) panColumns(-1);
        else moveCursorCol(-1);
        break;
      case "ArrowRight":
      case "l":
        e.preventDefault();
        if (e.shiftKey) panColumns(1);
        else moveCursorCol(1);
        break;
      case "Home":
        e.preventDefault();
        state.cursorCol = 0;
        paintCursor();
        break;
      case "End":
        e.preventDefault();
        state.cursorCol = Math.max(0, (state.data?.columns.length ?? 1) - 1);
        paintCursor();
        break;
      case "s":
        e.preventDefault();
        sortCursorColumn();
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
        openQuery();
        break;
      case "Escape":
        if (state.queryOpen) {
          e.preventDefault();
          closeQuery();
        }
        break;
      case "i":
        e.preventDefault();
        state.detailsOpen = !state.detailsOpen;
        render();
        break;
      case "c":
        e.preventDefault();
        toggleSchema();
        break;
    }
  });
}

function isTypingTarget(el: Element | null): boolean {
  return el instanceof HTMLTextAreaElement || el instanceof HTMLInputElement;
}

/// Registered once for the app's lifetime, so the returned unlisten handles
/// are not kept. The rejections are: without a catch these become unhandled
/// whenever the page runs outside Tauri, which is every time the frontend is
/// opened in a plain browser to work on the UI.
function setupMenuListeners(): void {
  const menuEvents: [string, () => void][] = [
    ["menu:open", () => { void handleOpenFile(); }],
    ["menu:export", () => { void exportCsv(); }],
    ["menu:shortcuts", toggleHelp],
  ];
  for (const [event, handler] of menuEvents) {
    listen(event, handler).catch((e: unknown) => {
      console.error(`could not listen for ${event}:`, e);
    });
  }
}

function init(): void {
  initTheme();
  setupKeyboardShortcuts();
  setupMenuListeners();
  render();
}

init();
