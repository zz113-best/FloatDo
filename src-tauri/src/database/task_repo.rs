use super::models::{Priority, Task, TaskInput, TaskPage, TaskQuery, TaskStatus, TaskUpdate};
use chrono::{DateTime, Local, Utc};
use rusqlite::{params, Connection, Row};

fn now() -> String {
    Utc::now().to_rfc3339()
}

fn map_row(row: &Row) -> rusqlite::Result<Task> {
    let status_str: String = row.get("status")?;
    let priority_str: String = row.get("priority")?;
    Ok(Task {
        id: row.get("id")?,
        title: row.get("title")?,
        description: row.get("description")?,
        status: TaskStatus::from_db(&status_str).unwrap_or(TaskStatus::Todo),
        priority: Priority::from_db(&priority_str).unwrap_or(Priority::Medium),
        category_id: row.get("category_id")?,
        tags: row.get("tags")?,
        created_at: row.get("created_at")?,
        updated_at: row.get("updated_at")?,
        due_at: row.get("due_at")?,
        completed_at: row.get("completed_at")?,
        estimated_minutes: row.get("estimated_minutes")?,
        reminder_enabled: row.get::<_, i64>("reminder_enabled")? != 0,
        reminder_time: row.get("reminder_time")?,
        repeat_rule: row.get("repeat_rule")?,
        sort_order: row.get("sort_order")?,
    })
}

pub fn list(conn: &Connection) -> Result<Vec<Task>, String> {
    let mut stmt = conn
        .prepare(
            "SELECT * FROM tasks ORDER BY sort_order ASC, id ASC",
        )
        .map_err(|e| format!("查询任务失败: {e}"))?;
    let tasks = stmt
        .query_map([], map_row)
        .map_err(|e| format!("查询任务失败: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(tasks)
}

/// 任务是否待完成。
fn is_pending(t: &Task) -> bool {
    matches!(t.status, TaskStatus::Todo | TaskStatus::InProgress)
}

/// 截止时间是否已过（RFC3339 有 "Z"/"+00:00" 两种格式，只能在 Rust 解析比较）。
fn is_overdue(t: &Task, now: DateTime<Utc>) -> bool {
    t.due_at
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc) < now)
        .unwrap_or(false)
}

/// 逾期完成：完成时间晚于截止时间（记录意义上的「逾期」，已完成任务也算）。
fn completed_late(t: &Task) -> bool {
    let (Some(due), Some(done)) = (t.due_at.as_deref(), t.completed_at.as_deref()) else {
        return false;
    };
    match (
        DateTime::parse_from_rfc3339(due).ok(),
        DateTime::parse_from_rfc3339(done).ok(),
    ) {
        (Some(due), Some(done)) => done.with_timezone(&Utc) > due.with_timezone(&Utc),
        _ => false,
    }
}

/// 记录意义上的逾期：未完成且截止已过，或逾期后才完成。
fn is_overdue_record(t: &Task, now: DateTime<Utc>) -> bool {
    (is_pending(t) && is_overdue(t, now)) || completed_late(t)
}

/// 时间戳（RFC3339）是否落在本地日期范围内。设置了范围而字段为空 → 不匹配。
fn stamp_in_range(
    stamp: Option<&str>,
    from: Option<chrono::NaiveDate>,
    to: Option<chrono::NaiveDate>,
) -> bool {
    if from.is_none() && to.is_none() {
        return true;
    }
    let Some(date) = stamp
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Local).date_naive())
    else {
        return false;
    };
    from.map(|f| date >= f).unwrap_or(true) && to.map(|t| date <= t).unwrap_or(true)
}

/// 统计页的任务记录：筛选 + 排序 + 分页。
/// 数据集是个人待办（量级很小），直接在内存里处理；
/// 时间比较不用 SQL 是因为库里 RFC3339 存在两种时区写法，字符串比较不可靠。
pub fn search(conn: &Connection, query: &TaskQuery) -> Result<TaskPage, String> {
    let mut filtered = filter_and_sort(conn, query)?;
    let total = filtered.len() as i64;
    let page = query.page.max(1);
    let page_size = query.page_size.clamp(1, 100);
    let start = ((page - 1) * page_size) as usize;
    let items = filtered
        .drain(..)
        .skip(start)
        .take(page_size as usize)
        .collect();
    Ok(TaskPage {
        items,
        total,
        page,
        page_size,
    })
}

