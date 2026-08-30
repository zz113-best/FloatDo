//! 悬浮窗/桌宠窗口的位置持久化：拖动后落盘到 settings 表，重启时恢复。
//! 拖动过程中 Moved 事件非常密集，不能每条都写库——这里只做「脏标记」，
//! 由后台线程每 0.8 秒统一落盘一次。
//! 保存的是「左边缘 x + 底边 y」：悬浮窗展开时向上生长（底边不动），
//! 记底边才能在重启后（以收起高度）还原到同一位置。

use crate::commands::DbState;
use crate::database::settings_repo;
use std::collections::HashSet;
use std::sync::Mutex;
use tauri::{AppHandle, Manager, PhysicalPosition};

const WIDGET_POS_KEY: &str = "posWidget";
const PET_POS_KEY: &str = "posPet";

#[derive(Default)]
pub struct PositionState {
    dirty: Mutex<HashSet<&'static str>>,
}

fn key_for(label: &str) -> Option<&'static str> {
    match label {
        "widget" => Some(WIDGET_POS_KEY),
        "pet" => Some(PET_POS_KEY),
        _ => None,
    }
}

fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}

/// 窗口移动了（on_window_event 里调用），后台线程稍后落盘。
pub fn mark_dirty(app: &AppHandle, label: &str) {
    let Some(key) = key_for(label) else {
        return;
    };
    let Some(state) = app.try_state::<PositionState>() else {
        return;
    };
    lock(&state.dirty).insert(key);
}

/// 立即保存窗口当前位置（存 x + 底边）。
fn save_now(app: &AppHandle, label: &str) {
    let Some(key) = key_for(label) else {
        return;
    };
    let Some(window) = app.get_webview_window(label) else {
        return;
    };
    let (Ok(pos), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return;
    };
    let bottom = pos.y + size.height as i32;
    if let Some(db) = app.try_state::<DbState>() {
        if let Ok(conn) = db.0.lock() {
            let _ = settings_repo::set(&conn, key, &format!("{},{}", pos.x, bottom));
        }
    }
}

/// 后台落盘线程。
pub fn spawn_saver(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(800));
        let dirty: Vec<&'static str> = {
            let Some(state) = app.try_state::<PositionState>() else {
                continue;
            };
            let mut guard = lock(&state.dirty);
            guard.drain().collect()
        };
        for key in dirty {
            let label = if key == WIDGET_POS_KEY { "widget" } else { "pet" };
            save_now(&app, label);
        }
    });
}

/// 启动时恢复窗口位置；没有存过（或位置已不在屏幕附近）就执行默认摆放并记住。
pub fn restore_or_default(app: &AppHandle, label: &str, default: impl FnOnce()) {
    let Some(key) = key_for(label) else {
        return;
    };
    let saved = read_pos(app, key);
    if let Some((x, bottom)) = saved {
        if let Some(window) = app.get_webview_window(label) {
            if let Ok(size) = window.outer_size() {
                if position_on_screen(&window, x, bottom - size.height as i32, size) {
                    let _ = window.set_position(PhysicalPosition::new(
                        x,
                        bottom - size.height as i32,
                    ));
                    return;
                }
            }
        }
    }
    default();
    save_now(app, label);
}

fn read_pos(app: &AppHandle, key: &str) -> Option<(i32, i32)> {
    let db = app.try_state::<DbState>()?;
    let conn = db.0.lock().ok()?;
    let raw = settings_repo::get(&conn, key).ok().flatten()?;
    let mut parts = raw.split(',');
    let x = parts.next()?.parse().ok()?;
    let bottom = parts.next()?.parse().ok()?;
    Some((x, bottom))
}

/// 位置合理性：恢复后窗口主体还落在当前显示器内（防止换分辨率/拔掉副屏后窗口消失）。
fn position_on_screen(
    window: &tauri::WebviewWindow,
    x: i32,
    y: i32,
    size: tauri::PhysicalSize<u32>,
) -> bool {
    let w = size.width as i32;
    let h = size.height as i32;
    let Some(monitor) = window.current_monitor().ok().flatten() else {
        return true; // 拿不到显示器信息时宁可相信旧位置
    };
    let mon = monitor.position();
    let mon_size = monitor.size();
    let right = x + w;
    let bottom = y + h;
    // 至少 1/3 与显示器相交，且不允许整块飘到屏幕外
    right > mon.x + w / 3
        && x < mon.x + mon_size.width as i32 - w / 3
        && bottom > mon.y + h / 3
        && y < mon.y + mon_size.height as i32 - h / 3
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_for_maps_known_windows_only() {
        assert_eq!(key_for("widget"), Some(WIDGET_POS_KEY));
        assert_eq!(key_for("pet"), Some(PET_POS_KEY));
        assert_eq!(key_for("settings"), None);
    }
}
