use std::ops::Range;

use dbiewlite_core::{CellValue, QueryResult, format_size};
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, List, ListItem, Padding, Paragraph, Row, Table,
};
use ratatui::Frame;
use unicode_width::UnicodeWidthStr;

use crate::app::{App, AppMode, ColMove, Panel};

// Terminal-native colors matching PanEx TUI style
const ACTIVE: Color = Color::Green;
const TEXT_MUTED: Color = Color::DarkGray;
const BORDER: Color = Color::DarkGray;
/// Amber accent — table icons and the footer brand.
const ACCENT: Color = Color::Rgb(255, 191, 0);
/// Padded so a clipped status line can't butt up against it.
const BRAND: &str = "  dbiew ";
/// Scroll thumb glyph. Half-width so it reads lighter than a full block while
/// staying distinct from the `│` border it is drawn over.
const SCROLL_THUMB: &str = "▐";

const SIDEBAR_WIDTH: u16 = 26;
const MIN_COL_WIDTH: usize = 6;
const MAX_COL_WIDTH: usize = 40;
const COL_SPACING: u16 = 1;

pub fn draw(f: &mut Frame, app: &mut App) {
    let area = f.area();

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Min(0),    // Main area
            Constraint::Length(1), // Status bar
        ])
        .split(area);

    draw_main(f, app, chunks[0]);
    draw_status_bar(f, app, chunks[1]);

    match app.mode {
        AppMode::Help => draw_help_dialog(f, area),
        AppMode::Info => draw_info_dialog(f, app, area),
        _ => {}
    }
}

fn draw_main(f: &mut Frame, app: &mut App, area: Rect) {
    if app.sidebar_collapsed {
        // Cleared so a stale rectangle can't swallow mouse events.
        app.sidebar_area = Rect::default();
        draw_content(f, app, area);
    } else {
        let chunks = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Length(SIDEBAR_WIDTH), // Sidebar
                Constraint::Min(0),                // Content
            ])
            .split(area);

        draw_sidebar(f, app, chunks[0]);
        draw_content(f, app, chunks[1]);
    }
}

fn draw_sidebar(f: &mut Frame, app: &mut App, area: Rect) {
    app.sidebar_area = area;

    let is_active = app.active_panel == Panel::Sidebar;
    let border_color = if is_active { ACTIVE } else { BORDER };

    let block = Block::default()
        .title(" Tables ")
        .title_style(Style::default().fg(if is_active { ACTIVE } else { TEXT_MUTED }))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));
    let inner = block.inner(area);

    let selected = app.sidebar_render_index();

    let items: Vec<ListItem> = app
        .tables
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let is_sel = i == selected;
            let icon_style = if is_sel {
                Style::default().fg(ACCENT).bg(Color::Blue)
            } else {
                Style::default().fg(ACCENT)
            };
            let name_style = if is_sel {
                Style::default()
                    .fg(Color::White)
                    .bg(Color::Blue)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(Color::Blue).add_modifier(Modifier::BOLD)
            };
            let count_style = if is_sel {
                Style::default().fg(Color::DarkGray).bg(Color::Blue)
            } else {
                Style::default().fg(Color::DarkGray)
            };
            ListItem::new(Line::from(vec![
                Span::styled(" \u{f0ce} ", icon_style),
                Span::styled(&t.name, name_style),
                Span::styled(format!(" ({})", t.row_count), count_style),
            ]))
        })
        .chain(if !app.views.is_empty() {
            let mut items = vec![ListItem::new("").style(Style::default().fg(TEXT_MUTED))];
            items.push(
                ListItem::new(" \u{2500} Views \u{2500}").style(Style::default().fg(TEXT_MUTED)),
            );
            for (i, v) in app.views.iter().enumerate() {
                let row = app.tables.len() + 2 + i;
                let style = if row == selected {
                    Style::default().fg(Color::White).bg(Color::Blue)
                } else {
                    Style::default().fg(TEXT_MUTED)
                };
                items.push(ListItem::new(format!(" {}", v)).style(style));
            }
            items
        } else {
            vec![]
        })
        .collect();

    let len = items.len();
    let list = List::new(items).block(block);
    // Drives scrolling only — the highlight is painted per item above so the
    // separator rows keep their own styling.
    app.sidebar_state.select(Some(selected));
    f.render_stateful_widget(list, area, &mut app.sidebar_state);

    draw_scroll_thumb(
        f,
        thumb_track(area, inner.y, inner.height),
        len,
        inner.height as usize,
        app.sidebar_state.offset(),
    );
}

