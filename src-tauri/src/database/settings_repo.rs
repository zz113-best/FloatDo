use rusqlite::{params, Connection};

pub fn get(conn: &Connection, key: &str) -> Result<Option<String>, String> {
    match conn.query_row("SELECT value FROM settings WHERE key = ?1", params![key], |r| {
        r.get::<_, String>(0)
    }) {
        Ok(v) => Ok(Some(v)),
        Err(rusqlite::Error::QueryReturnedNoRows) => Ok(None),
        Err(e) => Err(format!("读取设置失败 ({key}): {e}")),
    }
}

pub fn set(conn: &Connection, key: &str, value: &str) -> Result<(), String> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT(key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )
    .map_err(|e| format!("写入设置失败 ({key}): {e}"))?;
    Ok(())
}
