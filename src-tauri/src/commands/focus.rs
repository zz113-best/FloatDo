//! 专注模式（番茄钟）：一轮专注 → 一轮休息，循环由后端统一计时。
//! 计时的权威在后端：会话落 SQLite（focus_sessions 表），当前阶段放在
//! 进程内存 FocusRuntime。前端只展示倒计时与收发事件，窗口隐藏/重启
//! 都不会丢会话（重启时从 DB 的 RUNNING 记录恢复）。

use crate::commands::DbState;
use crate::database::{
    focus_repo,
    models::{FocusSession, FocusStatus},
    settings_repo,
};
use chrono::{DateTime, Local, TimeZone, Utc};
use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const EVENT_FOCUS_CHANGED: &str = "focus://changed";

const FOCUS_WORK_MINUTES_KEY: &str = "focusWorkMinutes";
const FOCUS_BREAK_MINUTES_KEY: &str = "focusBreakMinutes";
const DEFAULT_WORK_MINUTES: i64 = 25;
const DEFAULT_BREAK_MINUTES: i64 = 5;
/// 专注时长允许范围（分钟）。
pub const MIN_MINUTES: i64 = 1;
pub const MAX_MINUTES: i64 = 180;

const PHASE_IDLE: &str = "IDLE";
const PHASE_FOCUS: &str = "FOCUS";
const PHASE_BREAK: &str = "BREAK";

/// 调度线程的检查间隔。
const TICK_INTERVAL: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, PartialEq)]
pub enum Phase {
    Focus,
    Break,
}

/// 当前进行中的阶段（IDLE 时为 None）。
#[derive(Debug, Clone)]
pub struct ActivePhase {
    pub phase: Phase,
    pub ends_at: DateTime<Utc>,
}

/// 全局专注运行时，setup 时 manage 进 Tauri。
pub struct FocusRuntime(pub Mutex<Option<ActivePhase>>);

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusState {
    /// IDLE / FOCUS / BREAK
    pub phase: String,
    /// 当前阶段结束时间（RFC3339），IDLE 为 null
    pub ends_at: Option<String>,
    /// 进行中的会话；BREAK / IDLE 阶段为 null
    pub session: Option<FocusSession>,
    pub work_minutes: i64,
    pub break_minutes: i64,
    /// 今日（本地时间零点起）完成的专注总秒数
    pub today_seconds: i64,
}

/// 纯函数：中途停止时实际专注的秒数，封顶为计划时长。便于单元测试。
pub fn actual_focus_seconds(
    started_at: DateTime<Utc>,
    planned_minutes: i64,
    now: DateTime<Utc>,
) -> i64 {
    let planned_secs = planned_minutes.max(0) * 60;
    ((now - started_at).num_seconds().max(0)).min(planned_secs)
}

fn read_minutes(app: &AppHandle, key: &str, default: i64) -> Result<i64, String> {
    let db = app
        .try_state::<DbState>()
        .ok_or_else(|| "数据库初始化中".to_string())?;
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    Ok(settings_repo::get(&conn, key)?
        .and_then(|v| v.parse::<i64>().ok())
        .filter(|v| (MIN_MINUTES..=MAX_MINUTES).contains(v))
        .unwrap_or(default))
}

fn work_minutes(app: &AppHandle) -> Result<i64, String> {
    read_minutes(app, FOCUS_WORK_MINUTES_KEY, DEFAULT_WORK_MINUTES)
}

fn break_minutes(app: &AppHandle) -> Result<i64, String> {
    read_minutes(app, FOCUS_BREAK_MINUTES_KEY, DEFAULT_BREAK_MINUTES)
}

/// 今日（本地时区零点起）完成的专注秒数。
fn today_seconds(app: &AppHandle) -> Result<i64, String> {
    let midnight = chrono::Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|t| Local.from_local_datetime(&t).earliest())
        .unwrap_or_else(Local::now)
        .with_timezone(&Utc);
    let db = app
        .try_state::<DbState>()
        .ok_or_else(|| "数据库初始化中".to_string())?;
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    focus_repo::completed_seconds_since(&conn, &midnight.to_rfc3339())
}

