use super::DbState;
use crate::database::settings_repo;
use crate::pet_avatar::{self, PetStyle};
use crate::pet_segment;
use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Emitter, Manager};

pub const PET_ENABLED_KEY: &str = "petEnabled";
pub const PET_PHOTO_PATH_KEY: &str = "petPhotoPath";
pub const PET_PHOTO_ENABLED_KEY: &str = "petPhotoEnabled";
pub const PET_PHOTO_CHANGED_EVENT: &str = "pet://photo-changed";
/// 桌宠照片的「源图」（用户上传的原始照片）；PET_PHOTO_PATH_KEY 指向抠图+风格处理后的成品。
pub const PET_PHOTO_SOURCE_PATH_KEY: &str = "petPhotoSourcePath";
pub const PET_STYLE_KEY: &str = "petStyle";
pub const PET_TOLERANCE_KEY: &str = "petTolerance";
/// 桌宠人格（气泡文案 + AI 系统提示词共用）。
pub const PET_PERSONALITY_KEY: &str = "petPersonality";
pub const PET_PERSONALITY_CHANGED_EVENT: &str = "pet://personality-changed";
/// 抠图方式："true" = AI 人像分割（默认），"false" = 几何快速模式。
pub const PET_SEGMENTATION_KEY: &str = "petSegmentation";
/// 多帧动画的附加帧（JSON Vec<FrameEntry>，不含主形象）。
pub const PET_FRAMES_KEY: &str = "petFrames";
/// 多帧轮播间隔（毫秒）。
pub const PET_FRAME_MS_KEY: &str = "petFrameMs";
/// 桌宠显示边长（px，逻辑像素）。
pub const PET_SIZE_KEY: &str = "petSize";
/// 桌宠不透明度（%，20~100）。
pub const PET_OPACITY_KEY: &str = "petOpacity";
/// 源图文件名前缀；成品固定为 pet_photo.png（处理输出）。
const PET_SOURCE_FILE_STEM: &str = "pet_source";
const PET_SPRITE_FILE_NAME: &str = "pet_photo.png";

/// 默认抠图容差（相邻像素最大通道差 ≤ 容差视为同一片背景）。
pub const DEFAULT_TOLERANCE: u8 = 30;

const PET_PHOTO_SCHEME_HOST: &str = "http://petphoto.localhost";

/// 显示参数的合法范围。
pub const PET_SIZE_MIN: u32 = 64;
pub const PET_SIZE_MAX: u32 = 192;
pub const PET_SIZE_DEFAULT: u32 = 96;
pub const PET_OPACITY_DEFAULT: u8 = 100;
pub const PET_FRAME_MS_DEFAULT: u64 = 300;

/// 多帧动画的一帧：source 用于换风格时重处理，sprite 是抠图后的成品。
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FrameEntry {
    pub source: String,
    pub sprite: String,
}

fn clamp_u32(v: u32, min: u32, max: u32, fallback: u32) -> u32 {
    if v == 0 {
        return fallback;
    }
    v.clamp(min, max)
}

/// 桌宠人格：决定气泡文案语气与 AI 助手人设。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetPersonality {
    /// 温柔型
    Gentle,
    /// 激励型
    Motivator,
    /// 毒舌型
    Sarcastic,
    /// 高冷型
    Cool,
}

impl PetPersonality {
    pub fn as_str(&self) -> &'static str {
        match self {
            PetPersonality::Gentle => "gentle",
            PetPersonality::Motivator => "motivator",
            PetPersonality::Sarcastic => "sarcastic",
            PetPersonality::Cool => "cool",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "gentle" => Ok(PetPersonality::Gentle),
            "motivator" => Ok(PetPersonality::Motivator),
            "sarcastic" => Ok(PetPersonality::Sarcastic),
            "cool" => Ok(PetPersonality::Cool),
            other => Err(format!("未知的桌宠人格: {other}")),
        }
    }

    /// 从 settings 表读人格，非法/缺省回落温柔型。
    pub fn read_from(conn: &rusqlite::Connection) -> Self {
        settings_repo::get(conn, PET_PERSONALITY_KEY)
            .ok()
            .flatten()
            .and_then(|v| PetPersonality::from_db(&v).ok())
            .unwrap_or(PetPersonality::Gentle)
    }
}