/// 按条件筛选全部结果（不分页），CSV 导出用。
pub fn filtered_tasks(conn: &Connection, query: &TaskQuery) -> Result<Vec<Task>, String> {
    filter_and_sort(conn, query)
}

fn filter_and_sort(conn: &Connection, query: &TaskQuery) -> Result<Vec<Task>, String> {
    let tasks = list(conn)?;
    let now = Utc::now();
    let keyword = query.keyword.trim().to_lowercase();

    let due_from = parse_date(query.due_from.as_deref(), "开始截止日期")?;
    let due_to = parse_date(query.due_to.as_deref(), "结束截止日期")?;
    let completed_from = parse_date(query.completed_from.as_deref(), "开始完成日期")?;
    let completed_to = parse_date(query.completed_to.as_deref(), "结束完成日期")?;
    let priority_filter = query
        .priority
        .as_deref()
        .filter(|s| !s.is_empty())
        .map(Priority::from_db)
        .transpose()?;

    let mut filtered: Vec<&Task> = tasks
        .iter()
        .filter(|t| {
            if !keyword.is_empty()
                && !t.title.to_lowercase().contains(&keyword)
                && !t.description.to_lowercase().contains(&keyword)
            {
                return false;
            }
            if let Some(c) = query.completed {
                if matches!(t.status, TaskStatus::Completed) != c {
                    return false;
                }
            }
            if let Some(o) = query.overdue {
                if is_overdue_record(t, now) != o {
                    return false;
                }
            }
            if let Some(p) = &priority_filter {
                if t.priority != *p {
                    return false;
                }
            }
            if !stamp_in_range(t.due_at.as_deref(), due_from, due_to) {
                return false;
            }
            if !stamp_in_range(t.completed_at.as_deref(), completed_from, completed_to) {
                return false;
            }
            true
        })
        .collect();

    // 默认按创建时间新的在前（同批创建的按手动顺序）
    filtered.sort_by(|a, b| {
        b.created_at
            .cmp(&a.created_at)
            .then_with(|| a.sort_order.cmp(&b.sort_order))
    });
    Ok(filtered.into_iter().cloned().collect())
}

fn parse_date(value: Option<&str>, label: &str) -> Result<Option<chrono::NaiveDate>, String> {
    value
        .map(|s| chrono::NaiveDate::parse_from_str(s, "%Y-%m-%d"))
        .transpose()
        .map_err(|_| format!("{label}格式应为 YYYY-MM-DD"))
}

fn next_sort_order(conn: &Connection) -> Result<i64, String> {
    conn.query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM tasks", [], |r| {
        r.get(0)
    })
    .map_err(|e| format!("查询排序失败: {e}"))
}

pub fn create(conn: &Connection, input: &TaskInput) -> Result<Task, String> {
    if input.title.trim().is_empty() {
        return Err("任务标题不能为空".into());
    }
    let now = now();
    let sort_order = next_sort_order(conn)?;
    let priority = input
        .priority
        .as_ref()
        .map(|p| p.as_str())
        .unwrap_or("LOW");
    conn.execute(
        "INSERT INTO tasks (title, description, status, priority, category_id, tags, created_at, updated_at, due_at, estimated_minutes, sort_order)
         VALUES (?1, ?2, 'TODO', ?3, ?4, '[]', ?5, ?5, ?6, ?7, ?8)",
        params![
            input.title.trim(),
            input.description,
            priority,
            input.category_id,
            now,
            input.due_at,
            input.estimated_minutes,
            sort_order,
        ],
    )
    .map_err(|e| format!("创建任务失败: {e}"))?;
    let id = conn.last_insert_rowid();
    get(conn, id)
}

pub fn get(conn: &Connection, id: i64) -> Result<Task, String> {
    conn.query_row("SELECT * FROM tasks WHERE id = ?1", params![id], map_row)
        .map_err(|e| format!("任务不存在 (id={id}): {e}"))
}