fn build_state(app: &AppHandle) -> Result<FocusState, String> {
    let runtime = app.state::<FocusRuntime>();
    let active = runtime.0.lock().map_err(|_| "专注状态被占用")?.clone();

    let (phase, ends_at, session) = match active {
        Some(active) => {
            let db = app
                .try_state::<DbState>()
                .ok_or_else(|| "数据库初始化中".to_string())?;
            let conn = db.0.lock().map_err(|_| "数据库被占用")?;
            let session = if active.phase == Phase::Focus {
                // FOCUS 阶段：找进行中的会话
                focus_repo::running(&conn)?
            } else {
                None
            };
            (
                match active.phase {
                    Phase::Focus => PHASE_FOCUS,
                    Phase::Break => PHASE_BREAK,
                },
                Some(active.ends_at.to_rfc3339()),
                session,
            )
        }
        None => (PHASE_IDLE, None, None),
    };

    Ok(FocusState {
        phase: phase.to_string(),
        ends_at,
        session,
        work_minutes: work_minutes(app)?,
        break_minutes: break_minutes(app)?,
        today_seconds: today_seconds(app)?,
    })
}

fn publish(app: &AppHandle) {
    match build_state(app) {
        Ok(state) => {
            // 阶段切换是低频事件，打一行日志便于排查「界面没跟着切」类问题
            println!(
                "focus: phase={} ends_at={:?} today_secs={}",
                state.phase, state.ends_at, state.today_seconds
            );
            let _ = app.emit_to("widget", EVENT_FOCUS_CHANGED, state.clone());
            let _ = app.emit_to("pet", EVENT_FOCUS_CHANGED, state);
        }
        Err(e) => eprintln!("推送专注状态失败: {e}"),
    }
}

/// setup 时调用：把上次运行遗留的 RUNNING 会话接回内存。
/// 已过期的直接按「完整完成」收尾，不进入休息。
pub fn init(app: &AppHandle) -> Result<(), String> {
    // init 由 setup 在 manage(DbState) 之后调用，这里 state() 是安全的
    let db = app.state::<DbState>();
    let running = {
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        focus_repo::running(&conn)?
    };
    let Some(session) = running else {
        return Ok(());
    };
    let started = DateTime::parse_from_rfc3339(&session.started_at)
        .map_err(|e| format!("专注会话时间解析失败: {e}"))?
        .with_timezone(&Utc);
    let ends_at = started + chrono::Duration::minutes(session.planned_minutes);
    let runtime = app.state::<FocusRuntime>();
    if Utc::now() >= ends_at {
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        focus_repo::finish(&conn, session.id, session.planned_minutes * 60, &FocusStatus::Completed)?;
        return Ok(());
    }
    *runtime.0.lock().map_err(|_| "专注状态被占用")? = Some(ActivePhase {
        phase: Phase::Focus,
        ends_at,
    });
    Ok(())
}

/// setup 时调用：后台线程每秒检查一次到点切换（专注→休息→空闲）。
pub fn spawn_scheduler(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(TICK_INTERVAL);
        if let Err(e) = tick(&app) {
            eprintln!("专注调度失败: {e}");
        }
    });
}

