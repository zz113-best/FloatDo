//! 到期提醒调度器：后台线程定期扫描任务表，把「即将到期 / 刚逾期」的任务
//! 推送到桌宠窗口弹气泡。已提醒过的 (任务, 截止时间, 类型) 记录在 settings 表，
//! 应用重启后也不会重复轰炸。

use crate::commands::DbState;
use crate::database::{models::Task, settings_repo, task_repo};
use chrono::{DateTime, Utc};
use serde::Serialize;
use std::collections::HashSet;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};

pub const EVENT_PET_REMINDER: &str = "pet://reminder";

/// 提前提醒时间的 settings 键与缺省值（可在设置页改）。
pub const REMINDER_LEAD_KEY: &str = "reminderLeadMinutes";
pub const DEFAULT_LEAD_MINUTES: f64 = 10.0;
/// 刚逾期 2 分钟内再提醒一次（固定，不可配置）。
const OVERDUE_GRACE_MINUTES: f64 = 2.0;
/// 扫描间隔。
const TICK_INTERVAL: Duration = Duration::from_secs(15);

const REMINDED_KEY: &str = "remindedKeys";
/// settings 表里 KV 是字符串，提醒记录太多时裁剪，防止无限增长。
const MAX_REMINDED_KEYS: usize = 500;

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetReminder {
    pub task_id: i64,
    pub title: String,
    /// DUE_SOON（即将到期）或 OVERDUE（已逾期）。
    pub kind: String,
    /// DUE_SOON 时的实际提前量（分钟，四舍五入），气泡文案要用。
    pub lead_minutes: i64,
}

/// 纯函数：给定任务列表、当前时间和已提醒集合，算出本次要提醒的任务。
/// 便于单元测试，不触碰数据库与窗口。
pub fn collect_due_reminders(
    tasks: &[Task],
    now: DateTime<Utc>,
    reminded: &HashSet<String>,
    lead_minutes: f64,
) -> Vec<PetReminder> {
    let mut out = Vec::new();
    for task in tasks {
        if matches!(task.status, crate::database::models::TaskStatus::Completed)
            || matches!(task.status, crate::database::models::TaskStatus::Cancelled)
        {
            continue;
        }
        let Some(due_str) = task.due_at.as_deref() else {
            continue;
        };
        let Ok(due) = DateTime::parse_from_rfc3339(due_str) else {
            continue;
        };
        let minutes_until_due = (due.with_timezone(&Utc) - now).num_seconds() as f64 / 60.0;

        let kind = if (0.0..=lead_minutes).contains(&minutes_until_due) {
            "DUE_SOON"
        } else if (-OVERDUE_GRACE_MINUTES..0.0).contains(&minutes_until_due) {
            "OVERDUE"
        } else {
            continue;
        };

        // key 里带上 kind：同一任务「即将到期」和「已逾期」各提醒一次
        let key = format!("{kind}:{}:{due_str}", task.id);
        if reminded.contains(&key) {
            continue;
        }
        out.push(PetReminder {
            task_id: task.id,
            title: task.title.clone(),
            kind: kind.to_string(),
            lead_minutes: lead_minutes.round() as i64,
        });
    }
    out
}

/// 在 setup 里调用：启动后台线程，每 15 秒检查一次到期任务。
pub fn spawn_scheduler(app: AppHandle) {
    std::thread::spawn(move || loop {
        std::thread::sleep(TICK_INTERVAL);
        if let Err(e) = tick(&app) {
            eprintln!("提醒调度失败: {e}");
        }
    });
}

fn tick(app: &AppHandle) -> Result<(), String> {
    let reminders = {
        let db = app.state::<DbState>();
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        let tasks = task_repo::list(&conn)?;

        let reminded: HashSet<String> = settings_repo::get(&conn, REMINDED_KEY)?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();

        // 每轮扫描时读一次提前量（15 秒一次，开销可忽略），改设置即刻生效
        let lead_minutes = settings_repo::get(&conn, REMINDER_LEAD_KEY)
            .ok()
            .flatten()
            .and_then(|v| v.parse::<f64>().ok())
            .map(|v| v.clamp(1.0, 10080.0))
            .unwrap_or(DEFAULT_LEAD_MINUTES);
        let reminders = collect_due_reminders(&tasks, Utc::now(), &reminded, lead_minutes);
        if reminders.is_empty() {
            return Ok(());
        }

        let mut reminded = reminded;
        for r in &reminders {
            reminded.insert(format!("{}:{}:{}", r.kind, r.task_id, task_due_at(&tasks, r.task_id)));
        }
        // 超限时只保留最新的记录
        if reminded.len() > MAX_REMINDED_KEYS {
            let keep: Vec<String> = reminded.into_iter().collect();
            reminded = keep[keep.len() - MAX_REMINDED_KEYS..].iter().cloned().collect();
        }
        settings_repo::set(&conn, REMINDED_KEY, &serde_json::to_string(&reminded).unwrap_or_default())?;
        reminders
    };

    for r in reminders {
        let _ = app.emit_to("pet", EVENT_PET_REMINDER, r);
    }
    Ok(())
}

fn task_due_at(tasks: &[Task], id: i64) -> String {
    tasks
        .iter()
        .find(|t| t.id == id)
        .and_then(|t| t.due_at.clone())
        .unwrap_or_default()
}