pub fn update(conn: &Connection, id: i64, patch: &TaskUpdate) -> Result<Task, String> {
    let mut sets: Vec<String> = Vec::new();
    let mut values: Vec<Box<dyn rusqlite::ToSql>> = Vec::new();

    if let Some(v) = &patch.title {
        if v.trim().is_empty() {
            return Err("任务标题不能为空".into());
        }
        sets.push("title = ?".into());
        values.push(Box::new(v.trim().to_string()));
    }
    if let Some(v) = &patch.description {
        sets.push("description = ?".into());
        values.push(Box::new(v.clone()));
    }
    if let Some(v) = &patch.status {
        sets.push("status = ?".into());
        values.push(Box::new(v.as_str().to_string()));
        if matches!(v, TaskStatus::Completed) {
            sets.push("completed_at = ?".into());
            values.push(Box::new(now()));
        } else {
            sets.push("completed_at = ?".into());
            values.push(Box::new(Option::<String>::None));
        }
    }
    if let Some(v) = &patch.priority {
        sets.push("priority = ?".into());
        values.push(Box::new(v.as_str().to_string()));
    }
    if let Some(v) = patch.category_id {
        sets.push("category_id = ?".into());
        values.push(Box::new(v));
    }
    if let Some(v) = &patch.due_at {
        sets.push("due_at = ?".into());
        values.push(Box::new(v.clone()));
    }
    if let Some(v) = &patch.completed_at {
        sets.push("completed_at = ?".into());
        values.push(Box::new(v.clone()));
    }
    if let Some(v) = patch.estimated_minutes {
        sets.push("estimated_minutes = ?".into());
        values.push(Box::new(v));
    }
    if let Some(v) = patch.reminder_enabled {
        sets.push("reminder_enabled = ?".into());
        values.push(Box::new(if v { 1 } else { 0 }));
    }
    if let Some(v) = &patch.reminder_time {
        sets.push("reminder_time = ?".into());
        values.push(Box::new(v.clone()));
    }
    if let Some(v) = &patch.repeat_rule {
        sets.push("repeat_rule = ?".into());
        values.push(Box::new(v.clone()));
    }
    if let Some(v) = patch.sort_order {
        sets.push("sort_order = ?".into());
        values.push(Box::new(v));
    }

    if sets.is_empty() {
        return get(conn, id);
    }

    sets.push("updated_at = ?".into());
    values.push(Box::new(now()));
    values.push(Box::new(id));

    let sql = format!("UPDATE tasks SET {} WHERE id = ?", sets.join(", "));
    let params_ref: Vec<&dyn rusqlite::ToSql> = values.iter().map(|b| b.as_ref()).collect();
    conn.execute(&sql, params_ref.as_slice())
        .map_err(|e| format!("更新任务失败: {e}"))?;
    if conn.changes() == 0 {
        return Err(format!("任务不存在 (id={id})"));
    }
    get(conn, id)
}

pub fn delete(conn: &Connection, id: i64) -> Result<(), String> {
    conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])
        .map_err(|e| format!("删除任务失败: {e}"))?;
    if conn.changes() == 0 {
        return Err(format!("任务不存在 (id={id})"));
    }
    Ok(())
}

/// 按前端给定的顺序重写 sort_order（悬浮窗拖拽排序），事务保证原子性。
pub fn reorder(conn: &Connection, ordered_ids: &[i64]) -> Result<(), String> {
    conn.execute_batch("BEGIN")
        .map_err(|e| format!("开启事务失败: {e}"))?;
    let result = (|| {
        let mut stmt = conn
            .prepare("UPDATE tasks SET sort_order = ?1 WHERE id = ?2")
            .map_err(|e| format!("更新排序失败: {e}"))?;
        for (index, id) in ordered_ids.iter().enumerate() {
            stmt.execute(params![(index as i64) + 1, id])
                .map_err(|e| format!("更新排序失败: {e}"))?;
        }
        Ok(())
    })();
    match result {
        Ok(()) => conn
            .execute_batch("COMMIT")
            .map_err(|e| format!("提交事务失败: {e}")),
        Err(e) => {
            let _ = conn.execute_batch("ROLLBACK");
            Err(e)
        }
    }
}
