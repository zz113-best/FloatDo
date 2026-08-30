use super::DbState;
use crate::database::settings_repo;
use serde_json::json;
use tauri::{AppHandle, Emitter, Manager, State};

/// 主面板被要求打开某个页签时推给它（payload: {"tab": "settings" | "stats"}）。
pub const EVENT_PANEL_OPEN: &str = "panel://open";

#[tauri::command]
pub fn get_setting(db: State<DbState>, key: String) -> Result<Option<String>, String> {
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    settings_repo::get(&conn, &key)
}

#[tauri::command]
pub fn set_setting(db: State<DbState>, key: String, value: String) -> Result<(), String> {
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    settings_repo::set(&conn, &key, &value)
}

/// 打开主面板窗口（tauri.conf.json 静态定义、启动时隐藏）并切到指定页签。
/// 设置 / 统计共用这一个窗口，左侧导航切换页签，后续新增区块直接加页签。
pub fn open_panel(app: &AppHandle, tab: &str) -> Result<(), String> {
    let window = app
        .get_webview_window("settings")
        .ok_or_else(|| "主面板窗口未初始化".to_string())?;
    let _ = window.center();
    let _ = window.show();
    let _ = window.set_focus();
    let _ = app.emit_to("settings", EVENT_PANEL_OPEN, json!({ "tab": tab }));
    Ok(())
}

#[tauri::command]
pub fn open_settings(app: AppHandle) -> Result<(), String> {
    open_panel(&app, "settings")
}
