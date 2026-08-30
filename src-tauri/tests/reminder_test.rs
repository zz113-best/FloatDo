/// 提醒调度核心逻辑的测试：collect_due_reminders 是纯函数，
/// 直接构造 Task 验证「即将到期 / 刚逾期 / 已提醒过 / 已完成 / 无截止时间」各分支。
use floatdo_lib::database::models::{Priority, Task, TaskStatus};
use floatdo_lib::reminder::collect_due_reminders;
use chrono::{Duration, Utc};
use std::collections::HashSet;

fn make_task(id: i64, status: TaskStatus, due_at: Option<String>) -> Task {
    Task {
        id,
        title: format!("任务{id}"),
        description: String::new(),
        status,
        priority: Priority::Medium,
        category_id: None,
        tags: "[]".into(),
        created_at: "2026-08-29T00:00:00Z".into(),
        updated_at: "2026-08-29T00:00:00Z".into(),
        due_at,
        completed_at: None,
        estimated_minutes: None,
        reminder_enabled: false,
        reminder_time: None,
        repeat_rule: None,
        sort_order: 0,
    }
}

fn at(secs: &str) -> chrono::DateTime<Utc> {
    chrono::NaiveDateTime::parse_from_str(secs, "%Y-%m-%d %H:%M:%S")
        .unwrap()
        .and_utc()
}

#[test]
fn due_soon_task_gets_reminded() {
    let now = at("2026-08-29 10:00:00");
    // 5 分钟后到期 → DUE_SOON
    let task = make_task(1, TaskStatus::Todo, Some((now + Duration::minutes(5)).to_rfc3339()));
    let reminders = collect_due_reminders(&[task], now, &HashSet::new(), 10.0);
    assert_eq!(reminders.len(), 1);
    assert_eq!(reminders[0].kind, "DUE_SOON");
    assert_eq!(reminders[0].task_id, 1);
    assert_eq!(reminders[0].title, "任务1");
}

#[test]
fn just_overdue_task_gets_reminded() {
    let now = at("2026-08-29 10:00:00");
    // 逾期 1 分钟 → OVERDUE
    let task = make_task(2, TaskStatus::Todo, Some((now - Duration::minutes(1)).to_rfc3339()));
    let reminders = collect_due_reminders(&[task], now, &HashSet::new(), 10.0);
    assert_eq!(reminders.len(), 1);
    assert_eq!(reminders[0].kind, "OVERDUE");
}

#[test]
fn far_future_and_long_overdue_are_ignored() {
    let now = at("2026-08-29 10:00:00");
    let tasks = vec![
        // 明天才到期
        make_task(3, TaskStatus::Todo, Some((now + Duration::hours(24)).to_rfc3339())),
        // 早就逾期（老数据，不应轰炸）
        make_task(4, TaskStatus::Todo, Some((now - Duration::hours(24)).to_rfc3339())),
        // 没有截止时间
        make_task(5, TaskStatus::Todo, None),
        // 截止时间格式非法
        make_task(6, TaskStatus::Todo, Some("not-a-date".into())),
    ];
    assert!(collect_due_reminders(&tasks, now, &HashSet::new(), 10.0).is_empty());
}

#[test]
fn completed_and_cancelled_are_ignored() {
    let now = at("2026-08-29 10:00:00");
    let due = Some((now + Duration::minutes(5)).to_rfc3339());
    let tasks = vec![
        make_task(7, TaskStatus::Completed, due.clone()),
        make_task(8, TaskStatus::Cancelled, due),
    ];
    assert!(collect_due_reminders(&tasks, now, &HashSet::new(), 10.0).is_empty());
}

#[test]
fn already_reminded_keys_are_skipped() {
    let now = at("2026-08-29 10:00:00");
    let due = (now + Duration::minutes(5)).to_rfc3339();
    let task = make_task(9, TaskStatus::Todo, Some(due.clone()));

    let mut reminded = HashSet::new();
    reminded.insert(format!("DUE_SOON:9:{due}"));
    assert!(collect_due_reminders(&[task.clone()], now, &reminded, 10.0).is_empty());

    // 同一任务换成 OVERDUE key 后仍会提醒（各提醒一次）
    let mut overdue_key = HashSet::new();
    overdue_key.insert(format!("OVERDUE:9:{due}"));
    let reminders = collect_due_reminders(&[task], now, &overdue_key, 10.0);
    assert_eq!(reminders.len(), 1);
    assert_eq!(reminders[0].kind, "DUE_SOON");
}

#[test]
fn lead_minutes_controls_due_soon_window() {
    let now = Utc::now();
    // 半小时后到期
    let task = make_task(1, TaskStatus::Todo, Some((now + Duration::minutes(30)).to_rfc3339()));

    // 默认提前 10 分钟：还不到提醒的时候
    assert!(collect_due_reminders(&[task.clone()], now, &HashSet::new(), 10.0).is_empty());
    // 用户把提前量调成 60 分钟：立刻进入提醒窗口
    let reminders = collect_due_reminders(&[task], now, &HashSet::new(), 60.0);
    assert_eq!(reminders.len(), 1);
    assert_eq!(reminders[0].kind, "DUE_SOON");
    // 气泡文案要显示真实的提前量，而不是写死的 10
    assert_eq!(reminders[0].lead_minutes, 60);
}
