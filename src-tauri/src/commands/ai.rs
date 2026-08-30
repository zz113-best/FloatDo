//! AI 桌宠对话：调用用户自配的 OpenAI 兼容接口（/chat/completions）。
//! 接口配置存 settings 表，API Key 只落本机 SQLite；未配置时功能整体关闭。
//! 「个性化」来自真实本地数据：系统提示词注入当前待办 / 逾期 / 今日专注，
//! 全部在 Rust 侧组装，前端只负责聊天界面。

use crate::commands::pet::PetPersonality;
use crate::commands::DbState;
use crate::database::{models::Priority, models::Task, models::TaskStatus, settings_repo, task_repo};
use chrono::{DateTime, Local, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager};

/// 桌宠窗口收到后把 AI 回复弹成气泡。
pub const EVENT_AI_REPLY: &str = "ai://reply";

const AI_BASE_URL_KEY: &str = "aiBaseUrl";
const AI_API_KEY_KEY: &str = "aiApiKey";
const AI_MODEL_KEY: &str = "aiModel";

/// 单次请求超时。
const REQUEST_TIMEOUT_SECS: u64 = 60;
/// 发送给模型的对话历史上限（条），超出丢最早的。
const MAX_HISTORY_MESSAGES: usize = 20;
/// 单条消息长度上限（字符），防止误传超大文本。
const MAX_MESSAGE_CHARS: usize = 4000;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AiConfig {
    pub base_url: String,
    pub model: String,
    /// 是否已配置 API Key（Key 本身不回传前端）
    pub has_api_key: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AiChatMessage {
    pub role: String,
    pub content: String,
}

fn read_kv(conn: &rusqlite::Connection, key: &str) -> Option<String> {
    settings_repo::get(conn, key).ok().flatten()
}

/// 接口地址：容忍末尾斜杠；用户直接填到 /chat/completions 也不重复追加。
fn chat_endpoint(base_url: &str) -> String {
    let base = base_url.trim().trim_end_matches('/');
    if base.ends_with("/chat/completions") {
        base.to_string()
    } else {
        format!("{base}/chat/completions")
    }
}

fn is_configured(base_url: &str, api_key: &str, model: &str) -> bool {
    !base_url.trim().is_empty() && !api_key.trim().is_empty() && !model.trim().is_empty()
}

#[tauri::command]
pub fn get_ai_config(app: AppHandle) -> Result<AiConfig, String> {
    let db = app
        .try_state::<DbState>()
        .ok_or_else(|| "数据库初始化中".to_string())?;
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    Ok(AiConfig {
        base_url: read_kv(&conn, AI_BASE_URL_KEY).unwrap_or_default(),
        model: read_kv(&conn, AI_MODEL_KEY).unwrap_or_default(),
        has_api_key: read_kv(&conn, AI_API_KEY_KEY)
            .map(|v| !v.trim().is_empty())
            .unwrap_or(false),
    })
}

/// 保存 AI 接口配置。api_key 传空串表示「保持不变」，避免每次保存都要重填。
#[tauri::command]
pub fn set_ai_config(
    app: AppHandle,
    base_url: String,
    api_key: String,
    model: String,
) -> Result<(), String> {
    let db = app
        .try_state::<DbState>()
        .ok_or_else(|| "数据库初始化中".to_string())?;
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    settings_repo::set(&conn, AI_BASE_URL_KEY, base_url.trim())?;
    settings_repo::set(&conn, AI_MODEL_KEY, model.trim())?;
    if !api_key.trim().is_empty() {
        settings_repo::set(&conn, AI_API_KEY_KEY, api_key.trim())?;
    }
    Ok(())
}

#[tauri::command]
pub async fn test_ai(app: AppHandle) -> Result<String, String> {
    let request = build_request(
        &app,
        vec![AiChatMessage {
            role: "user".into(),
            content: "请只回复两个字：在的".into(),
        }],
    )
    .await?;
    let reply = send(request).await?;
    Ok(reply)
}

/// 发送一轮对话，返回助手回复。messages 是聊天窗口里的可见历史。
#[tauri::command]
pub async fn ai_chat(app: AppHandle, messages: Vec<AiChatMessage>) -> Result<String, String> {
    let mut history: Vec<AiChatMessage> = messages
        .into_iter()
        .filter(|m| (m.role == "user" || m.role == "assistant") && !m.content.trim().is_empty())
        .collect();
    if history.is_empty() {
        return Err("消息为空".into());
    }
    for m in &mut history {
        m.content = m.content.chars().take(MAX_MESSAGE_CHARS).collect();
    }
    if history.len() > MAX_HISTORY_MESSAGES {
        history = history.split_off(history.len() - MAX_HISTORY_MESSAGES);
    }

    let request = build_request(&app, history).await?;
    let reply = send(request).await?;

    // 桌宠同步弹气泡：只取第一行、限长，气泡放不下长文
    let bubble: String = reply
        .lines()
        .find(|l| !l.trim().is_empty())
        .unwrap_or(&reply)
        .chars()
        .take(60)
        .collect();
    let _ = app.emit_to("pet", EVENT_AI_REPLY, bubble);
    Ok(reply)
}

/// 组装完整请求体：系统提示词（含真实本地上下文）+ 用户对话历史。
async fn build_request(
    app: &AppHandle,
    history: Vec<AiChatMessage>,
) -> Result<reqwest::RequestBuilder, String> {
    let (base_url, api_key, model, personality) = {
        let db = app
            .try_state::<DbState>()
            .ok_or_else(|| "数据库初始化中".to_string())?;
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        (
            read_kv(&conn, AI_BASE_URL_KEY).unwrap_or_default(),
            read_kv(&conn, AI_API_KEY_KEY).unwrap_or_default(),
            read_kv(&conn, AI_MODEL_KEY).unwrap_or_default(),
            PetPersonality::read_from(&conn),
        )
    };
    if !is_configured(&base_url, &api_key, &model) {
        return Err("还没有配置 AI 接口，请先到「设置 → AI 助手」填写接口地址、API Key 和模型名".into());
    }

    let context = {
        let db = app
            .try_state::<DbState>()
            .ok_or_else(|| "数据库初始化中".to_string())?;
        let conn = db.0.lock().map_err(|_| "数据库被占用")?;
        build_local_context(&conn)?
    };

    let mut messages = vec![AiChatMessage {
        role: "system".into(),
        content: format!("{}\n\n{context}", system_prompt(personality)),
    }];
    messages.extend(history);

    let body = serde_json::json!({
        "model": model.trim(),
        "messages": messages,
        "temperature": 0.7,
        "max_tokens": 600,
    });

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(REQUEST_TIMEOUT_SECS))
        .build()
        .map_err(|e| format!("初始化网络客户端失败: {e}"))?;
    Ok(client
        .post(chat_endpoint(&base_url))
        .bearer_auth(api_key.trim())
        .json(&body))
}

