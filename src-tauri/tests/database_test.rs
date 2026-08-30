/// 数据层集成测试：直接对 repository 层跑真实 SQLite（临时文件），
/// 验证迁移、任务 CRUD、设置读写与持久化语义。
use floatdo_lib::database::{self, category_repo, models::TaskInput, models::{Priority, TaskStatus, TaskUpdate}, settings_repo, stats_repo, task_repo};
use rusqlite::params;

fn temp_conn() -> (rusqlite::Connection, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    let conn = database::init(dir.path()).expect("初始化数据库失败");
    (conn, dir)
}

#[test]
fn migration_seeds_default_categories() {
    let (conn, _dir) = temp_conn();
    let categories = category_repo::list(&conn).expect("读取分类失败");
    let names: Vec<&str> = categories.iter().map(|c| c.name.as_str()).collect();
    assert_eq!(names, vec!["收集箱", "重要", "今天", "计划", "已完成"]);
    assert!(categories.iter().all(|c| c.is_default));
}

#[test]
fn create_and_get_task() {
    let (conn, _dir) = temp_conn();
    let task = task_repo::create(
        &conn,
        &TaskInput {
            title: "完成项目方案".into(),
            description: String::new(),
            priority: Some(Priority::High),
            category_id: None,
            due_at: Some("2026-08-29T10:00:00Z".into()),
            estimated_minutes: Some(45),
        },
    )
    .expect("创建任务失败");

    assert_eq!(task.title, "完成项目方案");
    assert_eq!(task.status, TaskStatus::Todo);
    assert_eq!(task.priority, Priority::High);
    assert_eq!(task.due_at.as_deref(), Some("2026-08-29T10:00:00Z"));
    assert_eq!(task.estimated_minutes, Some(45));
    assert!(task.completed_at.is_none());

    let loaded = task_repo::get(&conn, task.id).expect("读取任务失败");
    assert_eq!(loaded.title, task.title);
}

#[test]
fn create_task_rejects_blank_title() {
    let (conn, _dir) = temp_conn();
    let result = task_repo::create(
        &conn,
        &TaskInput {
            title: "   ".into(),
            description: String::new(),
            priority: None,
            category_id: None,
            due_at: None,
            estimated_minutes: None,
        },
    );
    assert!(result.is_err(), "空白标题应被拒绝");
}

#[test]
fn update_task_fields_and_complete() {
    let (conn, _dir) = temp_conn();
    let task = task_repo::create(
        &conn,
        &TaskInput {
            title: "整理数据库代码".into(),
            description: String::new(),
            priority: None,
            category_id: None,
            due_at: None,
            estimated_minutes: None,
        },
    )
    .unwrap();

    let updated = task_repo::update(
        &conn,
        task.id,
        &TaskUpdate {
            priority: Some(Priority::Urgent),
            status: Some(TaskStatus::Completed),
            ..Default::default()
        },
    )
    .expect("更新任务失败");

    assert_eq!(updated.priority, Priority::Urgent);
    assert_eq!(updated.status, TaskStatus::Completed);
    assert!(updated.completed_at.is_some(), "完成时必须记录完成时间");

    // 重新打开为未完成时应清除完成时间
    let reopened = task_repo::update(
        &conn,
        task.id,
        &TaskUpdate {
            status: Some(TaskStatus::Todo),
            ..Default::default()
        },
    )
    .unwrap();
    assert!(reopened.completed_at.is_none());
}

#[test]
fn delete_task() {
    let (conn, _dir) = temp_conn();
    let task = task_repo::create(
        &conn,
        &TaskInput {
            title: "待删除".into(),
            description: String::new(),
            priority: None,
            category_id: None,
            due_at: None,
            estimated_minutes: None,
        },
    )
    .unwrap();
    task_repo::delete(&conn, task.id).expect("删除任务失败");
    assert!(task_repo::get(&conn, task.id).is_err());
    assert!(task_repo::delete(&conn, task.id).is_err(), "重复删除应报错");
}

#[test]
fn settings_roundtrip_and_persist() {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    {
        let conn = database::init(dir.path()).expect("初始化数据库失败");
        settings_repo::set(&conn, "theme", "dark").unwrap();
    }
    // 用新连接重新打开，验证真实落盘
    let conn2 = database::init(dir.path()).unwrap();
    assert_eq!(
        settings_repo::get(&conn2, "theme").unwrap().as_deref(),
        Some("dark")
    );
}

