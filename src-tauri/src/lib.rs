mod commands;
pub mod database;
pub mod pet_avatar;
pub mod pet_hit;
pub mod pet_segment;
pub mod reminder;
mod shortcuts;
mod tray;
pub mod window_pos;

use commands::{ai, categories, focus, pet, settings, stats, tasks, DbState};
use std::sync::Mutex;
use tauri::{Manager, PhysicalPosition, WindowEvent};

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // 单实例必须最先注册：重复启动时把已有实例的悬浮窗带到前台后退出自己
        .plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
            if let Some(widget) = app.get_webview_window("widget") {
                let _ = widget.show();
                let _ = widget.set_focus();
            }
        }))
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        // 全局快捷键：注册/处理全在 Rust 侧（shortcuts 模块），前端只负责配置界面
        .plugin(tauri_plugin_global_shortcut::Builder::new().build())
        // 专注模式运行时必须在窗口创建前就挂上：静态窗口先于 setup 启动加载，
        // 前端第一时间就会调 get_focus_state，晚了会 state() panic
        .manage(focus::FocusRuntime(Mutex::new(None)))
        // 桌宠像素级命中（人物不透明处可点、透明处穿透桌面）
        .manage(pet_hit::PetHitState::default())
        // 悬浮窗/桌宠位置持久化
        .manage(window_pos::PositionState::default())
        // 照片桌宠的本地资源协议：/photo → 抠图后的成品形象，/source → 用户上传的原始照片，
        // /frame?i=N → 多帧动画的第 N 帧（0 = 主形象）。只在本机响应。
        .register_uri_scheme_protocol("petphoto", |ctx, request| {
            let app = ctx.app_handle();
            let photo = pet::get_pet_photo_impl(app);
            let uri = request.uri();
            let frame_index = if uri.path().ends_with("/frame") {
                uri.query().and_then(|q| {
                    q.split('&')
                        .find_map(|kv| kv.strip_prefix("i="))
                        .and_then(|v| v.parse::<usize>().ok())
                })
            } else {
                None
            };
            let path = if let Some(i) = frame_index {
                if i == 0 {
                    photo.path
                } else {
                    let sprite: Option<String> = app.try_state::<DbState>().and_then(|db| {
                        let conn = db.0.lock().ok()?;
                        pet::read_frames(&conn)
                            .into_iter()
                            .nth(i - 1)
                            .map(|f| f.sprite)
                    });
                    sprite.filter(|p| std::path::Path::new(p).is_file())
                }
            } else if uri.path().ends_with("/source") {
                photo.source_path
            } else {
                photo.path
            };
            let served = path.and_then(|p| {
                let mime = pet::mime_for_ext(std::path::Path::new(&p));
                std::fs::read(&p).ok().map(|bytes| (mime, bytes))
            });
            match served {
                Some((mime, bytes)) => tauri::http::Response::builder()
                    .header("Content-Type", mime)
                    .header("Cache-Control", "no-store")
                    .body(std::borrow::Cow::Owned(bytes)),
                None => tauri::http::Response::builder()
                    .status(404)
                    .body(std::borrow::Cow::Owned(b"photo not found".to_vec())),
            }
            .expect("构建 petphoto 协议响应失败")
        })
        .setup(|app| {
            let data_dir = app
                .path()
                .app_data_dir()
                .map_err(|e| format!("无法确定数据目录: {e}"))?;
            let conn = database::init(&data_dir)?;
            app.manage(DbState(Mutex::new(conn)));

            tray::create_tray(app.handle())
                .map_err(|e| std::io::Error::other(e.to_string()))?;

            // 窗口位置：优先恢复用户上次拖放的位置，没存过才用默认摆放
            window_pos::restore_or_default(app.handle(), "widget", || {
                position_widget_bottom_right(app.handle());
            });
            window_pos::restore_or_default(app.handle(), "pet", || {
                position_pet_next_to_widget(app.handle());
            });
            // 移动后的位置由后台线程统一落盘
            window_pos::spawn_saver(app.handle().clone());
            pet::restore_pet_visibility(app.handle());

            // 专注模式：恢复上次遗留的进行中会话，再启动秒级调度线程
            focus::init(app.handle())?;
            focus::spawn_scheduler(app.handle().clone());

            // 全局快捷键：按 settings 表配置注册
            shortcuts::init(app.handle());

            // 到期提醒后台调度：推送到桌宠窗口弹气泡
            reminder::spawn_scheduler(app.handle().clone());

            // 桌宠像素级命中：轮询光标位置，透明处穿透到桌面
            pet_hit::spawn_hit_scheduler(app.handle().clone());
            Ok(())
        })
        .on_window_event(|window, event| {
            // 拖动/移动窗口 → 标记位置待落盘
            if let WindowEvent::Moved(_) = event {
                window_pos::mark_dirty(window.app_handle(), window.label());
            }
            // 悬浮窗、设置窗口、桌宠都没有「真正关闭」的概念：
            // 点关闭一律隐藏，应用退出只走托盘菜单。
            if matches!(window.label(), "widget" | "settings" | "pet") {
                if let WindowEvent::CloseRequested { api, .. } = event {
                    api.prevent_close();
                    let _ = window.hide();
                }
            }
        })
        .invoke_handler(tauri::generate_handler![
            tasks::get_tasks,
            tasks::create_task,
            tasks::update_task,
            tasks::delete_task,
            tasks::search_tasks,
            tasks::open_tasks,
            tasks::reorder_tasks,
            tasks::export_tasks_csv,
            categories::get_categories,
            categories::create_category,
            settings::get_setting,
            settings::set_setting,
            settings::open_settings,
            stats::get_stats,
            stats::open_stats,
            shortcuts::get_shortcut_config,
            shortcuts::set_shortcut,
            ai::get_ai_config,
            ai::set_ai_config,
            ai::test_ai,
            ai::ai_chat,
            ai::open_chat,
            pet::set_pet_visible,
            pet::is_pet_enabled_command,
            pet::get_pet_photo,
            pet::pick_pet_photo,
            pet::set_pet_photo_enabled,
            pet::reprocess_pet_photo,
            pet::get_pet_personality,
            pet::set_pet_personality,
            pet::open_pet_center,
            pet::add_pet_frame,
            pet::remove_pet_frame,
            pet::set_pet_frame_speed,
            pet::set_pet_display,
            pet_hit::set_pet_hit_region,
            pet_hit::set_pet_hit_pressed,
            focus::get_focus_state,
            focus::start_focus,
            focus::stop_focus,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// 启动时把悬浮窗放到主屏右下角。
fn position_widget_bottom_right(app: &tauri::AppHandle) {
    let Some(window) = app.get_webview_window("widget") else {
        return;
    };
    let Ok(Some(monitor)) = window.current_monitor() else {
        return;
    };
    let Ok(size) = window.outer_size() else {
        return;
    };
    let scale = monitor.scale_factor();
    let margin = (16.0 * scale) as i32;
    let monitor_size = monitor.size();
    let x = monitor_size.width as i32 - size.width as i32 - margin;
    let y = monitor_size.height as i32 - size.height as i32 - margin;
    let _ = window.set_position(PhysicalPosition::new(x.max(0), y.max(0)));
}

/// 桌宠默认放在悬浮窗左侧、底边对齐；用户可拖到任意位置（本次会话内）。
fn position_pet_next_to_widget(app: &tauri::AppHandle) {
    let (Some(pet), Some(widget)) = (
        app.get_webview_window("pet"),
        app.get_webview_window("widget"),
    ) else {
        return;
    };
    let (Ok(widget_pos), Ok(widget_size), Ok(pet_size)) = (
        widget.outer_position(),
        widget.outer_size(),
        pet.outer_size(),
    ) else {
        return;
    };
    let scale = widget
        .scale_factor()
        .ok()
        .unwrap_or(1.0);
    let gap = (12.0 * scale) as i32;
    let x = widget_pos.x - pet_size.width as i32 - gap;
    // 底边与悬浮窗对齐
    let y = widget_pos.y + widget_size.height as i32 - pet_size.height as i32;
    let _ = pet.set_position(PhysicalPosition::new(x.max(0), y.max(0)));
}
