use rusqlite::Connection;

/// 第一版迁移：tasks / categories / settings。
/// 后续阶段（标签、提醒、专注、桌宠等）在此追加增量迁移。
pub fn run(conn: &Connection) -> Result<(), String> {
    conn.execute_batch(
        r#"
        CREATE TABLE IF NOT EXISTS categories (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL UNIQUE,
            icon        TEXT NOT NULL DEFAULT '',
            is_default  INTEGER NOT NULL DEFAULT 0,
            sort_order  INTEGER NOT NULL DEFAULT 0
        );

        CREATE TABLE IF NOT EXISTS tasks (
            id                INTEGER PRIMARY KEY AUTOINCREMENT,
            title             TEXT NOT NULL,
            description       TEXT NOT NULL DEFAULT '',
            status            TEXT NOT NULL DEFAULT 'TODO',
            priority          TEXT NOT NULL DEFAULT 'MEDIUM',
            category_id       INTEGER REFERENCES categories(id) ON DELETE SET NULL,
            tags              TEXT NOT NULL DEFAULT '[]',
            created_at        TEXT NOT NULL,
            updated_at        TEXT NOT NULL,
            due_at            TEXT,
            completed_at      TEXT,
            estimated_minutes INTEGER,
            reminder_enabled  INTEGER NOT NULL DEFAULT 0,
            reminder_time     TEXT,
            repeat_rule       TEXT,
            sort_order        INTEGER NOT NULL DEFAULT 0
        );

        CREATE INDEX IF NOT EXISTS idx_tasks_status ON tasks(status);
        CREATE INDEX IF NOT EXISTS idx_tasks_due_at ON tasks(due_at);

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        -- 阶段 4：专注模式。每次专注一轮记一条，status:
        -- RUNNING（进行中）/ COMPLETED（完整跑完）/ INTERRUPTED（中途停止）
        CREATE TABLE IF NOT EXISTS focus_sessions (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            task_id         INTEGER REFERENCES tasks(id) ON DELETE SET NULL,
            started_at      TEXT NOT NULL,
            ended_at        TEXT,
            planned_minutes INTEGER NOT NULL,
            actual_seconds  INTEGER NOT NULL DEFAULT 0,
            status          TEXT NOT NULL DEFAULT 'RUNNING'
        );

        CREATE INDEX IF NOT EXISTS idx_focus_sessions_started ON focus_sessions(started_at);
        "#,
    )
    .map_err(|e| format!("数据库迁移失败: {e}"))?;

    seed_default_categories(conn)?;
    Ok(())
}

fn seed_default_categories(conn: &Connection) -> Result<(), String> {
    let defaults = [
        ("收集箱", "📥", 0),
        ("重要", "⭐", 1),
        ("今天", "📅", 2),
        ("计划", "📆", 3),
        ("已完成", "✓", 4),
    ];
    for (name, icon, sort) in defaults {
        conn.execute(
            "INSERT OR IGNORE INTO categories (name, icon, is_default, sort_order) VALUES (?1, ?2, 1, ?3)",
            rusqlite::params![name, icon, sort],
        )
        .map_err(|e| format!("写入默认分类失败: {e}"))?;
    }
    Ok(())
}
