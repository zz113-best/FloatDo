use super::DbState;
use crate::database::category_repo;
use tauri::State;

#[tauri::command]
pub fn get_categories(
    db: State<DbState>,
) -> Result<Vec<crate::database::models::Category>, String> {
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    category_repo::list(&conn)
}

#[tauri::command]
pub fn create_category(
    db: State<DbState>,
    name: String,
    icon: String,
) -> Result<crate::database::models::Category, String> {
    let conn = db.0.lock().map_err(|_| "数据库被占用")?;
    category_repo::create(&conn, &name, &icon)
}