#[test]
fn tasks_persist_across_connections() {
    let dir = tempfile::tempdir().expect("创建临时目录失败");
    {
        let conn = database::init(dir.path()).unwrap();
        task_repo::create(
            &conn,
            &TaskInput {
                title: "持久化任务".into(),
                description: String::new(),
                priority: Some(Priority::Medium),
                category_id: None,
                due_at: None,
                estimated_minutes: None,
            },
        )
        .unwrap();
    }
    let conn2 = database::init(dir.path()).unwrap();
    let tasks = task_repo::list(&conn2).unwrap();
    assert_eq!(tasks.len(), 1);
    assert_eq!(tasks[0].title, "持久化任务");
}

/// 插入一条指定开始时间的已完成专注会话。
/// create_running 只会写「当前时间」，历史会话只能直接造数据模拟。
fn seed_focus_session(conn: &rusqlite::Connection, started_at: chrono::DateTime<chrono::Local>, actual_seconds: i64) {
    let started = started_at.to_rfc3339();
    conn.execute(
        "INSERT INTO focus_sessions (task_id, started_at, ended_at, planned_minutes, actual_seconds, status)
         VALUES (NULL, ?1, ?1, 25, ?2, 'COMPLETED')",
        params![started, actual_seconds],
    )
    .expect("插入测试专注会话失败");
}

#[test]
fn focus_daily_buckets_by_local_day() {
    let (conn, _dir) = temp_conn();
    // 用「今天中午」做基准：即使测试在午夜前后跑，各种子也不会跨天（now-2h 在凌晨会落到昨天）
    let noon = chrono::Local
        .from_local_datetime(
            &chrono::Local::now()
                .date_naive()
                .and_hms_opt(12, 0, 0)
                .expect("有效时间"),
        )
        .single()
        .expect("本地时间应唯一");
    use chrono::TimeZone as _;
    seed_focus_session(&conn, noon, 600);
    seed_focus_session(&conn, noon - chrono::Duration::hours(1), 900);
    seed_focus_session(&conn, noon - chrono::Duration::days(3), 1200);
    seed_focus_session(&conn, noon - chrono::Duration::days(40), 3000);

    let days = stats_repo::focus_daily(&conn, 7).expect("统计专注失败");
    assert_eq!(days.len(), 7, "应返回 7 个日期桶，空的天也保留");
    // 今天（最后一格）：2 轮共 1500 秒
    assert_eq!(days[6].sessions, 2);
    assert_eq!(days[6].focus_seconds, 1500);
    // 3 天前：1 轮 1200 秒
    assert_eq!(days[3].sessions, 1);
    assert_eq!(days[3].focus_seconds, 1200);
    // 其余天为空，40 天前超出范围不计入
    assert!(days[..3].iter().all(|d| d.sessions == 0));
    assert_eq!(days[4].sessions, 0);
    assert_eq!(days[5].sessions, 0);
}

#[test]
fn task_overview_and_daily_completed() {
    let (conn, _dir) = temp_conn();
    let create = |title: &str, due_at: Option<String>| {
        task_repo::create(
            &conn,
            &TaskInput {
                title: title.into(),
                description: String::new(),
                priority: None,
                category_id: None,
                due_at,
                estimated_minutes: None,
            },
        )
        .unwrap()
    };
    let done = create("已完成任务", None);
    create("逾期任务", Some((chrono::Utc::now() - chrono::Duration::days(1)).to_rfc3339()));
    create("正常待办", Some((chrono::Utc::now() + chrono::Duration::days(1)).to_rfc3339()));
    task_repo::update(&conn, done.id, &TaskUpdate { status: Some(TaskStatus::Completed), ..Default::default() }).unwrap();

    let overview = stats_repo::task_overview(&conn).expect("统计任务总览失败");
    assert_eq!(overview.total, 3);
    assert_eq!(overview.completed, 1);
    assert_eq!(overview.pending, 2);
    assert_eq!(overview.overdue, 1);

    // 今天完成的那条任务应落在最后一格
    let days = stats_repo::task_daily_completed(&conn, 7).expect("统计任务按日失败");
    assert_eq!(days.len(), 7);
    assert_eq!(days[6].completed, 1);
    assert!(days[..6].iter().all(|d| d.completed == 0));
}