#[tauri::command]
pub fn set_pet_visible(app: AppHandle, visible: bool) -> Result<(), String> {
    set_pet_visible_impl(&app, visible)
}

/// 显示/隐藏桌宠并持久化开关。托盘菜单与主面板共用。
pub fn set_pet_visible_impl(app: &AppHandle, visible: bool) -> Result<(), String> {
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        settings_repo::set(&conn, PET_ENABLED_KEY, if visible { "true" } else { "false" })?;
    }
    if let Some(window) = app.get_webview_window("pet") {
        if visible {
            window.show().map_err(|e| format!("显示桌宠失败: {e}"))?;
        } else {
            window.hide().map_err(|e| format!("隐藏桌宠失败: {e}"))?;
        }
    }
    Ok(())
}

/// 当前桌宠开关（settings 表，缺省视为开启）。
pub fn is_pet_enabled(app: &AppHandle) -> bool {
    let Some(db) = app.try_state::<DbState>() else {
        return true;
    };
    let Ok(conn) = db.0.lock() else {
        return true;
    };
    settings_repo::get(&conn, PET_ENABLED_KEY)
        .ok()
        .flatten()
        .map(|v| v != "false")
        .unwrap_or(true)
}

/// 应用启动时根据设置恢复桌宠可见性（关掉过的用户重启后不应该再看到桌宠）。
pub fn restore_pet_visibility(app: &AppHandle) {
    if !is_pet_enabled(app) {
        if let Some(window) = app.get_webview_window("pet") {
            let _ = window.hide();
        }
    }
}

#[tauri::command]
pub fn is_pet_enabled_command(app: AppHandle) -> bool {
    is_pet_enabled(&app)
}

/// 打开桌宠中心：主面板（settings 窗口）切到 pet 页签。
#[tauri::command]
pub fn open_pet_center(app: AppHandle) -> Result<(), String> {
    super::settings::open_panel(&app, "pet")
}

// ---------- 照片桌宠（阶段 3 上传 + 阶段 8 抠图/风格） ----------

/// 照片桌宠配置：pet 窗口与桌宠中心共用。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetPhotoConfig {
    /// 抠图+风格处理后的桌宠形象路径（未导入过则为 null）
    pub path: Option<String>,
    /// 是否正在使用照片桌宠（false = 默认小猫）
    pub enabled: bool,
    /// 成品 <img src> 地址（petphoto:// 协议）
    pub url: Option<String>,
    /// 用户上传的原始照片路径
    pub source_path: Option<String>,
    /// 原始照片 <img src> 地址（预览/对照用）
    pub source_url: Option<String>,
    /// 当前视觉风格
    pub style: String,
    /// 当前抠图容差
    pub tolerance: u8,
    /// 是否使用 AI 人像分割（false = 几何快速模式）
    pub use_ai: bool,
    /// 多帧动画的所有帧地址（第 0 帧即主形象；单帧 = 静态 + 呼吸/眨眼微动画）
    pub frames: Vec<String>,
    /// 多帧轮播间隔（毫秒）
    pub frame_ms: u64,
    /// 显示边长（px）
    pub pet_size: u32,
    /// 不透明度（%）
    pub pet_opacity: u8,
}

fn read_kv(app: &AppHandle, key: &str) -> Option<String> {
    // try_state：热重启时前端可能抢在 setup 完成前发来照片协议请求，
    // 直接 state() 会 panic；未就绪时按「无配置」处理，前端稍后会重试
    let db = app.try_state::<DbState>()?;
    let conn = db.0.lock().ok()?;
    settings_repo::get(&conn, key).ok().flatten()
}