async fn send(request: reqwest::RequestBuilder) -> Result<String, String> {
    let response = request
        .send()
        .await
        .map_err(|e| format!("连接 AI 接口失败: {e}"))?;
    let status = response.status();
    let body = response
        .text()
        .await
        .map_err(|e| format!("读取 AI 接口响应失败: {e}"))?;
    if !status.is_success() {
        let snippet: String = body.chars().take(200).collect();
        return Err(format!("AI 接口返回 {status}: {snippet}"));
    }

    let parsed: serde_json::Value =
        serde_json::from_str(&body).map_err(|e| format!("解析 AI 响应失败: {e}"))?;
    let content = parsed["choices"][0]["message"]["content"]
        .as_str()
        .ok_or_else(|| {
            let snippet: String = body.chars().take(200).collect();
            format!("AI 响应里没有回复内容: {snippet}")
        })?;
    Ok(content.trim().to_string())
}

/// 各人格的开场人设（气泡文案与这里保持同一种语气）。
pub fn persona_prompt(personality: PetPersonality) -> &'static str {
    match personality {
        PetPersonality::Gentle => {
            "你是 FloatDo 的桌宠助手，一只住在用户桌面上的温柔小猫。语气亲切、体贴、会鼓励人，可以偶尔用「喵」。"
        }
        PetPersonality::Motivator => {
            "你是 FloatDo 的桌宠助手，一个热血的加油教练（小猫形象）。充满干劲、爱打气、多用感叹号，可以喊「冲呀」，但不要聒噪。"
        }
        PetPersonality::Sarcastic => {
            "你是 FloatDo 的桌宠助手，一只嘴硬心软的毒舌小猫。爱调侃用户的拖延和小懒惰，但绝不辱骂、不贬低人格，关键提醒要认真给。"
        }
        PetPersonality::Cool => {
            "你是 FloatDo 的桌宠助手，一只高冷的小猫。话少而精，一两句点破重点，不啰嗦、基本不用语气词和表情。"
        }
    }
}

const PROMPT_TAIL: &str = "回复保持简短（尽量不超过 100 字），不要用 markdown 格式，不要列表符号。用户给你的资料是此刻的真实数据，回答相关问题时直接引用；用户聊别的话题时也正常陪聊，保持角色。";

fn system_prompt(personality: PetPersonality) -> String {
    format!("{}{PROMPT_TAIL}", persona_prompt(personality))
}

