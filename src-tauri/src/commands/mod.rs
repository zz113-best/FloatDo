pub mod ai;
pub mod categories;
pub mod focus;
pub mod pet;
pub mod settings;
pub mod stats;
pub mod tasks;

use std::sync::Mutex;

/// 全局共享的数据库连接（单进程桌面应用，Mutex 足够）。
pub struct DbState(pub Mutex<rusqlite::Connection>);