/// 根据扩展名猜 MIME（照片协议响应头用）。
pub fn mime_for_ext(path: &Path) -> &'static str {
    match path
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .as_deref()
    {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("webp") => "image/webp",
        Some("gif") => "image/gif",
        _ => "application/octet-stream",
    }
}

/// 协议地址带文件修改时间做缓存穿透：换照片后 <img> 立即刷新。
/// kind 是 "photo"（成品）、"source"（原始照片）或 "frame"（附加帧，带 i=序号）。
fn photo_url(path: &str, kind: &str) -> Option<String> {
    let version = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some(format!("{PET_PHOTO_SCHEME_HOST}/{kind}?v={version}"))
}

fn frame_url(path: &str, index: usize) -> Option<String> {
    let version = std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_secs())
        .unwrap_or(0);
    Some(format!("{PET_PHOTO_SCHEME_HOST}/frame?i={index}&v={version}"))
}

/// 读取附加帧列表（JSON，损坏视为空）。协议层也要按序号取帧文件，故 pub。
pub fn read_frames(conn: &rusqlite::Connection) -> Vec<FrameEntry> {
    settings_repo::get(conn, PET_FRAMES_KEY)
        .ok()
        .flatten()
        .and_then(|raw| serde_json::from_str(&raw).ok())
        .unwrap_or_default()
}

fn write_frames(conn: &rusqlite::Connection, frames: &[FrameEntry]) -> Result<(), String> {
    let json = serde_json::to_string(frames).map_err(|e| format!("序列化帧列表失败: {e}"))?;
    settings_repo::set(conn, PET_FRAMES_KEY, &json)
}

pub fn get_pet_photo_impl(app: &AppHandle) -> PetPhotoConfig {
    let path = read_kv(app, PET_PHOTO_PATH_KEY).filter(|p| Path::new(p).is_file());
    // 旧版本（阶段 3）没有源图概念：把已导入的照片同时当源图，便于直接重抠
    let source_path = read_kv(app, PET_PHOTO_SOURCE_PATH_KEY)
        .filter(|p| Path::new(p).is_file())
        .or_else(|| path.clone());
    let enabled = read_kv(app, PET_PHOTO_ENABLED_KEY).map(|v| v == "true").unwrap_or(false);
    let style = read_kv(app, PET_STYLE_KEY).unwrap_or_else(|| PetStyle::Original.as_str().into());
    let tolerance = read_kv(app, PET_TOLERANCE_KEY)
        .and_then(|v| v.parse::<u8>().ok())
        .unwrap_or(DEFAULT_TOLERANCE);
    let use_ai = read_kv(app, PET_SEGMENTATION_KEY)
        .map(|v| v != "false")
        .unwrap_or(true);
    let frame_ms = read_kv(app, PET_FRAME_MS_KEY)
        .and_then(|v| v.parse::<u64>().ok())
        .map(|v| v.clamp(100, 2000))
        .unwrap_or(PET_FRAME_MS_DEFAULT);
    let pet_size = read_kv(app, PET_SIZE_KEY)
        .and_then(|v| v.parse::<u32>().ok())
        .map(|v| clamp_u32(v, PET_SIZE_MIN, PET_SIZE_MAX, PET_SIZE_DEFAULT))
        .unwrap_or(PET_SIZE_DEFAULT);
    let pet_opacity = read_kv(app, PET_OPACITY_KEY)
        .and_then(|v| v.parse::<u8>().ok())
        .map(|v| v.clamp(20, 100))
        .unwrap_or(PET_OPACITY_DEFAULT);

    // 帧 0 = 主形象，后面是附加帧
    let mut frames: Vec<String> = Vec::new();
    if let Some(p) = &path {
        if let Some(u) = frame_url(p, 0) {
            frames.push(u);
        }
    }
    let extra_sprites: Vec<String> = {
        let db = app.try_state::<DbState>();
        match db {
            Some(db) => match db.0.lock() {
                Ok(conn) => read_frames(&conn)
                    .into_iter()
                    .filter(|f| Path::new(&f.sprite).is_file())
                    .map(|f| f.sprite)
                    .collect(),
                Err(_) => Vec::new(),
            },
            None => Vec::new(),
        }
    };
    for (i, sprite) in extra_sprites.iter().enumerate() {
        if let Some(u) = frame_url(sprite, i + 1) {
            frames.push(u);
        }
    }

    PetPhotoConfig {
        url: path.as_ref().and_then(|p| photo_url(p, "photo")),
        source_url: source_path.as_ref().and_then(|p| photo_url(p, "source")),
        path,
        enabled,
        source_path,
        style,
        tolerance,
        use_ai,
        frames,
        frame_ms,
        pet_size,
        pet_opacity,
    }
}

