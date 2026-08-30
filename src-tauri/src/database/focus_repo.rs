use super::models::{FocusSession, FocusStatus};
use chrono::Utc;
use rusqlite::{params, Connection, OptionalExtension};

fn map_row(row: &rusqlite::Row) -> rusqlite::Result<FocusSession> {
    let status_str: String = row.get("status")?;
    Ok(FocusSession {
        id: row.get("id")?,
        task_id: row.get("task_id")?,
        started_at: row.get("started_at")?,
        ended_at: row.get("ended_at")?,
        planned_minutes: row.get("planned_minutes")?,
        actual_seconds: row.get("actual_seconds")?,
        status: FocusStatus::from_db(&status_str).unwrap_or(FocusStatus::Interrupted),
    })
}

const COLS: &str =
    "id, task_id, started_at, ended_at, planned_minutes, actual_seconds, status";

/// 新建一条进行中的专注会话。
pub fn create_running(
    conn: &Connection,
    task_id: Option<i64>,
    planned_minutes: i64,
) -> Result<FocusSession, String> {
    let started_at = Utc::now().to_rfc3339();
    conn.execute(
        "INSERT INTO focus_sessions (task_id, started_at, planned_minutes, status)
         VALUES (?1, ?2, ?3, 'RUNNING')",
        params![task_id, started_at, planned_minutes],
    )
    .map_err(|e| format!("创建专注会话失败: {e}"))?;
    let id = conn.last_insert_rowid();
    get(conn, id)
}

pub fn get(conn: &Connection, id: i64) -> Result<FocusSession, String> {
    conn.query_row(
        &format!("SELECT {COLS} FROM focus_sessions WHERE id = ?1"),
        params![id],
        map_row,
    )
    .map_err(|e| format!("查询专注会话失败: {e}"))
}

/// 当前进行中的会话（最多一条，取最近开始的那次）。
pub fn running(conn: &Connection) -> Result<Option<FocusSession>, String> {
    conn.query_row(
        &format!(
            "SELECT {COLS} FROM focus_sessions WHERE status = 'RUNNING'
             ORDER BY started_at DESC LIMIT 1"
        ),
        [],
        map_row,
    )
    .optional()
    .map_err(|e| format!("查询进行中专注失败: {e}"))
}

/// 结束会话：actual_seconds 为实际专注秒数，status 为 COMPLETED / INTERRUPTED。
pub fn finish(
    conn: &Connection,
    id: i64,
    actual_seconds: i64,
    status: &FocusStatus,
) -> Result<(), String> {
    conn.execute(
        "UPDATE focus_sessions
         SET ended_at = ?2, actual_seconds = ?3, status = ?4
         WHERE id = ?1",
        params![id, Utc::now().to_rfc3339(), actual_seconds, status.as_str()],
    )
    .map_err(|e| format!("结束专注会话失败: {e}"))?;
    Ok(())
}

/// 汇总某时间点之后完成的专注秒数（用于「今日已专注」）。
pub fn completed_seconds_since(conn: &Connection, since_rfc3339: &str) -> Result<i64, String> {
    conn.query_row(
        "SELECT COALESCE(SUM(actual_seconds), 0) FROM focus_sessions
         WHERE status = 'COMPLETED' AND started_at >= ?1",
        params![since_rfc3339],
        |r| r.get(0),
    )
    .map_err(|e| format!("统计专注时长失败: {e}"))
}
