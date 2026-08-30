use super::models::Category;
use rusqlite::{params, Connection, Row};

fn map_row(row: &Row) -> rusqlite::Result<Category> {
    Ok(Category {
        id: row.get("id")?,
        name: row.get("name")?,
        icon: row.get("icon")?,
        is_default: row.get::<_, i64>("is_default")? != 0,
        sort_order: row.get("sort_order")?,
    })
}

pub fn list(conn: &Connection) -> Result<Vec<Category>, String> {
    let mut stmt = conn
        .prepare("SELECT * FROM categories ORDER BY sort_order ASC, id ASC")
        .map_err(|e| format!("查询分类失败: {e}"))?;
    let categories = stmt
        .query_map([], map_row)
        .map_err(|e| format!("查询分类失败: {e}"))?
        .filter_map(|r| r.ok())
        .collect();
    Ok(categories)
}

pub fn create(conn: &Connection, name: &str, icon: &str) -> Result<Category, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("分类名称不能为空".into());
    }
    let sort_order: i64 = conn
        .query_row("SELECT COALESCE(MAX(sort_order), 0) + 1 FROM categories", [], |r| r.get(0))
        .map_err(|e| format!("查询分类排序失败: {e}"))?;
    conn.execute(
        "INSERT INTO categories (name, icon, is_default, sort_order) VALUES (?1, ?2, 0, ?3)",
        params![name, icon, sort_order],
    )
    .map_err(|e| format!("创建分类失败: {e}"))?;
    let id = conn.last_insert_rowid();
    conn.query_row("SELECT * FROM categories WHERE id = ?1", params![id], map_row)
        .map_err(|e| format!("分类不存在 (id={id}): {e}"))
}