#[tauri::command]
pub fn get_pet_photo(app: AppHandle) -> PetPhotoConfig {
    get_pet_photo_impl(&app)
}

/// 弹出系统文件对话框选照片，复制进应用数据目录、自动抠图并启用照片桌宠。
/// async + spawn_blocking：图像处理很重，绝不能占主线程（否则所有窗口冻结）。
/// 抠图失败时源图已落库，可在桌宠中心调容差重试。
#[tauri::command]
pub async fn pick_pet_photo(app: AppHandle) -> Result<Option<PetPhotoConfig>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(src) = rfd::FileDialog::new()
            .set_title("选择桌宠照片")
            .add_filter("图片", &["png", "jpg", "jpeg", "webp", "gif"])
            .pick_file()
        else {
            // 用户取消选择：不算错误
            return Ok(None);
        };
        set_pet_photo_from_path(&app, src).map(Some)
    })
    .await
    .map_err(|e| format!("照片处理线程失败: {e}"))?
}

/// 导入照片：复制源图 → 按当前容差/风格抠图 → 写库 → 广播。
/// 源图路径先落库再抠图：即使抠图失败，用户也能调容差重试，不会「卡死」在上传。
pub fn set_pet_photo_from_path(app: &AppHandle, src: PathBuf) -> Result<PetPhotoConfig, String> {
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_else(|| "png".to_string());
    let dest_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法确定数据目录: {e}"))?;
    std::fs::create_dir_all(&dest_dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    let source = dest_dir.join(format!("{PET_SOURCE_FILE_STEM}.{ext}"));
    std::fs::copy(&src, &source).map_err(|e| format!("复制照片失败: {e}"))?;
    let source_str = source.to_string_lossy().to_string();

    let current = get_pet_photo_impl(app);
    let style = PetStyle::from_db(&current.style).unwrap_or(PetStyle::Original);
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        settings_repo::set(&conn, PET_PHOTO_SOURCE_PATH_KEY, &source_str)?;
    }
    // 失败也返回 Err，但源图已记录：前端刷新后会出现容差/风格控件供重试
    process_and_save(app, &source_str, style, current.tolerance, current.use_ai)?;
    let config = get_pet_photo_impl(app);
    emit_photo_changed(app, &config);
    Ok(config)
}

/// 按新参数重新处理已导入的照片（分割/抠图 + 风格）。传 None 表示沿用当前值。
/// 同样走 spawn_blocking；参数先落库，拖滑块失败也不回跳。
#[tauri::command]
pub async fn reprocess_pet_photo(
    app: AppHandle,
    tolerance: Option<u8>,
    style: Option<String>,
    use_ai: Option<bool>,
) -> Result<PetPhotoConfig, String> {
    tauri::async_runtime::spawn_blocking(move || {
        reprocess_pet_photo_impl(&app, tolerance, style, use_ai)
    })
    .await
    .map_err(|e| format!("照片处理线程失败: {e}"))?
}