/// The one-column strip on a panel's right border where its scroll thumb goes.
fn thumb_track(area: Rect, y: u16, height: u16) -> Rect {
    Rect {
        x: area.x + area.width.saturating_sub(1),
        y,
        width: 1,
        height,
    }
}

fn draw_content(f: &mut Frame, app: &mut App, area: Rect) {
    let has_query = app.query_visible();

    // `+` hands the results the share the grid had, `-` gives it back.
    let constraints = match (has_query, app.query_expanded) {
        (false, _) => vec![Constraint::Percentage(100)],
        (true, false) => vec![Constraint::Percentage(60), Constraint::Percentage(40)],
        (true, true) => vec![Constraint::Percentage(40), Constraint::Percentage(60)],
    };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints(constraints)
        .split(area);

    draw_data_table(f, app, chunks[0]);

    if has_query && chunks.len() > 1 {
        draw_query_panel(f, app, chunks[1]);
    }
}

/// Text and color for a cell. Shared by the width pass and the render pass so
/// the computed column widths match what actually gets drawn.
fn cell_display(val: &CellValue) -> (String, Color) {
    match val {
        CellValue::Null => ("NULL".to_string(), TEXT_MUTED),
        CellValue::Integer(n) => (n.to_string(), Color::Cyan),
        CellValue::Real(r) => (format!("{}", r), Color::Cyan),
        // Drawn as nothing at all otherwise, which reads as a NULL or as a
        // column that failed to render.
        CellValue::Text(s) if s.is_empty() => ("<empty>".to_string(), TEXT_MUTED),
        CellValue::Text(s) => {
            // Truncate by chars, not bytes — slicing mid-codepoint panics.
            let display = if s.chars().count() > MAX_COL_WIDTH {
                let head: String = s.chars().take(MAX_COL_WIDTH - 1).collect();
                format!("{}\u{2026}", head)
            } else {
                s.clone()
            };
            (display, Color::Reset)
        }
        CellValue::Blob(b) => (format!("<blob {} B>", b.len()), TEXT_MUTED),
    }
}

/// Width each column wants: the widest of its header and its visible cells,
/// clamped so one long text column can't push the rest off screen.
fn column_widths(data: &QueryResult, header_pad: usize) -> Vec<u16> {
    data.columns
        .iter()
        .enumerate()
        .map(|(i, col)| {
            let mut w = UnicodeWidthStr::width(col.as_str()) + header_pad;
            for row in &data.rows {
                if let Some(cell) = row.get(i) {
                    w = w.max(UnicodeWidthStr::width(cell_display(cell).0.as_str()));
                }
            }
            w.clamp(MIN_COL_WIDTH, MAX_COL_WIDTH) as u16
        })
        .collect()
}

/// The columns that fit in `avail` starting at `offset`. Always yields at least
/// one column, even if it overflows — better a clipped column than none.
fn visible_range(widths: &[u16], offset: usize, avail: u16) -> Range<usize> {
    if widths.is_empty() {
        return 0..0;
    }
    let offset = offset.min(widths.len() - 1);
    let mut used: u32 = 0;
    let mut end = offset;
    while end < widths.len() {
        let need = widths[end] as u32 + if end > offset { COL_SPACING as u32 } else { 0 };
        if end > offset && used + need > avail as u32 {
            break;
        }
        used += need;
        end += 1;
    }
    offset..end
}

