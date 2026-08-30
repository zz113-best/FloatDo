use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum TaskStatus {
    Todo,
    InProgress,
    Completed,
    Overdue,
    Cancelled,
}

impl TaskStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            TaskStatus::Todo => "TODO",
            TaskStatus::InProgress => "IN_PROGRESS",
            TaskStatus::Completed => "COMPLETED",
            TaskStatus::Overdue => "OVERDUE",
            TaskStatus::Cancelled => "CANCELLED",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "TODO" => Ok(TaskStatus::Todo),
            "IN_PROGRESS" => Ok(TaskStatus::InProgress),
            "COMPLETED" => Ok(TaskStatus::Completed),
            "OVERDUE" => Ok(TaskStatus::Overdue),
            "CANCELLED" => Ok(TaskStatus::Cancelled),
            other => Err(format!("未知的任务状态: {other}")),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum Priority {
    Low,
    Medium,
    High,
    Urgent,
}

impl Priority {
    pub fn as_str(&self) -> &'static str {
        match self {
            Priority::Low => "LOW",
            Priority::Medium => "MEDIUM",
            Priority::High => "HIGH",
            Priority::Urgent => "URGENT",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "LOW" => Ok(Priority::Low),
            "MEDIUM" => Ok(Priority::Medium),
            "HIGH" => Ok(Priority::High),
            "URGENT" => Ok(Priority::Urgent),
            other => Err(format!("未知的优先级: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Task {
    pub id: i64,
    pub title: String,
    pub description: String,
    pub status: TaskStatus,
    pub priority: Priority,
    pub category_id: Option<i64>,
    pub tags: String,
    pub created_at: String,
    pub updated_at: String,
    pub due_at: Option<String>,
    pub completed_at: Option<String>,
    pub estimated_minutes: Option<i64>,
    pub reminder_enabled: bool,
    pub reminder_time: Option<String>,
    pub repeat_rule: Option<String>,
    pub sort_order: i64,
}

/// 创建任务时的输入（前端传入）。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskInput {
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub priority: Option<Priority>,
    pub category_id: Option<i64>,
    pub due_at: Option<String>,
    pub estimated_minutes: Option<i64>,
}

/// 任务记录查询（统计页表格）：关键词 + 独立的完成/逾期开关 + 两组日期范围 + 分页。
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskQuery {
    #[serde(default)]
    pub keyword: String,
    /// Some(true)=已完成 Some(false)=未完成 None=全部
    pub completed: Option<bool>,
    /// Some(true)=逾期（未完成且截止已过）Some(false)=未逾期 None=全部
    pub overdue: Option<bool>,
    /// 截止日期范围，YYYY-MM-DD 本地日期；设置了范围则无截止时间的任务不匹配
    pub due_from: Option<String>,
    pub due_to: Option<String>,
    /// 完成日期范围，YYYY-MM-DD 本地日期
    pub completed_from: Option<String>,
    pub completed_to: Option<String>,
    /// 按优先级筛选（URGENT/HIGH/MEDIUM/LOW，空 = 全部）
    pub priority: Option<String>,
    #[serde(default)]
    pub page: i64,
    #[serde(default)]
    pub page_size: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskPage {
    pub items: Vec<Task>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

/// 更新任务时的部分字段（只更新出现的字段）。
#[derive(Debug, Default, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskUpdate {
    pub title: Option<String>,
    pub description: Option<String>,
    pub status: Option<TaskStatus>,
    pub priority: Option<Priority>,
    pub category_id: Option<i64>,
    pub due_at: Option<String>,
    pub completed_at: Option<String>,
    pub estimated_minutes: Option<i64>,
    pub reminder_enabled: Option<bool>,
    pub reminder_time: Option<String>,
    pub repeat_rule: Option<String>,
    pub sort_order: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Category {
    pub id: i64,
    pub name: String,
    pub icon: String,
    pub is_default: bool,
    pub sort_order: i64,
}

/// 专注会话状态：进行中 / 完整完成 / 中途停止。
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum FocusStatus {
    #[serde(rename = "RUNNING")]
    Running,
    #[serde(rename = "COMPLETED")]
    Completed,
    #[serde(rename = "INTERRUPTED")]
    Interrupted,
}

impl FocusStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            FocusStatus::Running => "RUNNING",
            FocusStatus::Completed => "COMPLETED",
            FocusStatus::Interrupted => "INTERRUPTED",
        }
    }

    pub fn from_db(value: &str) -> Result<Self, String> {
        match value {
            "RUNNING" => Ok(FocusStatus::Running),
            "COMPLETED" => Ok(FocusStatus::Completed),
            "INTERRUPTED" => Ok(FocusStatus::Interrupted),
            other => Err(format!("未知的专注状态: {other}")),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FocusSession {
    pub id: i64,
    pub task_id: Option<i64>,
    pub started_at: String,
    pub ended_at: Option<String>,
    pub planned_minutes: i64,
    pub actual_seconds: i64,
    pub status: FocusStatus,
}