fn reprocess_pet_photo_impl(
    app: &AppHandle,
    tolerance: Option<u8>,
    style: Option<String>,
    use_ai: Option<bool>,
) -> Result<PetPhotoConfig, String> {
    let current = get_pet_photo_impl(app);
    let source = current
        .source_path
        .clone()
        .filter(|p| Path::new(p).is_file())
        .ok_or_else(|| "还没有导入照片".to_string())?;
    let new_style = match &style {
        Some(s) => PetStyle::from_db(s)?,
        None => PetStyle::from_db(&current.style).unwrap_or(PetStyle::Original),
    };
    let new_tolerance = tolerance.unwrap_or(current.tolerance);
    let new_use_ai = use_ai.unwrap_or(current.use_ai);
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        if let Some(s) = &style {
            settings_repo::set(&conn, PET_STYLE_KEY, s)?;
        }
        if tolerance.is_some() {
            settings_repo::set(&conn, PET_TOLERANCE_KEY, &new_tolerance.to_string())?;
        }
        if use_ai.is_some() {
            settings_repo::set(
                &conn,
                PET_SEGMENTATION_KEY,
                if new_use_ai { "true" } else { "false" },
            )?;
        }
    }
    process_and_save(app, &source, new_style, new_tolerance, new_use_ai)?;
    // 风格/容差变了：附加帧也按同样参数重处理（源图已丢的帧跳过）
    let frames = {
        let db = app.state::<DbState>();
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        read_frames(&conn)
    };
    for f in &frames {
        if Path::new(&f.source).is_file() {
            if let Some(name) = Path::new(&f.sprite).file_name() {
                let _ = process_sprite_to(
                    app,
                    &f.source,
                    new_style,
                    new_tolerance,
                    new_use_ai,
                    &name.to_string_lossy(),
                );
            }
        }
    }
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        // 换了新形象：自动启用照片桌宠（关着的用户点「重新抠图」显然想看效果）
        settings_repo::set(&conn, PET_PHOTO_ENABLED_KEY, "true")?;
    }
    let config = get_pet_photo_impl(app);
    emit_photo_changed(app, &config);
    Ok(config)
}

/// 抠图 + 风格处理并落盘到指定文件名，返回成品路径。
fn process_sprite_to(
    app: &AppHandle,
    source_path: &str,
    style: PetStyle,
    tolerance: u8,
    use_ai: bool,
    file_name: &str,
) -> Result<PathBuf, String> {
    let img = image::open(source_path).map_err(|e| format!("读取照片失败: {e}"))?;
    let model_path = if use_ai {
        let model_dir = app
            .path()
            .app_data_dir()
            .map_err(|e| format!("无法确定数据目录: {e}"))?
            .join("models");
        Some(pet_segment::ensure_model(&model_dir)?)
    } else {
        None
    };
    let sprite = pet_avatar::process_avatar(&img, style, tolerance, use_ai, model_path.as_deref())?;
    let dest_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法确定数据目录: {e}"))?;
    std::fs::create_dir_all(&dest_dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    let dest = dest_dir.join(file_name);
    sprite
        .save_with_format(&dest, image::ImageFormat::Png)
        .map_err(|e| format!("保存桌宠形象失败: {e}"))?;
    Ok(dest)
}

/// 主形象处理并写回 PET_PHOTO_PATH_KEY。
fn process_and_save(
    app: &AppHandle,
    source_path: &str,
    style: PetStyle,
    tolerance: u8,
    use_ai: bool,
) -> Result<(), String> {
    let dest = process_sprite_to(app, source_path, style, tolerance, use_ai, PET_SPRITE_FILE_NAME)?;
    let db = app.state::<DbState>();
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    settings_repo::set(&conn, PET_PHOTO_PATH_KEY, &dest.to_string_lossy())?;
    Ok(())
}

/// 切换「照片桌宠 / 默认小猫」（不动已导入的照片，随时能切回来）。
#[tauri::command]
pub fn set_pet_photo_enabled(app: AppHandle, enabled: bool) -> Result<PetPhotoConfig, String> {
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        settings_repo::set(&conn, PET_PHOTO_ENABLED_KEY, if enabled { "true" } else { "false" })?;
    }
    let config = get_pet_photo_impl(&app);
    emit_photo_changed(&app, &config);
    Ok(config)
}

