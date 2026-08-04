use tauri::menu::{Menu, MenuBuilder, MenuItemBuilder, SubmenuBuilder};
use tauri::{AppHandle, Emitter, Runtime};

/// Item ids, matched again in the event handler below.
const OPEN_ID: &str = "file_open";
const EXPORT_ID: &str = "file_export";
const SIDEBAR_ID: &str = "view_sidebar";
const THEME_ID: &str = "view_theme";
const DETAILS_ID: &str = "view_details";
const COLUMNS_ID: &str = "view_columns";
const QUERY_ID: &str = "view_query";
const SHORTCUTS_ID: &str = "help_shortcuts";

/// Events the frontend listens for. Opening a file and writing the CSV are
/// already whole flows in TypeScript, so the menu forwards to them rather than
/// growing a second copy on this side.
const OPEN_EVENT: &str = "menu:open";
const EXPORT_EVENT: &str = "menu:export";
const SIDEBAR_EVENT: &str = "menu:sidebar";
const THEME_EVENT: &str = "menu:theme";
const DETAILS_EVENT: &str = "menu:details";
const COLUMNS_EVENT: &str = "menu:columns";
const QUERY_EVENT: &str = "menu:query";
const SHORTCUTS_EVENT: &str = "menu:shortcuts";

pub fn build<R: Runtime>(app: &AppHandle<R>) -> tauri::Result<Menu<R>> {
    let open = MenuItemBuilder::with_id(OPEN_ID, "Open Database…")
        .accelerator("CmdOrCtrl+O")
        .build(app)?;
    let export = MenuItemBuilder::with_id(EXPORT_ID, "Export CSV…")
        .accelerator("CmdOrCtrl+E")
        .build(app)?;

    let file = SubmenuBuilder::new(app, "File")
        .item(&open)
        .item(&export)
        .separator()
        .close_window()
        .build()?;

    // Setting a menu replaces the default one outright rather than adding to
    // it, so the ordinary submenus have to be rebuilt here. Leaving them out
    // is how an app ships without a working Cmd+Q or Cmd+C.
    let edit = SubmenuBuilder::new(app, "Edit")
        .undo()
        .redo()
        .separator()
        .cut()
        .copy()
        .paste()
        .select_all()
        .build()?;

    // The panels and the theme were reachable only by keys the menus never
    // mentioned. Only the two that already had chorded shortcuts carry
    // accelerators; the rest answer to single keys, which a menu cannot show.
    let view = SubmenuBuilder::new(app, "View")
        .item(
            &MenuItemBuilder::with_id(SIDEBAR_ID, "Toggle Sidebar")
                .accelerator("CmdOrCtrl+B")
                .build(app)?,
        )
        .item(
            &MenuItemBuilder::with_id(THEME_ID, "Next Theme")
                .accelerator("CmdOrCtrl+T")
                .build(app)?,
        )
        .separator()
        .item(&MenuItemBuilder::with_id(DETAILS_ID, "Database Details").build(app)?)
        .item(&MenuItemBuilder::with_id(COLUMNS_ID, "Column Details").build(app)?)
        .item(&MenuItemBuilder::with_id(QUERY_ID, "SQL Query").build(app)?)
        .separator()
        .fullscreen()
        .build()?;

    // Cmd+/ rather than ?, so it does not collide with the ? the webview
    // handles: an accelerator the menu claims never reaches the page.
    let shortcuts = MenuItemBuilder::with_id(SHORTCUTS_ID, "Keyboard Shortcuts")
        .accelerator("CmdOrCtrl+/")
        .build(app)?;
    let help = SubmenuBuilder::new(app, "Help").item(&shortcuts).build()?;

    let window = SubmenuBuilder::new(app, "Window")
        .minimize()
        .maximize()
        .separator()
        // Expected in this menu on macOS, and absent from Tauri's default.
        .bring_all_to_front()
        .close_window()
        .build()?;

    #[cfg(target_os = "macos")]
    {
        // macOS takes the first submenu as the application menu; the other
        // platforms have no such thing and would render it as a stray entry.
        let app_menu = SubmenuBuilder::new(app, "DBiewLite")
            .about(Some(tauri::menu::AboutMetadata::default()))
            .separator()
            .services()
            .separator()
            .hide()
            .hide_others()
            .show_all()
            .separator()
            .quit()
            .build()?;

        // Tail expression rather than an early return: the arm below is
        // compiled away on this platform, leaving this block as the last thing
        // in the function.
        MenuBuilder::new(app)
            .items(&[&app_menu, &file, &edit, &view, &window, &help])
            .build()
    }

    #[cfg(not(target_os = "macos"))]
    MenuBuilder::new(app)
        .items(&[&file, &edit, &view, &window, &help])
        .build()
}

pub fn handle_event<R: Runtime>(app: &AppHandle<R>, id: &str) {
    let event = match id {
        OPEN_ID => OPEN_EVENT,
        EXPORT_ID => EXPORT_EVENT,
        SIDEBAR_ID => SIDEBAR_EVENT,
        THEME_ID => THEME_EVENT,
        DETAILS_ID => DETAILS_EVENT,
        COLUMNS_ID => COLUMNS_EVENT,
        QUERY_ID => QUERY_EVENT,
        SHORTCUTS_ID => SHORTCUTS_EVENT,
        _ => return,
    };
    if let Err(e) = app.emit(event, ()) {
        log::error!("failed to forward menu event {}: {}", id, e);
    }
}