/// 从 SQLite 拼装用户的真实上下文（待办 / 逾期 / 今日专注）。
fn build_local_context(conn: &rusqlite::Connection) -> Result<String, String> {
    let tasks = task_repo::list(conn)?;
    let now = Utc::now();
    let pending: Vec<&Task> = tasks
        .iter()
        .filter(|t| matches!(t.status, TaskStatus::Todo | TaskStatus::InProgress))
        .collect();
    let overdue = pending
        .iter()
        .filter(|t| {
            t.due_at
                .as_deref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|d| d.with_timezone(&Utc) < now)
                .unwrap_or(false)
        })
        .count();
    let completed = tasks
        .iter()
        .filter(|t| matches!(t.status, TaskStatus::Completed))
        .count();

    let midnight = Local::now()
        .date_naive()
        .and_hms_opt(0, 0, 0)
        .and_then(|t| Local.from_local_datetime(&t).earliest())
        .unwrap_or_else(Local::now)
        .with_timezone(&Utc);
    let focus_seconds = crate::database::focus_repo::completed_seconds_since(conn, &midnight.to_rfc3339())?;

    let mut lines = vec![format!(
        "- 今日待办 {} 个（其中逾期 {} 个），累计已完成 {} 个",
        pending.len(),
        overdue,
        completed
    )];
    if pending.is_empty() {
        lines.push("- 待办列表是空的".into());
    } else {
        lines.push("- 最靠前的几件事：".into());
        for line in pending_lines(&pending, now) {
            lines.push(format!("  {line}"));
        }
    }
    lines.push(format!(
        "- 今日专注 {} 分钟",
        focus_seconds / 60
    ));
    Ok(lines.join("\n"))
}

/// 纯函数：待办按「逾期 → 优先级 → 截止」取前 5 条并格式化，便于单元测试。
fn pending_lines(pending: &[&Task], now: DateTime<Utc>) -> Vec<String> {
    let priority_rank = |p: &Priority| match p {
        Priority::Urgent => 0,
        Priority::High => 1,
        Priority::Medium => 2,
        Priority::Low => 3,
    };
    let is_overdue = |t: &Task| {
        t.due_at
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|d| d.with_timezone(&Utc) < now)
            .unwrap_or(false)
    };
    let mut sorted: Vec<&&Task> = pending.iter().collect();
    sorted.sort_by(|a, b| {
        is_overdue(b)
            .cmp(&is_overdue(a))
            .then(priority_rank(&a.priority).cmp(&priority_rank(&b.priority)))
            .then(a.due_at.cmp(&b.due_at))
    });

    sorted
        .iter()
        .take(5)
        .map(|t| {
            let due = t
                .due_at
                .as_deref()
                .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                .map(|d| {
                    let local = d.with_timezone(&Local);
                    format!("，截止 {}", local.format("%m/%d %H:%M"))
                })
                .unwrap_or_default();
            let overdue_mark = if is_overdue(t) { "（已逾期）" } else { "" };
            format!(
                "「{}」（{:?}优先级{overdue_mark}{due}）",
                t.title, t.priority
            )
        })
        .collect()
}

/// 打开 AI 对话：主面板（settings 窗口）切到 chat 页签。
#[tauri::command]
pub fn open_chat(app: AppHandle) -> Result<(), String> {
    super::settings::open_panel(&app, "chat")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_prompt_follows_personality() {
        let prompts: Vec<String> = [
            PetPersonality::Gentle,
            PetPersonality::Motivator,
            PetPersonality::Sarcastic,
            PetPersonality::Cool,
        ]
        .iter()
        .map(|p| system_prompt(*p))
        .collect();
        // 四种人格各不相同，但都保留输出约束
        for (i, p) in prompts.iter().enumerate() {
            assert!(p.contains("不超过 100 字"));
            assert!(p.contains("保持角色"));
            for (j, q) in prompts.iter().enumerate() {
                if i != j {
                    assert_ne!(p, q, "人格 {} 与 {} 的提示词不应相同", i, j);
                }
            }
        }
        assert!(prompts[0].contains("温柔"));
        assert!(prompts[2].contains("毒舌"));
        assert!(prompts[3].contains("高冷"));
    }

    #[test]
    fn chat_endpoint_handles_suffixes() {
        assert_eq!(
            chat_endpoint("https://api.openai.com/v1"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            chat_endpoint("https://api.openai.com/v1/"),
            "https://api.openai.com/v1/chat/completions"
        );
        assert_eq!(
            chat_endpoint("https://x.com/v1/chat/completions"),
            "https://x.com/v1/chat/completions"
        );
    }

    #[test]
    fn pending_lines_sorts_overdue_first_and_caps_at_five() {
        let mk = |title: &str, priority: Priority, due_at: Option<String>| Task {
            id: 0,
            title: title.into(),
            description: String::new(),
            status: TaskStatus::Todo,
            priority,
            category_id: None,
            tags: "[]".into(),
            created_at: String::new(),
            updated_at: String::new(),
            due_at,
            completed_at: None,
            estimated_minutes: None,
            reminder_enabled: false,
            reminder_time: None,
            repeat_rule: None,
            sort_order: 0,
        };
        let now = Utc::now();
        let tasks = vec![
            mk("普通", Priority::Medium, None),
            mk("逾期", Priority::Low, Some((now - chrono::Duration::days(1)).to_rfc3339())),
            mk("紧急", Priority::Urgent, None),
        ];
        let refs: Vec<&Task> = tasks.iter().collect();
        let lines = pending_lines(&refs, now);
        assert_eq!(lines.len(), 3);
        assert!(lines[0].starts_with("「逾期」"), "逾期的排最前: {}", lines[0]);
        assert!(lines[0].contains("已逾期"));
        assert!(lines[1].starts_with("「紧急」"));
    }
}
