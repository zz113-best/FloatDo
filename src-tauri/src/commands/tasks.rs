use super::DbState;
use crate::database::{
    models::{Task, TaskInput, TaskPage, TaskQuery, TaskUpdate},
    task_repo,
};
use tauri::{AppHandle, Emitter, Manager, State};

/// 任务有任何变化时通知相关窗口刷新（桌宠与 Todo 联动；悬浮窗是独立 store，也要重拉）。
const EVENT_PET_TASKS_CHANGED: &str = "pet://tasks-changed";
const EVENT_PET_TASK_COMPLETED: &str = "pet://task-completed";
/// 需要随任务变化刷新的窗口（桌宠 + 悬浮窗；主面板自己就是发起方）。
const TASKS_CHANGED_TARGETS: [&str; 2] = ["pet", "widget"];

fn emit_tasks_changed(app: &AppHandle) {
    for target in TASKS_CHANGED_TARGETS {
        let _ = app.emit_to(target, EVENT_PET_TASKS_CHANGED, ());
    }
}

#[tauri::command]
pub fn get_tasks(db: State<DbState>) -> Result<Vec<Task>, String> {
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    task_repo::list(&conn)
}

/// 任务记录查询：关键词 / 完成状态 / 日期范围筛选 + 分页（统计页表格用）。
#[tauri::command]
pub fn search_tasks(db: State<DbState>, query: TaskQuery) -> Result<TaskPage, String> {
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    task_repo::search(&conn, &query)
}

/// 打开主面板并切到「任务」页签（悬浮窗「显示全部」入口）。
#[tauri::command]
pub fn open_tasks(app: AppHandle) -> Result<(), String> {
    super::settings::open_panel(&app, "tasks")
}

/// 悬浮窗拖拽排序：按给定 id 顺序重写 sort_order。
#[tauri::command]
pub fn reorder_tasks(
    db: State<DbState>,
    app: AppHandle,
    ordered_ids: Vec<i64>,
) -> Result<(), String> {
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    task_repo::reorder(&conn, &ordered_ids)?;
    drop(conn);
    emit_tasks_changed(&app);
    Ok(())
}

fn csv_escape(field: &str) -> String {
    if field.contains(',') || field.contains('"') || field.contains('\n') || field.contains('\r') {
        format!("\"{}\"", field.replace('"', "\"\""))
    } else {
        field.to_string()
    }
}

/// 导出任务记录为 CSV（含 BOM，Excel 直接打开不乱码）。
/// 只导出按当前筛选条件选出的记录；用户取消返回 null。
#[tauri::command]
pub async fn export_tasks_csv(app: AppHandle, query: TaskQuery) -> Result<Option<String>, String> {
    tauri::async_runtime::spawn_blocking(move || {
        let Some(path) = rfd::FileDialog::new()
            .set_title("导出任务记录")
            .set_file_name("floatdo_tasks.csv")
            .add_filter("CSV 文件", &["csv"])
            .save_file()
        else {
            return Ok(None);
        };
        let db = app.state::<DbState>();
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        let tasks = task_repo::filtered_tasks(&conn, &query)?;
        drop(conn);

        let mut out = String::from(
            "\u{FEFF}编号,标题,描述,状态,优先级,截止时间,完成时间,创建时间,预计分钟\n",
        );
        for t in tasks {
            let fields = [
                t.id.to_string(),
                t.title.clone(),
                t.description.clone(),
                t.status.as_str().to_string(),
                t.priority.as_str().to_string(),
                t.due_at.clone().unwrap_or_default(),
                t.completed_at.clone().unwrap_or_default(),
                t.created_at.clone(),
                t.estimated_minutes.map(|v| v.to_string()).unwrap_or_default(),
            ];
            let line: Vec<String> = fields.iter().map(|f| csv_escape(f)).collect();
            out.push_str(&line.join(","));
            out.push('\n');
        }
        std::fs::write(&path, out).map_err(|e| format!("写入 CSV 失败: {e}"))?;
        Ok(Some(path.to_string_lossy().to_string()))
    })
    .await
    .map_err(|e| format!("导出线程失败: {e}"))?
}

#[tauri::command]
pub fn create_task(
    db: State<DbState>,
    app: AppHandle,
    input: TaskInput,
) -> Result<Task, String> {
    if input.title.trim().is_empty() {
        return Err("任务标题不能为空".into());
    }
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    let task = task_repo::create(&conn, &input)?;
    drop(conn);
    emit_tasks_changed(&app);
    Ok(task)
}

#[tauri::command]
pub fn update_task(
    db: State<DbState>,
    app: AppHandle,
    id: i64,
    patch: TaskUpdate,
) -> Result<Task, String> {
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    let task = task_repo::update(&conn, id, &patch)?;
    drop(conn);
    if patch.status.as_ref() == Some(&crate::database::models::TaskStatus::Completed) {
        let _ = app.emit_to("pet", EVENT_PET_TASK_COMPLETED, task.title.clone());
    }
    emit_tasks_changed(&app);
    Ok(task)
}

#[tauri::command]
pub fn delete_task(db: State<DbState>, app: AppHandle, id: i64) -> Result<(), String> {
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    task_repo::delete(&conn, id)?;
    drop(conn);
    emit_tasks_changed(&app);
    Ok(())
}