fn draw_data_table(f: &mut Frame, app: &mut App, area: Rect) {
    app.data_area = area;
    // Two borders plus the header row.
    app.data_rows_visible = area.height.saturating_sub(3) as usize;

    let is_active = app.active_panel == Panel::Data;
    let border_color = if is_active { ACTIVE } else { BORDER };

    let Some(tv) = &mut app.table_view else {
        let block = Block::default()
            .title(" No table selected ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));
        let msg = Paragraph::new("Select a table from the sidebar")
            .style(Style::default().fg(TEXT_MUTED))
            .block(block);
        f.render_widget(msg, area);
        return;
    };

    let total = tv.data.total_rows.unwrap_or(0);
    let start = tv.page * tv.page_size + 1;
    let end = std::cmp::min(start + tv.data.rows.len().saturating_sub(1), total as usize);

    // Header cells carry a two-cell sort indicator, so reserve room for it.
    let widths = column_widths(&tv.data, 2);
    // Two borders plus the block's horizontal padding. Computed rather than
    // taken from `block.inner()` because the title below needs the range first.
    let inner_width = area.width.saturating_sub(4);

    // Reconcile the cursor with the viewport. Which rule applies depends on
    // which the user moved last, otherwise revealing the cursor would
    // immediately undo a pan.
    match tv.col_move {
        ColMove::Cursor if tv.cursor_col < tv.col_offset => {
            tv.col_offset = tv.cursor_col;
        }
        ColMove::Cursor => {
            // `visible_range` always yields at least one column, so `end`
            // exceeds `col_offset` and this terminates by `cursor_col`.
            while tv.col_offset < tv.cursor_col
                && visible_range(&widths, tv.col_offset, inner_width).end <= tv.cursor_col
            {
                tv.col_offset += 1;
            }
        }
        ColMove::Pan => {}
    }

    let range = visible_range(&widths, tv.col_offset, inner_width);
    if tv.col_move == ColMove::Pan {
        tv.cursor_col = tv
            .cursor_col
            .clamp(range.start, range.end.saturating_sub(1).max(range.start));
    }

    let mut title = format!(" {} \u{2502} {}-{} of {} ", tv.name, start, end, total);
    if range.len() < tv.data.columns.len() {
        title = format!(
            " {} \u{2502} {}-{} of {} \u{2502} cols {}-{}/{} ",
            tv.name,
            start,
            end,
            total,
            range.start + 1,
            range.end,
            tv.data.columns.len()
        );
    }

    let block = Block::default()
        .title(title)
        .title_style(Style::default().fg(if is_active { ACTIVE } else { Color::Reset }))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .padding(Padding::horizontal(1));
    let inner = block.inner(area);

    tv.col_spans = range
        .clone()
        .scan(inner.x, |x, i| {
            let span = (i, *x, widths[i]);
            *x = x.saturating_add(widths[i] + COL_SPACING);
            Some(span)
        })
        .collect();

    let header_cells: Vec<Cell> = tv.data.columns[range.clone()]
        .iter()
        .enumerate()
        .map(|(i, col)| {
            let is_sorted = matches!(&tv.sort, Some(s) if s.column == *col);
            let indicator = match &tv.sort {
                Some(s) if s.column == *col => {
                    if s.ascending {
                        " \u{25b2}"
                    } else {
                        " \u{25bc}"
                    }
                }
                _ => "",
            };
            // The cursor column reads as a chip, matching the sidebar's
            // selected row, so it's obvious what `s` will sort.
            let style = if range.start + i == tv.cursor_col {
                Style::default().fg(Color::White).bg(Color::Blue)
            } else if is_sorted {
                Style::default().fg(Color::Cyan)
            } else {
                Style::default().fg(TEXT_MUTED)
            };
            Cell::from(format!("{}{}", col, indicator))
                .style(style.add_modifier(Modifier::BOLD))
        })
        .collect();
    let header = Row::new(header_cells).height(1);

    let rows: Vec<Row> = tv
        .data
        .rows
        .iter()
        .map(|row| {
            let cells: Vec<Cell> = row
                .get(range.clone())
                .unwrap_or(&[])
                .iter()
                .map(|val| {
                    let (text, color) = cell_display(val);
                    Cell::from(text).style(Style::default().fg(color))
                })
                .collect();
            Row::new(cells).height(1)
        })
        .collect();

    let constraints: Vec<Constraint> = widths[range.clone()]
        .iter()
        .map(|w| Constraint::Length(*w))
        .collect();

    let table = Table::new(rows, constraints)
        .header(header)
        .block(block)
        .column_spacing(COL_SPACING)
        .row_highlight_style(
            Style::default()
                .bg(Color::DarkGray)
                .fg(Color::White)
                .add_modifier(Modifier::BOLD),
        );

    f.render_stateful_widget(table, area, &mut tv.table_state);

    // Read the offset *after* rendering — the table adjusts it to keep the
    // selected row visible. The header eats the first inner row.
    //
    // The thumb tracks the whole table, not the current page, so it stays an
    // honest picture of where you are in a 10-million-row file.
    let viewport = inner.height.saturating_sub(1);
    let len = (total as usize).max(tv.data.rows.len());
    let offset = tv.page * tv.page_size + tv.table_state.offset();
    draw_scroll_thumb(
        f,
        thumb_track(area, inner.y + 1, viewport),
        len,
        viewport as usize,
        offset,
    );
}

/// Draws a scroll thumb over `area` (one column, spanning the list rows).
///
/// Hand-rolled rather than `ratatui::Scrollbar`, which rounds the thumb's start
/// and end independently and so visibly changes the thumb's length by a cell as
/// you scroll. Here the length is computed once and only the position moves, so
/// the thumb sits flush at both ends and never resizes mid-scroll.
fn draw_scroll_thumb(f: &mut Frame, area: Rect, len: usize, viewport: usize, offset: usize) {
    let track = area.height as usize;
    // Nothing to indicate when everything already fits.
    if track == 0 || viewport == 0 || len <= viewport {
        return;
    }

    // Length is proportional to the visible fraction, fixed for a given list.
    let thumb = (track * viewport / len).clamp(1, track);
    let travel = track - thumb;
    let max_offset = len - viewport;
    // Round to nearest so the thumb lands flush at the top and the bottom.
    let start = (offset.min(max_offset) * travel + max_offset / 2) / max_offset;

    let style = Style::default().fg(TEXT_MUTED);
    let buf = f.buffer_mut();
    for i in start..start + thumb {
        let y = area.y + i as u16;
        if y >= area.y + area.height {
            break;
        }
        // `cell_mut` returns None off-buffer — a panel squeezed to the screen
        // edge can put this column out of range, and indexing would panic.
        if let Some(cell) = buf.cell_mut((area.x, y)) {
            cell.set_symbol(SCROLL_THUMB).set_style(style);
        }
    }
}

fn draw_query_panel(f: &mut Frame, app: &mut App, area: Rect) {
    let is_active = app.active_panel == Panel::Query;
    let border_color = if is_active { ACTIVE } else { BORDER };

    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Length(3), Constraint::Min(0)])
        .split(area);

    // Query input
    let input_block = Block::default()
        .title(" SQL Query ")
        .title_style(Style::default().fg(if is_active { ACTIVE } else { TEXT_MUTED }))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color));

    let input = Paragraph::new(app.query_input.as_str())
        .style(Style::default().fg(Color::Reset))
        .block(input_block);
    f.render_widget(input, chunks[0]);

    // Show cursor when in query mode
    if app.mode == AppMode::QueryInput {
        f.set_cursor_position((chunks[0].x + app.query_cursor as u16 + 1, chunks[0].y + 1));
    }

    // Query results
    let result_block = Block::default()
        .title(" Results ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(if is_active { ACTIVE } else { BORDER }));

    if let Some(err) = &app.query_error {
        let msg = Paragraph::new(err.as_str())
            .style(Style::default().fg(Color::Red))
            .block(result_block);
        f.render_widget(msg, chunks[1]);
    } else if let Some(result) = &app.query_result {
        let header_cells: Vec<Cell> = result
            .columns
            .iter()
            .map(|c| {
                Cell::from(c.as_str())
                    .style(Style::default().fg(Color::Cyan).add_modifier(Modifier::BOLD))
            })
            .collect();
        let header = Row::new(header_cells).height(1);

        let rows: Vec<Row> = result
            .rows
            .iter()
            .map(|row| {
                let cells: Vec<Cell> = row
                    .iter()
                    .map(|v| {
                        let (text, color) = cell_display(v);
                        Cell::from(text).style(Style::default().fg(color))
                    })
                    .collect();
                Row::new(cells).height(1)
            })
            .collect();

        let constraints: Vec<Constraint> = column_widths(result, 0)
            .iter()
            .map(|w| Constraint::Length(*w))
            .collect();

        let table = Table::new(rows, constraints)
            .header(header)
            .block(result_block)
            .column_spacing(COL_SPACING)
            .row_highlight_style(
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD),
            );
        // Stateful so the results can be scrolled rather than only glimpsed.
        f.render_stateful_widget(table, chunks[1], &mut app.query_state);
    } else {
        f.render_widget(result_block, chunks[1]);
    }
}

