//! 桌宠窗口的「像素级命中」：人物不透明处才接收鼠标，透明处穿透到桌面。
//! 原理：后台线程轮询全局光标位置，换算成桌宠窗口内的 CSS 逻辑坐标，
//! 查成品形象 PNG 在该点的 alpha 值，动态开/关窗口的 ignore_cursor_events。
//! 不能只靠前端 mousemove：窗口一旦穿透就收不到任何鼠标事件，必须由
//! 不依赖窗口事件的轮询来「唤醒」。

use crate::commands::pet;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Mutex;
use tauri::{AppHandle, Manager, WebviewWindow};

/// 前端上报的形象显示区（相对桌宠窗口视口的 CSS 逻辑坐标）。
#[derive(Debug, Clone, Copy)]
pub struct HitRegion {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

/// 成品 PNG 的 alpha 缓存，按「路径 + 修改时间」失效。
#[derive(Debug, Clone)]
struct SpriteAlpha {
    path: PathBuf,
    mtime: u64,
    width: u32,
    height: u32,
    alpha: Vec<u8>,
}

#[derive(Default)]
pub struct PetHitState {
    region: Mutex<Option<HitRegion>>,
    sprite: Mutex<Option<SpriteAlpha>>,
    last_ignore: Mutex<bool>,
    /// 鼠标按下期间冻结穿透状态，避免拖动中人物滑到透明处导致拖动中断。
    pressed: AtomicBool,
}

/// 前端在形象元素布局/换图后上报显示区。
#[tauri::command]
pub fn set_pet_hit_region(app: AppHandle, x: f64, y: f64, width: f64, height: f64) {
    let Some(state) = app.try_state::<PetHitState>() else {
        return;
    };
    let mut region = lock(&state.region);
    *region = Some(HitRegion {
        x,
        y,
        width,
        height,
    });
}

/// 鼠标按下/抬起（拖动保护）。
#[tauri::command]
pub fn set_pet_hit_pressed(app: AppHandle, pressed: bool) {
    let Some(state) = app.try_state::<PetHitState>() else {
        return;
    };
    state.pressed.store(pressed, Ordering::Relaxed);
}

pub fn spawn_hit_scheduler(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(std::time::Duration::from_millis(100));
        if let Err(e) = tick(&app) {
            eprintln!("桌宠命中检测失败: {e}");
        }
    });
}

fn tick(app: &AppHandle) -> Result<(), String> {
    let Some(window) = app.get_webview_window("pet") else {
        return Ok(());
    };
    if !window.is_visible().unwrap_or(false) {
        return Ok(());
    }
    let Some(state) = app.try_state::<PetHitState>() else {
        return Ok(());
    };
    let config = pet::get_pet_photo_impl(app);
    let using_photo = config.enabled && config.path.is_some();

    let want_ignore = if !using_photo {
        false // 默认小猫保持整个窗口可点
    } else {
        hit_test(&window, &config, &state)
    };

    let mut last = lock(&state.last_ignore);
    if *last != want_ignore {
        window
            .set_ignore_cursor_events(want_ignore)
            .map_err(|e| format!("设置鼠标穿透失败: {e}"))?;
        *last = want_ignore;
    }
    Ok(())
}

/// true = 光标处透明 → 应穿透。
fn hit_test(window: &WebviewWindow, config: &pet::PetPhotoConfig, state: &PetHitState) -> bool {
    let Some(region) = *lock(&state.region) else {
        return false; // 前端还没上报：宁可先不穿透
    };
    let Ok(cursor) = window.app_handle().cursor_position() else {
        return false;
    };
    let Ok(pos) = window.outer_position() else {
        return false;
    };
    let Ok(scale) = window.scale_factor() else {
        return false;
    };
    let lx = (cursor.x - pos.x as f64) / scale;
    let ly = (cursor.y - pos.y as f64) / scale;
    if lx < region.x || ly < region.y || lx >= region.x + region.width || ly >= region.y + region.height
    {
        return true; // 形象区域外
    }

    let Some(path) = config.path.as_deref() else {
        return false;
    };
    let Some(sprite) = sprite_alpha(state, path) else {
        return false;
    };

    // object-contain：形象按比例完整放入显示区并居中
    let fit = (region.width / sprite.width as f64).min(region.height / sprite.height as f64);
    let dw = sprite.width as f64 * fit;
    let dh = sprite.height as f64 * fit;
    let dx = region.x + (region.width - dw) / 2.0;
    let dy = region.y + (region.height - dh) / 2.0;
    if lx < dx || ly < dy || lx >= dx + dw || ly >= dy + dh {
        return true;
    }
    let u = (((lx - dx) / dw) * sprite.width as f64) as u32;
    let v = (((ly - dy) / dh) * sprite.height as f64) as u32;
    let alpha = sprite.alpha[(v * sprite.width + u) as usize];
    alpha < 8
}

fn sprite_alpha(state: &PetHitState, path: &str) -> Option<SpriteAlpha> {
    let mtime = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let mut cached = lock(&state.sprite);
    if let Some(s) = cached.as_ref() {
        if s.path == PathBuf::from(path) && s.mtime == mtime {
            return Some(s.clone());
        }
    }
    let img = image::open(path).ok()?.to_rgba8();
    let (w, h) = img.dimensions();
    let alpha = img
        .as_raw()
        .iter()
        .skip(3)
        .step_by(4)
        .copied()
        .collect();
    let s = SpriteAlpha {
        path: PathBuf::from(path),
        mtime,
        width: w,
        height: h,
        alpha,
    };
    *cached = Some(s.clone());
    Some(s)
}

/// 锁中毒时不让轮询线程挂掉（前一个持锁线程 panic 过也应继续工作）。
fn lock<T>(mutex: &Mutex<T>) -> std::sync::MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|e| e.into_inner())
}