// ---------- 桌宠人格 ----------

#[tauri::command]
pub fn get_pet_personality(app: AppHandle) -> Result<String, String> {
    let db = app
        .try_state::<DbState>()
        .ok_or_else(|| "数据库初始化中".to_string())?;
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    Ok(PetPersonality::read_from(&conn).as_str().to_string())
}

/// 设置桌宠人格；桌宠窗口与主面板都会收到变更事件。
#[tauri::command]
pub fn set_pet_personality(app: AppHandle, personality: String) -> Result<(), String> {
    let value = PetPersonality::from_db(&personality)?;
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        settings_repo::set(&conn, PET_PERSONALITY_KEY, value.as_str())?;
    }
    let _ = app.emit_to("pet", PET_PERSONALITY_CHANGED_EVENT, value.as_str());
    let _ = app.emit_to("settings", PET_PERSONALITY_CHANGED_EVENT, value.as_str());
    Ok(())
}

fn emit_photo_changed(app: &AppHandle, config: &PetPhotoConfig) {
    let _ = app.emit_to("pet", PET_PHOTO_CHANGED_EVENT, config);
    let _ = app.emit_to("settings", PET_PHOTO_CHANGED_EVENT, config);
}

// ---------- 多帧动画与显示调节 ----------

/// 追加一帧动画：选照片 → 按当前风格/容差/分割模式处理 → 加入帧列表。
#[tauri::command]
pub async fn add_pet_frame(app: AppHandle) -> Result<PetPhotoConfig, String> {
    tauri::async_runtime::spawn_blocking(move || add_pet_frame_impl(&app))
        .await
        .map_err(|e| format!("处理线程失败: {e}"))?
}

fn add_pet_frame_impl(app: &AppHandle) -> Result<PetPhotoConfig, String> {
    let current = get_pet_photo_impl(app);
    if current.source_path.is_none() {
        return Err("请先上传主形象照片，再添加动画帧".to_string());
    }
    let Some(src) = rfd::FileDialog::new()
        .set_title("选择新一帧照片")
        .add_filter("图片", &["png", "jpg", "jpeg", "webp"])
        .pick_file()
    else {
        // 用户取消：返回当前配置，不算错误
        return Ok(current);
    };
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        if read_frames(&conn).len() >= 7 {
            return Err("帧数已达上限（主形象 + 7 帧）".to_string());
        }
    }
    let ext = src
        .extension()
        .and_then(|e| e.to_str())
        .map(|e| e.to_ascii_lowercase())
        .unwrap_or_else(|| "png".to_string());
    let dest_dir = app
        .path()
        .app_data_dir()
        .map_err(|e| format!("无法确定数据目录: {e}"))?;
    std::fs::create_dir_all(&dest_dir).map_err(|e| format!("创建数据目录失败: {e}"))?;
    let ts = chrono::Utc::now().timestamp_millis();
    let source = dest_dir.join(format!("pet_source_f{ts}.{ext}"));
    std::fs::copy(&src, &source).map_err(|e| format!("复制照片失败: {e}"))?;
    let style = PetStyle::from_db(&current.style).unwrap_or(PetStyle::Original);
    let sprite = process_sprite_to(
        app,
        &source.to_string_lossy(),
        style,
        current.tolerance,
        current.use_ai,
        &format!("pet_frame_{ts}.png"),
    )?;
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        let mut frames = read_frames(&conn);
        frames.push(FrameEntry {
            source: source.to_string_lossy().to_string(),
            sprite: sprite.to_string_lossy().to_string(),
        });
        write_frames(&conn, &frames)?;
    }
    let config = get_pet_photo_impl(app);
    emit_photo_changed(app, &config);
    Ok(config)
}