fn draw_status_bar(f: &mut Frame, app: &App, area: Rect) {
    let db_name = std::path::Path::new(&app.db_info.path)
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_default();

    let dim = Style::default().fg(TEXT_MUTED);
    let sep = Span::styled(" \u{2502} ", dim);

    // Running a query returns to Normal mode while leaving its panel up and
    // focused, and the generic hint named none of the keys that then apply —
    // clearing, exporting or dismissing it — which is exactly the moment they
    // are looked for. Naming the export target here also settles what Ctrl+E
    // writes, which otherwise depends on a focus the reader has to infer.
    let showing_results = app.query_result.is_some() || app.query_error.is_some();
    let keys = match app.mode {
        AppMode::QueryInput => "Esc:cancel  Enter:run  Ctrl+U:clear",
        AppMode::Help => "Esc/q/?:close",
        AppMode::Info => "Esc/q/i:close",
        AppMode::Normal if showing_results && app.active_panel == Panel::Query => {
            "j/k:rows  +/-:size  /:edit  Ctrl+U:clear  Ctrl+E:export  Esc:hide"
        }
        AppMode::Normal if showing_results => "Esc:hide results  Ctrl+E:export table  /:query",
        AppMode::Normal => "q:quit  Tab:panel  ?:help  s:sort  Ctrl+B:tables  Ctrl+E:export  /:query",
    };

    let mut left = vec![
        Span::raw(" "),
        Span::styled(db_name, Style::default().fg(Color::Reset)),
        sep.clone(),
    ];
    if let Some(msg) = &app.status_message {
        left.push(Span::styled(msg.clone(), Style::default().fg(Color::Yellow)));
        left.push(sep);
    }
    left.push(Span::styled(keys, dim));

    // Split the row so a long status line is clipped rather than running under
    // the brand.
    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Min(0),
            Constraint::Length(BRAND.chars().count() as u16),
        ])
        .split(area);

    f.render_widget(Paragraph::new(Line::from(left)), cols[0]);
    f.render_widget(
        Paragraph::new(BRAND).style(Style::default().fg(ACCENT).add_modifier(Modifier::BOLD)),
        cols[1],
    );
}

