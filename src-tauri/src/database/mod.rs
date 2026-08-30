pub mod category_repo;
pub mod focus_repo;
pub mod migrations;
pub mod models;
pub mod settings_repo;
pub mod stats_repo;
pub mod task_repo;

use rusqlite::Connection;
use std::path::Path;

/// 打开（必要时创建）SQLite 数据库连接并执行迁移。
pub fn init<P: AsRef<Path>>(data_dir: P) -> Result<Connection, String> {
    let dir = data_dir.as_ref();
    std::fs::create_dir_all(dir).map_err(|e| format!("无法创建数据目录 {}: {e}", dir.display()))?;
    let db_path = dir.join("floatdo.db");
    let conn = Connection::open(&db_path)
        .map_err(|e| format!("无法打开数据库 {}: {e}", db_path.display()))?;
    conn.pragma_update(None, "journal_mode", "WAL")
        .map_err(|e| format!("设置 WAL 模式失败: {e}"))?;
    conn.pragma_update(None, "foreign_keys", "ON")
        .map_err(|e| format!("启用外键失败: {e}"))?;
    migrations::run(&conn)?;
    Ok(conn)
}
