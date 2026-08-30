//! 全局快捷键：系统级热键，任何应用在前台都能触发。
//! 组合键字符串存在 settings 表（如 "Ctrl+Alt+F"，空串 = 未启用），
//! 配置变化时整体重注册。可用动作见 ACTIONS。

use crate::commands::pet::{is_pet_enabled, set_pet_visible_impl};
use crate::commands::DbState;
use crate::database::settings_repo;
use serde::Serialize;
use std::str::FromStr;
use tauri::{AppHandle, Manager};
use tauri_plugin_global_shortcut::{GlobalShortcutExt, Shortcut, ShortcutState};

/// 一个可配置的快捷键动作定义。
pub struct ShortcutAction {
    /// 动作标识（settings key 同时用它加前缀存储）
    pub id: &'static str,
    /// settings 表里的键名
    pub key: &'static str,
    /// 默认组合键（空串 = 默认不启用）
    pub default: &'static str,
}

pub const ACTIONS: &[ShortcutAction] = &[
    ShortcutAction { id: "toggle_widget", key: "shortcutToggleWidget", default: "Ctrl+Alt+Space" },
    ShortcutAction { id: "quick_add", key: "shortcutQuickAdd", default: "Ctrl+Alt+N" },
    ShortcutAction { id: "toggle_focus", key: "shortcutToggleFocus", default: "Ctrl+Alt+F" },
    ShortcutAction { id: "toggle_pet", key: "shortcutTogglePet", default: "" },
];

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutConfig {
    pub action: &'static str,
    pub label: &'static str,
    pub value: String,
    pub default: &'static str,
}

impl ShortcutAction {
    pub fn label(&self) -> &'static str {
        match self.id {
            "toggle_widget" => "显示 / 隐藏悬浮窗",
            "quick_add" => "快速添加任务",
            "toggle_focus" => "开始 / 停止专注",
            "toggle_pet" => "显示 / 隐藏桌宠",
            _ => "未知动作",
        }
    }
}

fn read_combo(conn: &rusqlite::Connection, action: &ShortcutAction) -> String {
    settings_repo::get(conn, action.key)
        .ok()
        .flatten()
        .unwrap_or_else(|| action.default.to_string())
}

/// 组合键是否合法（能被插件解析）。空串表示未启用，也算合法。
/// 插件解析不区分大小写，存储统一用 "Ctrl+Alt+F" 这种展示友好格式。
pub fn combo_valid(combo: &str) -> bool {
    if combo.is_empty() {
        return true;
    }
    Shortcut::from_str(combo).is_ok()
}

/// setup 时调用：按当前配置注册全部快捷键。单个失败只记日志不阻断启动。
pub fn init(app: &AppHandle) {
    if let Err(e) = apply(app) {
        eprintln!("注册全局快捷键失败: {e}");
    }
}

/// 按数据库配置重注册全部快捷键（先全部注销再注册，避免自冲突）。
fn apply(app: &AppHandle) -> Result<(), String> {
    let manager = app.global_shortcut();
    let _ = manager.unregister_all();

    let db = app
        .try_state::<DbState>()
        .ok_or_else(|| "数据库初始化中".to_string())?;
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    let combos: Vec<(&str, String)> = ACTIONS
        .iter()
        .map(|a| (a.id, read_combo(&conn, a)))
        .collect();
    drop(conn);

    for (action, combo) in combos {
        if combo.is_empty() {
            continue;
        }
        let action = action.to_string();
        let result = manager.on_shortcut(combo.as_str(), move |app, _shortcut, event| {
            if matches!(event.state, ShortcutState::Pressed) {
                dispatch(app, &action);
            }
        });
        if let Err(e) = result {
            return Err(format!("注册 {combo} 失败: {e}"));
        }
    }
    Ok(())
}

/// 快捷键触发后的动作分发。
fn dispatch(app: &AppHandle, action: &str) {
    match action {
        "toggle_widget" => crate::tray::toggle_widget(app),
        "quick_add" => crate::tray::show_widget_and_expand(app),
        "toggle_focus" => toggle_focus(app),
        "toggle_pet" => {
            if let Err(e) = set_pet_visible_impl(app, !is_pet_enabled(app)) {
                eprintln!("快捷键切换桌宠失败: {e}");
            }
        }
        _ => {}
    }
}

/// 专注开着就停，没开就按设置时长开一轮。
fn toggle_focus(app: &AppHandle) {
    let running = match app.try_state::<crate::commands::focus::FocusRuntime>() {
        Some(runtime) => runtime.0.lock().map(|g| g.is_some()).unwrap_or(false),
        None => false,
    };
    let result = if running {
        crate::commands::focus::stop_focus(app.clone())
    } else {
        crate::commands::focus::start_focus(app.clone(), None, None)
    };
    if let Err(e) = result {
        eprintln!("快捷键切换专注失败: {e}");
    }
}

/// 读全部快捷键配置（前端渲染设置页用）。
#[tauri::command]
pub fn get_shortcut_config(app: AppHandle) -> Result<Vec<ShortcutConfig>, String> {
    let db = app
        .try_state::<DbState>()
        .ok_or_else(|| "数据库初始化中".to_string())?;
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    Ok(ACTIONS
        .iter()
        .map(|a| ShortcutConfig {
            action: a.id,
            label: a.label(),
            value: read_combo(&conn, a),
            default: a.default,
        })
        .collect())
}

/// 设置某个动作的组合键（空串 = 停用）。先探测能否注册（防与其他应用冲突），
/// 成功才落库并整体重注册。
#[tauri::command]
pub fn set_shortcut(app: AppHandle, action: String, value: String) -> Result<(), String> {
    let found = ACTIONS.iter().find(|a| a.id == action);
    let Some(action_def) = found else {
        return Err(format!("未知快捷键动作: {action}"));
    };
    let value = value.trim().to_string();
    if !combo_valid(&value) {
        return Err(format!("无法识别的组合键: {value}"));
    }

    if !value.is_empty() {
        // 先探测注册（冲突时立刻报错给前端），探测到的注册会在 apply 里被注销重挂
        let manager = app.global_shortcut();
        manager
            .register(value.as_str())
            .map_err(|e| format!("{value} 已被其他程序占用: {e}"))?;
    }

    {
        let db = app
            .try_state::<DbState>()
            .ok_or_else(|| "数据库初始化中".to_string())?;
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        settings_repo::set(&conn, action_def.key, &value)?;
    }
    apply(&app)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn combo_valid_accepts_known_formats() {
        assert!(combo_valid(""));
        assert!(combo_valid("Ctrl+Alt+F"));
        assert!(combo_valid("Ctrl+Alt+Space"));
        assert!(combo_valid("Ctrl+Shift+F1"));
        assert!(combo_valid("Alt+Up"));
    }

    #[test]
    fn combo_valid_rejects_garbage() {
        assert!(!combo_valid("Ctrl"));
        assert!(!combo_valid("Ctrl+Snap"));
        assert!(!combo_valid("hello"));
    }
}