fn thousands(n: u64) -> String {
    let s = n.to_string();
    let mut out = String::with_capacity(s.len() + s.len() / 3);
    for (i, ch) in s.chars().enumerate() {
        if i > 0 && (s.len() - i).is_multiple_of(3) {
            out.push(',');
        }
        out.push(ch);
    }
    out
}

/// Shortens a path for display: `$HOME` becomes `~`, and anything still too
/// long is clipped from the left so the meaningful tail survives.
fn short_path(path: &str, max: usize) -> String {
    let mut s = path.to_string();
    if let Some(home) = dirs::home_dir().and_then(|h| h.to_str().map(str::to_string))
        && let Some(rest) = s.strip_prefix(&home)
    {
        s = format!("~{}", rest);
    }
    if s.chars().count() > max {
        let skip = s.chars().count() - max + 1;
        s = format!("\u{2026}{}", s.chars().skip(skip).collect::<String>());
    }
    s
}

/// Facts about the open file, grouped into sections.
///
/// Deliberately not one fixed field list: a Parquet file has no tables, views,
/// indexes or pages, and its `engine_version` is really the version of the
/// DuckDB that reads it — printing "Parquet 1.4.1" would invent a format
/// version that doesn't exist.
fn info_sections(app: &App, value_width: usize) -> Vec<(&'static str, Vec<(String, String)>)> {
    let info = &app.db_info;
    let is_parquet = info.engine == "Parquet";

    // Resolved so the folder is useful even when the app was launched with a
    // relative path; falls back to whatever was passed if the file moved.
    let path = std::fs::canonicalize(&info.path)
        .unwrap_or_else(|_| std::path::PathBuf::from(&info.path));
    let name = path
        .file_name()
        .map(|f| f.to_string_lossy().to_string())
        .unwrap_or_else(|| info.path.clone());
    let folder = path
        .parent()
        .map(|p| short_path(&p.to_string_lossy(), value_width))
        .unwrap_or_default();

    let mut file = vec![
        ("Name".to_string(), name),
        ("Folder".to_string(), folder),
        ("Size".to_string(), format_size(info.file_size)),
    ];

    if is_parquet {
        file.push(("Format".to_string(), "Parquet".to_string()));
        // Named separately so the DuckDB version is never mistaken for a
        // Parquet format version.
        file.push((
            "Reader".to_string(),
            format!("DuckDB {}", info.engine_version),
        ));
    } else {
        file.push((
            "Engine".to_string(),
            format!("{} {}", info.engine, info.engine_version),
        ));
        if let (Some(count), Some(size)) = (info.page_count, info.page_size) {
            file.push((
                "Pages".to_string(),
                format!("{} \u{00d7} {}", thousands(count), format_size(size)),
            ));
        }
    }

    let total_rows: u64 = app.tables.iter().map(|t| t.row_count).sum();
    let mut sections = vec![("File", file)];

    if is_parquet {
        // One table by definition, so report the shape of the data directly.
        let columns = app.tables.first().map(|t| t.column_count).unwrap_or(0);
        sections.push((
            "Contents",
            vec![
                ("Columns".to_string(), columns.to_string()),
                ("Rows".to_string(), thousands(total_rows)),
            ],
        ));
        return sections;
    }

    let mut contents = vec![
        ("Tables".to_string(), thousands(app.tables.len() as u64)),
        ("Views".to_string(), thousands(app.views.len() as u64)),
        ("Indexes".to_string(), thousands(app.index_count as u64)),
        ("Rows".to_string(), format!("{} total", thousands(total_rows))),
    ];
    if app.views.is_empty() {
        contents.remove(1);
    }
    sections.push(("Contents", contents));

    if let Some(tv) = &app.table_view {
        sections.push((
            "Selected",
            vec![(
                tv.name.clone(),
                format!(
                    "{} columns \u{00b7} {} rows",
                    tv.data.columns.len(),
                    thousands(tv.data.total_rows.unwrap_or(0))
                ),
            )],
        ));
    }

    sections
}