fn tick(app: &AppHandle) -> Result<bool, String> {
    let transitioned = {
        let runtime = app.state::<FocusRuntime>();
        let mut active = runtime.0.lock().map_err(|_| "专注状态被占用")?;
        let Some(current) = active.clone() else {
            return Ok(false);
        };
        if Utc::now() < current.ends_at {
            return Ok(false);
        }
        match current.phase {
            Phase::Focus => {
                // 专注轮完整跑完：按计划时长收尾，进入休息。
                // 注意：休息/专注时长要先在拿数据库锁之前读好——
                // read_minutes 内部也要锁 DbState，而 std Mutex 不可重入，
                // 在持有 conn 的情况下再锁会自死锁（调度线程卡死 → 主线程跟着冻结）
                let minutes = work_minutes(app)?;
                let rest = break_minutes(app)?;
                let db = app
                    .try_state::<DbState>()
                    .ok_or_else(|| "数据库初始化中".to_string())?;
                {
                    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
                    let running = focus_repo::running(&conn)?;
                    if let Some(session) = running {
                        focus_repo::finish(&conn, session.id, minutes * 60, &FocusStatus::Completed)?;
                    }
                }
                *active = Some(ActivePhase {
                    phase: Phase::Break,
                    ends_at: Utc::now() + chrono::Duration::minutes(rest),
                });
            }
            Phase::Break => {
                *active = None;
            }
        }
        true
    };
    if transitioned {
        publish(app);
    }
    Ok(transitioned)
}

fn clamp_minutes(minutes: i64) -> i64 {
    minutes.clamp(MIN_MINUTES, MAX_MINUTES)
}

#[tauri::command]
pub fn get_focus_state(app: AppHandle) -> Result<FocusState, String> {
    build_state(&app)
}

/// 开始一轮专注。已在专注/休息中则直接返回当前状态，不重复开新会话。
/// minutes 不传时用设置里的专注时长。
#[tauri::command]
pub fn start_focus(
    app: AppHandle,
    task_id: Option<i64>,
    minutes: Option<i64>,
) -> Result<FocusState, String> {
    let runtime = app.state::<FocusRuntime>();
    if runtime.0.lock().map_err(|_| "专注状态被占用")?.is_some() {
        return build_state(&app);
    }
    let planned = clamp_minutes(minutes.unwrap_or(work_minutes(&app)?));
    let db = app
        .try_state::<DbState>()
        .ok_or_else(|| "数据库初始化中".to_string())?;
    {
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        focus_repo::create_running(&conn, task_id, planned)?;
    }
    *runtime.0.lock().map_err(|_| "专注状态被占用")? = Some(ActivePhase {
        phase: Phase::Focus,
        ends_at: Utc::now() + chrono::Duration::minutes(planned),
    });
    publish(&app);
    build_state(&app)
}

/// 停止当前阶段：专注轮记 INTERRUPTED（按实际秒数），休息轮直接取消。
#[tauri::command]
pub fn stop_focus(app: AppHandle) -> Result<FocusState, String> {
    let runtime = app.state::<FocusRuntime>();
    let active = runtime.0.lock().map_err(|_| "专注状态被占用")?.take();
    if let Some(active) = active {
        if active.phase == Phase::Focus {
            let db = app
                .try_state::<DbState>()
                .ok_or_else(|| "数据库初始化中".to_string())?;
            let conn = db.0.lock().map_err(|_| "数据库被占用")?;
            let running = focus_repo::running(&conn)?;
            if let Some(session) = running {
                let started = DateTime::parse_from_rfc3339(&session.started_at)
                    .map_err(|e| format!("专注会话时间解析失败: {e}"))?
                    .with_timezone(&Utc);
                let actual = actual_focus_seconds(started, session.planned_minutes, Utc::now());
                focus_repo::finish(&conn, session.id, actual, &FocusStatus::Interrupted)?;
            }
        }
    }
    publish(&app);
    build_state(&app)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    #[test]
    fn actual_seconds_capped_at_planned() {
        let start = Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap();
        // 跑了 10 分钟，计划 25 分钟 → 600 秒
        assert_eq!(
            actual_focus_seconds(start, 25, start + chrono::Duration::minutes(10)),
            600
        );
        // 超过计划时长（调度线程延迟）→ 封顶为计划秒数
        assert_eq!(
            actual_focus_seconds(start, 25, start + chrono::Duration::minutes(30)),
            25 * 60
        );
        // 起跑瞬间停止 → 0
        assert_eq!(actual_focus_seconds(start, 25, start), 0);
    }
}
