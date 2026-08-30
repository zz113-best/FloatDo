//! 统计后端：从仓库层取按日汇总，拼成前端报表。
//! 只做拼装，聚合与归桶都在 stats_repo。
//! 统计页不再独立成窗口，而是主面板（settings 窗口）左侧导航的一个页签。

use super::DbState;
use crate::database::stats_repo;
use serde::Serialize;
use tauri::{AppHandle, Manager};

/// 默认统计范围（天）。
const DEFAULT_DAYS: i64 = 7;
const MAX_DAYS: i64 = 90;

/// 最近完成任务明细最多展示条数。
const RECENT_TASKS_LIMIT: i64 = 10;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StatsReport {
    /// 实际统计范围（天）
    pub days: i64,
    pub focus_days: Vec<stats_repo::FocusDayStat>,
    pub task_days: Vec<stats_repo::TaskDayStat>,
    /// 范围内专注总秒数 / 完整完成的轮数
    pub focus_total_seconds: i64,
    pub focus_total_sessions: i64,
    /// 今日（本地日）完成的专注秒数
    pub focus_today_seconds: i64,
    pub task_overview: stats_repo::TaskOverview,
    /// 最近完成的任务明细
    pub recent_tasks: Vec<stats_repo::RecentTask>,
    /// 范围内专注时长按任务拆分（时长降序）
    pub focus_by_task: Vec<stats_repo::TaskFocusStat>,
}

fn build_report(conn: &rusqlite::Connection, days: i64) -> Result<StatsReport, String> {
    let days = days.clamp(1, MAX_DAYS);
    let focus_days = stats_repo::focus_daily(conn, days)?;
    let task_days = stats_repo::task_daily_completed(conn, days)?;
    let focus_total_seconds = focus_days.iter().map(|d| d.focus_seconds).sum();
    let focus_total_sessions = focus_days.iter().map(|d| d.sessions).sum();
    // 序列最后一格就是今天
    let focus_today_seconds = focus_days.last().map(|d| d.focus_seconds).unwrap_or(0);
    let task_overview = stats_repo::task_overview(conn)?;
    let recent_tasks = stats_repo::recent_completed_tasks(conn, RECENT_TASKS_LIMIT)?;
    let focus_by_task = stats_repo::focus_by_task(conn, days)?;
    Ok(StatsReport {
        days,
        focus_days,
        task_days,
        focus_total_seconds,
        focus_total_sessions,
        focus_today_seconds,
        task_overview,
        recent_tasks,
        focus_by_task,
    })
}

#[tauri::command]
pub fn get_stats(app: AppHandle, days: Option<i64>) -> Result<StatsReport, String> {
    let db = app
        .try_state::<DbState>()
        .ok_or_else(|| "数据库初始化中".to_string())?;
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    build_report(&conn, days.unwrap_or(DEFAULT_DAYS))
}

/// 托盘/设置入口：打开主面板并直接跳到统计页。
#[tauri::command]
pub fn open_stats(app: AppHandle) -> Result<(), String> {
    super::settings::open_panel(&app, "stats")
}