fn draw_info_dialog(f: &mut Frame, app: &App, area: Rect) {
    let width = 62u16.min(area.width.saturating_sub(4));
    let label_width = 12usize;
    let value_width = (width as usize).saturating_sub(label_width + 5);
    let sections = info_sections(app, value_width);

    let mut lines: Vec<Line> = Vec::new();
    for (i, (title, rows)) in sections.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            format!(" {}", title),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
        for (label, value) in rows {
            lines.push(Line::from(vec![
                Span::styled(
                    format!("  {:<w$}", label, w = label_width),
                    Style::default().fg(Color::Cyan),
                ),
                Span::styled(value.clone(), Style::default().fg(Color::White)),
            ]));
        }
    }

    let height = (lines.len() as u16 + 2).min(area.height.saturating_sub(2));
    let dialog = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    f.render_widget(Clear, dialog);
    let title = if app.db_info.engine == "Parquet" {
        " File details "
    } else {
        " Database details "
    };
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(title);
    let inner = block.inner(dialog);
    f.render_widget(block, dialog);
    f.render_widget(Paragraph::new(lines), inner);
}

fn help_lines(sections: &[(&str, &[(&str, &str)])]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    for (i, (title, items)) in sections.iter().enumerate() {
        if i > 0 {
            lines.push(Line::from(""));
        }
        lines.push(Line::from(Span::styled(
            format!(" {}", title),
            Style::default().fg(Color::Yellow).add_modifier(Modifier::BOLD),
        )));
        for (key, desc) in items.iter() {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:<12}", key), Style::default().fg(Color::Cyan)),
                Span::styled((*desc).to_string(), Style::default().fg(Color::White)),
            ]));
        }
    }
    lines
}

