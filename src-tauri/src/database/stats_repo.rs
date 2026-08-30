//! 统计查询：专注 / 任务的按日汇总与任务总览。
//! 库里时间戳统一是 UTC RFC3339，按「本地日」归桶在这一层完成，
//! 汇总结果直接是前端要画的序列，React 组件里不做任何聚合运算。

use chrono::{DateTime, Local, NaiveDate, TimeZone, Utc};
use rusqlite::{params, Connection};
use serde::Serialize;

/// 单日专注汇总（本地日）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusDayStat {
    /// 本地日期 YYYY-MM-DD
    pub date: String,
    /// 当天完成的专注总秒数
    pub focus_seconds: i64,
    /// 当天完整完成的专注轮数
    pub sessions: i64,
}

/// 单日完成任务数（按 completed_at 归本地日）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskDayStat {
    pub date: String,
    pub completed: i64,
}

/// 任务总览（全量，不随统计时间范围变化）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskOverview {
    pub total: i64,
    pub completed: i64,
    pub pending: i64,
    /// 待办里已过截止时间的数量
    pub overdue: i64,
    /// 已完成任务里「完成时间晚于截止时间」的数量（逾期后才完成的）
    pub completed_late: i64,
}

/// 最近完成的任务（最近完成时间倒序），供统计页展示明细。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecentTask {
    pub id: i64,
    pub title: String,
    pub completed_at: Option<String>,
    pub due_at: Option<String>,
    /// 完成时间晚于截止时间（逾期完成）
    pub late: bool,
}