/// 删除一个附加帧（index 对应配置 frames 数组的下标，0 是主形象不可删）。
#[tauri::command]
pub async fn remove_pet_frame(app: AppHandle, index: usize) -> Result<PetPhotoConfig, String> {
    tauri::async_runtime::spawn_blocking(move || {
        if index == 0 {
            return Err("第 0 帧是主形象，请用「更换照片…」替换".to_string());
        }
        let db = app.state::<DbState>();
        let (removed, remaining) = {
            let conn = db.0.lock().map_err(|_| "数据库被占用")?;
            let mut frames = read_frames(&conn);
            if index - 1 >= frames.len() {
                return Err("帧不存在".to_string());
            }
            let removed = frames.remove(index - 1);
            write_frames(&conn, &frames)?;
            (removed, frames)
        };
        let _ = std::fs::remove_file(&removed.sprite);
        let _ = std::fs::remove_file(&removed.source);
        let config = get_pet_photo_impl(&app);
        emit_photo_changed(&app, &config);
        let _ = remaining;
        Ok(config)
    })
    .await
    .map_err(|e| format!("处理线程失败: {e}"))?
}

/// 设置多帧轮播间隔（毫秒，100~2000）。
#[tauri::command]
pub fn set_pet_frame_speed(app: AppHandle, ms: u64) -> Result<(), String> {
    let ms = ms.clamp(100, 2000);
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        settings_repo::set(&conn, PET_FRAME_MS_KEY, &ms.to_string())?;
    }
    let config = get_pet_photo_impl(&app);
    emit_photo_changed(&app, &config);
    Ok(())
}

/// 设置桌宠显示参数：边长（64~192px）与不透明度（20~100%）。
#[tauri::command]
pub fn set_pet_display(app: AppHandle, size: u32, opacity: u8) -> Result<(), String> {
    let size = clamp_u32(size, PET_SIZE_MIN, PET_SIZE_MAX, PET_SIZE_DEFAULT);
    let opacity = opacity.clamp(20, 100);
    {
        let db = app.state::<DbState>();
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        settings_repo::set(&conn, PET_SIZE_KEY, &size.to_string())?;
        settings_repo::set(&conn, PET_OPACITY_KEY, &opacity.to_string())?;
    }
    let config = get_pet_photo_impl(&app);
    emit_photo_changed(&app, &config);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mime_matches_common_image_types() {
        assert_eq!(mime_for_ext(Path::new("C:\\a\\pet_photo.PNG")), "image/png");
        assert_eq!(mime_for_ext(Path::new("p.jpg")), "image/jpeg");
        assert_eq!(mime_for_ext(Path::new("p.JPEG")), "image/jpeg");
        assert_eq!(mime_for_ext(Path::new("p.webp")), "image/webp");
        assert_eq!(mime_for_ext(Path::new("p.gif")), "image/gif");
        assert_eq!(mime_for_ext(Path::new("p.bmp")), "application/octet-stream");
    }

    #[test]
    fn personality_roundtrip_and_fallback() {
        for p in [
            PetPersonality::Gentle,
            PetPersonality::Motivator,
            PetPersonality::Sarcastic,
            PetPersonality::Cool,
        ] {
            assert_eq!(PetPersonality::from_db(p.as_str()).unwrap(), p);
        }
        assert!(PetPersonality::from_db("angry").is_err());
        // 缺省回落温柔型
        let conn = rusqlite::Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "CREATE TABLE settings (key TEXT PRIMARY KEY, value TEXT NOT NULL);",
        )
        .unwrap();
        assert_eq!(PetPersonality::read_from(&conn), PetPersonality::Gentle);
        conn.execute_batch("INSERT INTO settings (key, value) VALUES ('petPersonality', 'sarcastic');")
            .unwrap();
        assert_eq!(PetPersonality::read_from(&conn), PetPersonality::Sarcastic);
    }
}