fn draw_help_dialog(f: &mut Frame, area: Rect) {
    let left = help_lines(&[
        (
            "Navigation",
            &[
                ("j/k  \u{2191}\u{2193}", "move selection"),
                ("Tab", "switch panel"),
                ("Enter", "focus the data grid"),
                ("g / G", "first / last row"),
                ("Ctrl+B", "toggle tables panel"),
            ],
        ),
        (
            "Table data",
            &[
                ("h/l  \u{2190}\u{2192}", "move column cursor"),
                ("H/L  \u{21e7}\u{2190}\u{2192}", "pan sideways"),
                ("s", "sort by cursor column"),
                ("Home / End", "first / last column"),
                ("Ctrl+U / D", "half screen up / down"),
                ("PgUp / [", "previous page"),
                ("PgDn / ]", "next page"),
            ],
        ),
    ]);

    let right = help_lines(&[
        (
            "Query",
            &[
                ("/ or :", "open SQL query"),
                ("Enter", "run query"),
                ("j/k  g/G", "move through results"),
                ("+ / -", "grow / restore results"),
                ("Ctrl+U", "clear query and results"),
                ("Esc", "leave query / hide results"),
            ],
        ),
        (
            "Export",
            &[("Ctrl+E", "export table, or query results")],
        ),
        (
            "Mouse",
            &[
                ("wheel", "scroll rows"),
                ("Shift+wheel", "pan sideways"),
                ("swipe \u{21c4}", "pan sideways"),
                ("click", "select row + column"),
            ],
        ),
        (
            "Other",
            &[
                ("i", "database details"),
                ("?", "toggle this help"),
                ("q", "quit"),
            ],
        ),
    ]);

    let content_height = left.len().max(right.len()) as u16 + 2;
    let height = content_height.min(area.height.saturating_sub(2));
    let width = 74u16.min(area.width.saturating_sub(4));
    let dialog = Rect::new(
        area.x + area.width.saturating_sub(width) / 2,
        area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    );
    f.render_widget(Clear, dialog);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan))
        .title(" Keyboard Shortcuts ");
    let inner = block.inner(dialog);
    f.render_widget(block, dialog);

    let cols = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(inner);
    f.render_widget(Paragraph::new(left), cols[0]);
    f.render_widget(Paragraph::new(right), cols[1]);
}