#[test]
fn search_tasks_filters_sorts_and_paginates() {
    use floatdo_lib::database::models::TaskQuery;

    fn q(page: i64, page_size: i64) -> TaskQuery {
        TaskQuery {
            keyword: String::new(),
            completed: None,
            overdue: None,
            due_from: None,
            due_to: None,
            completed_from: None,
            completed_to: None,
            priority: None,
            page,
            page_size,
        }
    }

    let (conn, _dir) = temp_conn();
    let mk = |title: &str, priority: Priority, due_at: Option<String>| TaskInput {
        title: title.into(),
        description: String::new(),
        priority: Some(priority),
        category_id: None,
        due_at,
        estimated_minutes: None,
    };
    let report = task_repo::create(&conn, &mk("写周报", Priority::Medium, Some("2026-08-20T10:00:00Z".into()))).unwrap();
    let tea = task_repo::create(&conn, &mk("买奶茶", Priority::Low, None)).unwrap();
    let rent = task_repo::create(&conn, &mk("交房租", Priority::Urgent, Some("2026-08-01T00:00:00Z".into()))).unwrap();
    task_repo::update(
        &conn,
        report.id,
        &TaskUpdate {
            status: Some(TaskStatus::Completed),
            completed_at: Some("2026-08-20T12:00:00Z".into()),
            ..Default::default()
        },
    )
    .unwrap();

    // 默认按创建时间新的在前
    let page = task_repo::search(&conn, &q(1, 10)).unwrap();
    assert_eq!(page.total, 3);
    assert_eq!(page.items.iter().map(|t| t.id).collect::<Vec<_>>(), vec![rent.id, tea.id, report.id]);

    // 关键词命中标题
    let page = task_repo::search(&conn, &TaskQuery { keyword: "奶茶".into(), ..q(1, 10) }).unwrap();
    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].title, "买奶茶");

    // 按优先级筛选
    let page = task_repo::search(&conn, &TaskQuery { priority: Some("URGENT".into()), ..q(1, 10) }).unwrap();
    assert_eq!(page.items.iter().map(|t| t.id).collect::<Vec<_>>(), vec![rent.id]);

    // 完成 / 未完成（独立开关）
    let page = task_repo::search(&conn, &TaskQuery { completed: Some(true), ..q(1, 10) }).unwrap();
    assert_eq!(page.items.iter().map(|t| t.id).collect::<Vec<_>>(), vec![report.id]);
    let page = task_repo::search(&conn, &TaskQuery { completed: Some(false), ..q(1, 10) }).unwrap();
    assert_eq!(page.total, 2);

    // 逾期 / 未逾期（独立开关）：未完成且截止已过，或逾期后才完成都算「已逾期」
    // （rent 未完成已过期；report 完成时间 12:00 晚于截止 10:00，逾期完成）
    let page = task_repo::search(&conn, &TaskQuery { overdue: Some(true), ..q(1, 10) }).unwrap();
    assert_eq!(page.items.iter().map(|t| t.id).collect::<Vec<_>>(), vec![rent.id, report.id]);
    let page = task_repo::search(&conn, &TaskQuery { overdue: Some(false), ..q(1, 10) }).unwrap();
    assert_eq!(page.items.iter().map(|t| t.id).collect::<Vec<_>>(), vec![tea.id]);

    // 分页每页 2 条
    let page = task_repo::search(&conn, &q(1, 2)).unwrap();
    assert_eq!(page.items.iter().map(|t| t.id).collect::<Vec<_>>(), vec![rent.id, tea.id]);
    let page = task_repo::search(&conn, &q(2, 2)).unwrap();
    assert_eq!(page.items.iter().map(|t| t.id).collect::<Vec<_>>(), vec![report.id]);

    // 截止日期范围：按截止时间，无截止时间的不匹配
    let page = task_repo::search(&conn, &TaskQuery {
        due_from: Some("2026-08-15".into()),
        due_to: Some("2026-08-25".into()),
        ..q(1, 10)
    })
    .unwrap();
    assert_eq!(page.items.iter().map(|t| t.id).collect::<Vec<_>>(), vec![report.id]);

    // 完成日期范围：按完成时间
    let page = task_repo::search(&conn, &TaskQuery {
        completed_from: Some("2026-08-15".into()),
        completed_to: Some("2026-08-25".into()),
        ..q(1, 10)
    })
    .unwrap();
    assert_eq!(page.items.iter().map(|t| t.id).collect::<Vec<_>>(), vec![report.id]);
}
