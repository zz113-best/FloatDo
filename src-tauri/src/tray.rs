use crate::commands::pet::{set_pet_visible_impl, is_pet_enabled};
use crate::commands::settings::open_panel;
use tauri::{
    AppHandle, Emitter, Manager,
    menu::{ContextMenu, Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
};

pub const EVENT_EXPAND_ADD: &str = "widget://expand-add";

pub fn create_tray(app: &AppHandle) -> Result<(), String> {
    let open_main = MenuItem::with_id(app, "open_main", "打开主界面", true, None::<&str>)
        .map_err(|e| format!("创建托盘菜单失败: {e}"))?;
    let toggle = MenuItem::with_id(app, "toggle", "显示 / 隐藏 Todo", true, None::<&str>)
        .map_err(|e| format!("创建托盘菜单失败: {e}"))?;
    let settings = MenuItem::with_id(app, "settings", "设置", true, None::<&str>)
        .map_err(|e| format!("创建托盘菜单失败: {e}"))?;
    let toggle_pet = MenuItem::with_id(app, "toggle_pet", "显示 / 隐藏 桌宠", true, None::<&str>)
        .map_err(|e| format!("创建托盘菜单失败: {e}"))?;
    let quit = MenuItem::with_id(app, "quit", "退出", true, None::<&str>)
        .map_err(|e| format!("创建托盘菜单失败: {e}"))?;
    let sep1 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let sep2 = PredefinedMenuItem::separator(app).map_err(|e| e.to_string())?;
    let menu = Menu::with_items(
        app,
        &[
            &open_main, &toggle, &toggle_pet, &sep1, &settings, &sep2, &quit,
        ],
    )
        .map_err(|e| format!("创建托盘菜单失败: {e}"))?;

    let icon = app
        .default_window_icon()
        .cloned()
        .ok_or_else(|| "缺少应用图标".to_string())?;

    // 不把菜单挂到托盘（内置的右键弹出在部分 Windows 环境不显示，tauri 已知问题），
    // 改为在 on_tray_icon_event 的右键事件里手动弹出。
    let popup_menu = menu.clone();
    TrayIconBuilder::with_id("floatdo-tray")
        .icon(icon)
        .tooltip("FloatDo")
        .on_menu_event(|app, event| match event.id.as_ref() {
            // 主界面默认落在统计页
            "open_main" => {
                if let Err(e) = open_panel(app, "stats") {
                    eprintln!("{e}");
                }
            }
            "toggle" => toggle_widget(app),
            "settings" => {
                if let Err(e) = open_panel(app, "settings") {
                    eprintln!("{e}");
                }
            }
            "toggle_pet" => {
                if let Err(e) = set_pet_visible_impl(app, !is_pet_enabled(app)) {
                    eprintln!("{e}");
                }
            }
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(move |tray, event| match event {
            TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } => {
                toggle_widget(tray.app_handle());
            }
            TrayIconEvent::Click {
                button: MouseButton::Right,
                button_state: MouseButtonState::Up,
                ..
            } => {
                // popup 在光标位置弹出；随便指定一个窗口只为拿到 hwnd
                if let Some(window) = tray
                    .app_handle()
                    .get_webview_window("widget")
                    .map(|w| w.as_ref().window())
                {
                    if let Err(e) = popup_menu.popup(window) {
                        eprintln!("弹出托盘菜单失败: {e}");
                    }
                }
            }
            _ => {}
        })
        .build(app)
        .map_err(|e| format!("创建托盘失败: {e}"))?;
    Ok(())
}

pub fn toggle_widget(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("widget") {
        let visible = window.is_visible().unwrap_or(false);
        if visible {
            let _ = window.hide();
        } else {
            let _ = window.show();
            let _ = window.set_focus();
        }
    }
}

/// 显示悬浮窗并打开添加表单（托盘菜单 / 全局快捷键共用）。
pub fn show_widget_and_expand(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("widget") {
        let _ = window.show();
        let _ = window.set_focus();
        let _ = app.emit_to("widget", EVENT_EXPAND_ADD, ());
    }
}