/// 最近 days 天（含今天）的专注按日汇总，旧 → 新。没有数据的天也保留空桶，图表每天有柱位。
pub fn focus_daily(conn: &Connection, days: i64) -> Result<Vec<FocusDayStat>, String> {
    let (start, dates) = day_range(days);
    let mut buckets: Vec<FocusDayStat> = dates
        .iter()
        .map(|date| FocusDayStat {
            date: date.format("%Y-%m-%d").to_string(),
            focus_seconds: 0,
            sessions: 0,
        })
        .collect();

    let rows: Vec<(String, i64)> = conn
        .prepare(
            "SELECT started_at, actual_seconds FROM focus_sessions
             WHERE status = 'COMPLETED' AND started_at >= ?1",
        )
        .map_err(|e| format!("统计专注数据失败: {e}"))?
        .query_map(params![day_start_utc(start).to_rfc3339()], |r| {
            Ok((r.get(0)?, r.get(1)?))
        })
        .map_err(|e| format!("统计专注数据失败: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("统计专注数据失败: {e}"))?;

    for (started_at, seconds) in rows {
        let Ok(t) = DateTime::parse_from_rfc3339(&started_at) else {
            continue;
        };
        if let Some(index) = bucket_index(&dates, start, &t.with_timezone(&Local).date_naive()) {
            buckets[index].focus_seconds += seconds;
            buckets[index].sessions += 1;
        }
    }
    Ok(buckets)
}

/// 单个任务的专注累计（统计范围内）。
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskFocusStat {
    pub task_id: Option<i64>,
    /// 任务已被删除时为 None（前端显示「未关联/已删除任务」）
    pub title: Option<String>,
    pub focus_seconds: i64,
    pub sessions: i64,
}

/// 最近 days 天的专注时长按任务拆分，时长降序；未关联任务的会话单独一组。
pub fn focus_by_task(conn: &Connection, days: i64) -> Result<Vec<TaskFocusStat>, String> {
    let (start, _) = day_range(days);
    let mut stmt = conn
        .prepare(
            "SELECT s.task_id, t.title, SUM(s.actual_seconds), COUNT(*)
             FROM focus_sessions s
             LEFT JOIN tasks t ON t.id = s.task_id
             WHERE s.status = 'COMPLETED' AND s.started_at >= ?1
             GROUP BY s.task_id
             ORDER BY SUM(s.actual_seconds) DESC",
        )
        .map_err(|e| format!("统计专注分布失败: {e}"))?;
    let rows = stmt
        .query_map(params![day_start_utc(start).to_rfc3339()], |r| {
            Ok(TaskFocusStat {
                task_id: r.get(0)?,
                title: r.get(1)?,
                focus_seconds: r.get::<_, Option<i64>>(2)?.unwrap_or(0),
                sessions: r.get::<_, Option<i64>>(3)?.unwrap_or(0),
            })
        })
        .map_err(|e| format!("统计专注分布失败: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(rows)
}

/// 最近 days 天（含今天）的完成任务数按日汇总，旧 → 新。
pub fn task_daily_completed(conn: &Connection, days: i64) -> Result<Vec<TaskDayStat>, String> {
    let (start, dates) = day_range(days);
    let mut buckets: Vec<TaskDayStat> = dates
        .iter()
        .map(|date| TaskDayStat {
            date: date.format("%Y-%m-%d").to_string(),
            completed: 0,
        })
        .collect();

    let rows: Vec<String> = conn
        .prepare(
            "SELECT completed_at FROM tasks
             WHERE status = 'COMPLETED' AND completed_at IS NOT NULL AND completed_at >= ?1",
        )
        .map_err(|e| format!("统计任务数据失败: {e}"))?
        .query_map(params![day_start_utc(start).to_rfc3339()], |r| r.get(0))
        .map_err(|e| format!("统计任务数据失败: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("统计任务数据失败: {e}"))?;

    for completed_at in rows {
        let Ok(t) = DateTime::parse_from_rfc3339(&completed_at) else {
            continue;
        };
        if let Some(index) = bucket_index(&dates, start, &t.with_timezone(&Local).date_naive()) {
            buckets[index].completed += 1;
        }
    }
    Ok(buckets)
}

/// 任务总览：总数 / 已完成 / 待办 / 逾期 / 逾期后完成。
/// 逾期与逾期判定都在 Rust 里逐条比对：due_at 是前端 toISOString 存的 UTC 时间，
/// 库里还有 chrono 存的 +00:00 格式，字符串比较不可靠。
pub fn task_overview(conn: &Connection) -> Result<TaskOverview, String> {
    let (total, completed, pending) = conn
        .query_row(
            "SELECT COUNT(*),
                    COALESCE(SUM(CASE WHEN status = 'COMPLETED' THEN 1 ELSE 0 END), 0),
                    COALESCE(SUM(CASE WHEN status IN ('TODO', 'IN_PROGRESS') THEN 1 ELSE 0 END), 0)
             FROM tasks",
            [],
            |r| Ok((r.get(0)?, r.get(1)?, r.get(2)?)),
        )
        .map_err(|e| format!("统计任务总览失败: {e}"))?;

    let now = Utc::now();
    // 当前逾期：还没做完但截止时间已过
    let dues: Vec<Option<String>> = conn
        .prepare(
            "SELECT due_at FROM tasks
             WHERE status IN ('TODO', 'IN_PROGRESS') AND due_at IS NOT NULL",
        )
        .map_err(|e| format!("统计逾期任务失败: {e}"))?
        .query_map([], |r| r.get(0))
        .map_err(|e| format!("统计逾期任务失败: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("统计逾期任务失败: {e}"))?;
    let overdue = dues
        .iter()
        .filter(|d| is_past_due(d.as_deref(), now))
        .count() as i64;

    // 逾期后才完成：做完的时间点已经晚于截止时间
    let done_rows: Vec<(Option<String>, Option<String>)> = conn
        .prepare(
            "SELECT due_at, completed_at FROM tasks
             WHERE status = 'COMPLETED' AND due_at IS NOT NULL AND completed_at IS NOT NULL",
        )
        .map_err(|e| format!("统计逾期完成任务失败: {e}"))?
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(|e| format!("统计逾期完成任务失败: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("统计逾期完成任务失败: {e}"))?;
    let completed_late = done_rows
        .iter()
        .filter(|(due, done)| {
            match (
                due.as_deref().and_then(|s| DateTime::parse_from_rfc3339(s).ok()),
                done.as_deref().and_then(|s| DateTime::parse_from_rfc3339(s).ok()),
            ) {
                (Some(due), Some(done)) => done.with_timezone(&Utc) > due.with_timezone(&Utc),
                _ => false,
            }
        })
        .count() as i64;

    Ok(TaskOverview {
        total,
        completed,
        pending,
        overdue,
        completed_late,
    })
}

/// 最近完成的任务明细，按完成时间倒序。
pub fn recent_completed_tasks(conn: &Connection, limit: i64) -> Result<Vec<RecentTask>, String> {
    let limit = limit.clamp(1, 100);
    let rows: Vec<(i64, String, Option<String>, Option<String>)> = conn
        .prepare(
            "SELECT id, title, completed_at, due_at FROM tasks
             WHERE status = 'COMPLETED' AND completed_at IS NOT NULL
             ORDER BY completed_at DESC LIMIT ?1",
        )
        .map_err(|e| format!("查询最近完成任务失败: {e}"))?
        .query_map(params![limit], |r| {
            Ok((r.get(0)?, r.get(1)?, r.get(2)?, r.get(3)?))
        })
        .map_err(|e| format!("查询最近完成任务失败: {e}"))?
        .collect::<Result<_, _>>()
        .map_err(|e| format!("查询最近完成任务失败: {e}"))?;

    Ok(rows
        .into_iter()
        .map(|(id, title, completed_at, due_at)| {
            let late = match (
                due_at.as_deref().and_then(|s| DateTime::parse_from_rfc3339(s).ok()),
                completed_at
                    .as_deref()
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok()),
            ) {
                (Some(due), Some(done)) => done.with_timezone(&Utc) > due.with_timezone(&Utc),
                _ => false,
            };
            RecentTask {
                id,
                title,
                completed_at,
                due_at,
                late,
            }
        })
        .collect())
}

/// 截止时间是否已过（非法格式视为没过）。
fn is_past_due(due_at: Option<&str>, now: DateTime<Utc>) -> bool {
    due_at
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.with_timezone(&Utc) < now)
        .unwrap_or(false)
}

/// 最近 days 天（含今天）的本地日期，旧 → 新；返回（范围起点日期, 全部日期）。
fn day_range(days: i64) -> (NaiveDate, Vec<NaiveDate>) {
    let days = days.clamp(1, 366);
    let today = Local::now().date_naive();
    let start = today - chrono::Duration::days(days - 1);
    (
        start,
        (0..days).map(|i| start + chrono::Duration::days(i)).collect(),
    )
}

/// 本地某天零点对应的 UTC 时间。
fn day_start_utc(date: NaiveDate) -> DateTime<Utc> {
    date.and_hms_opt(0, 0, 0)
        .and_then(|t| Local.from_local_datetime(&t).earliest())
        .unwrap_or_else(Local::now)
        .with_timezone(&Utc)
}

/// 日期在桶序列里的下标（= 距范围起点的天数），不在范围内返回 None。
fn bucket_index(dates: &[NaiveDate], start: NaiveDate, date: &NaiveDate) -> Option<usize> {
    let offset = (*date - start).num_days();
    if offset >= 0 && (offset as usize) < dates.len() {
        Some(offset as usize)
    } else {
        None
    }
}
