//! 数据库：rusqlite 连接 + refinery 迁移 + Task 的持久化。
//!
//! frequency / slots 用 JSON 文本列存（domain 类型 serde 派生）。

use std::sync::Mutex;

use dailyplan_domain::{Frequency, Task, TimeSlot};
use rusqlite::{params, Connection, Row};

/// 应用全局持有的 DB 连接（包 Mutex 给 Tauri State 跨命令共享）。
pub struct Db(pub Mutex<Connection>);

mod embedded_migrations {
    use refinery::embed_migrations;
    embed_migrations!("migrations");
}

/// 打开（或创建）DB 文件并跑迁移。
pub fn open_and_migrate(path: &std::path::Path) -> rusqlite::Result<Connection> {
    let mut conn = Connection::open(path)?;
    embedded_migrations::migrations::runner()
        .run(&mut conn)
        .map_err(|e| rusqlite::Error::ToSqlConversionFailure(Box::new(e)))?;
    Ok(conn)
}

// ===== 行 <-> domain 转换 =====

/// 把 JSON 列反序列化错误转成 rusqlite 错误。
/// FromSqlConversionFailure 需要 (列索引, 期望类型, 错误盒)，
/// 但反序列化失败时拿不到精确的列索引/类型，用 0 + Text 占位即可。
fn json_decode_err(column: &str, e: serde_json::Error) -> rusqlite::Error {
    // serde_json::Error 实现了 StdError，直接装盒；列名靠 wrap 携带。
    let wrapped: Box<dyn std::error::Error + Send + Sync + 'static> =
        format!("column `{column}` JSON 反序列化失败: {e}").into();
    rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, wrapped)
}

fn row_to_task(row: &Row<'_>) -> rusqlite::Result<Task> {
    let id: i64 = row.get("id")?;
    let name: String = row.get("name")?;
    let description: Option<String> = row.get("description")?;
    let frequency_json: String = row.get("frequency")?;
    let slots_json: String = row.get("slots")?;
    let priority_rank: i32 = row.get("priority")?;
    let active_int: i64 = row.get("active")?;

    let frequency: Frequency = serde_json::from_str(&frequency_json)
        .map_err(|e| json_decode_err("frequency", e))?;
    let slots: Vec<TimeSlot> = serde_json::from_str(&slots_json)
        .map_err(|e| json_decode_err("slots", e))?;

    Ok(Task {
        id,
        name,
        description,
        frequency,
        slots,
        priority_level: dailyplan_domain::PriorityLevel::from_rank(priority_rank),
        active: active_int != 0,
    })
}

// ===== CRUD =====

/// 列出所有任务（按 id 升序）。
pub fn list_tasks(db: &Db) -> rusqlite::Result<Vec<Task>> {
    let conn = db.0.lock().unwrap();
    let mut stmt =
        conn.prepare("SELECT id, name, description, frequency, slots, priority, active FROM tasks ORDER BY id")?;
    let rows = stmt.query_map([], row_to_task)?;
    let mut out = Vec::new();
    for r in rows {
        out.push(r?);
    }
    Ok(out)
}

/// 插入任务，返回带新 id 的 Task。
pub fn insert_task(db: &Db, task: &Task) -> rusqlite::Result<Task> {
    let conn = db.0.lock().unwrap();
    let frequency_json = serde_json::to_string(&task.frequency).unwrap();
    let slots_json = serde_json::to_string(&task.slots).unwrap();
    conn.execute(
        "INSERT INTO tasks (name, description, frequency, slots, priority, active)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            task.name,
            task.description,
            frequency_json,
            slots_json,
            task.priority_level.rank(),
            task.active as i64,
        ],
    )?;
    let id = conn.last_insert_rowid();
    Ok(Task { id: id as i64, ..task.clone() })
}

/// 按 id 更新任务（name/description/frequency/slots/priority/active）。
pub fn update_task(db: &Db, task: &Task) -> rusqlite::Result<()> {
    let conn = db.0.lock().unwrap();
    let frequency_json = serde_json::to_string(&task.frequency).unwrap();
    let slots_json = serde_json::to_string(&task.slots).unwrap();
    let affected = conn.execute(
        "UPDATE tasks SET
            name = ?1, description = ?2, frequency = ?3, slots = ?4,
            priority = ?5, active = ?6, updated_at = datetime('now')
         WHERE id = ?7",
        params![
            task.name,
            task.description,
            frequency_json,
            slots_json,
            task.priority_level.rank(),
            task.active as i64,
            task.id,
        ],
    )?;
    if affected == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

/// 按 id 删除任务。
pub fn delete_task(db: &Db, id: i64) -> rusqlite::Result<()> {
    let conn = db.0.lock().unwrap();
    let affected = conn.execute("DELETE FROM tasks WHERE id = ?1", params![id])?;
    if affected == 0 {
        return Err(rusqlite::Error::QueryReturnedNoRows);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::{NaiveDate, NaiveTime};
    use dailyplan_domain::{Frequency, PriorityLevel};

    fn in_mem_db() -> Db {
        let conn = open_and_migrate(&std::env::temp_dir().join("dailyplan_test.db")).unwrap();
        // 测试用内存库更干净——但 refinery 要文件，这里直接用临时文件并清表。
        conn.execute("DELETE FROM tasks", []).unwrap();
        Db(Mutex::new(conn))
    }

    fn slot(s: &str, e: &str) -> TimeSlot {
        TimeSlot {
            start: NaiveTime::parse_from_str(s, "%H:%M").unwrap(),
            end: NaiveTime::parse_from_str(e, "%H:%M").unwrap(),
        }
    }

    fn sample(name: &str) -> Task {
        Task {
            id: 0,
            name: name.into(),
            description: Some("测试".into()),
            frequency: Frequency::Weekly {
                weekdays: [true; 7],
            },
            slots: vec![slot("07:00", "07:30")],
            priority_level: PriorityLevel::High,
            active: true,
        }
    }

    #[test]
    fn insert_and_list() {
        let db = in_mem_db();
        let t = insert_task(&db, &sample("晨跑")).unwrap();
        assert!(t.id > 0);
        let all = list_tasks(&db).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].name, "晨跑");
        // 反序列化回来字段一致
        assert_eq!(all[0].priority_level, PriorityLevel::High);
        assert!(all[0].active);
        assert_eq!(all[0].slots.len(), 1);
    }

    #[test]
    fn update_changes_fields() {
        let db = in_mem_db();
        let mut t = insert_task(&db, &sample("读书")).unwrap();
        t.priority_level = PriorityLevel::Urgent;
        t.active = false;
        update_task(&db, &t).unwrap();
        let all = list_tasks(&db).unwrap();
        assert_eq!(all[0].priority_level, PriorityLevel::Urgent);
        assert!(!all[0].active);
    }

    #[test]
    fn delete_removes_row() {
        let db = in_mem_db();
        let t = insert_task(&db, &sample("冥想")).unwrap();
        delete_task(&db, t.id).unwrap();
        assert!(list_tasks(&db).unwrap().is_empty());
    }

    #[test]
    fn frequency_roundtrip_all_variants() {
        let db = in_mem_db();
        let variants = vec![
            Frequency::Daily { times_per_day: 3 },
            Frequency::Weekly { weekdays: [true, false, true, false, true, false, false] },
            Frequency::Interval { every_days: 4, start: NaiveDate::from_ymd_opt(2026, 7, 1).unwrap() },
            Frequency::Once { date: NaiveDate::from_ymd_opt(2026, 8, 8).unwrap() },
            Frequency::Custom {
                dates: vec![
                    NaiveDate::from_ymd_opt(2026, 7, 5).unwrap(),
                    NaiveDate::from_ymd_opt(2026, 7, 10).unwrap(),
                ],
            },
        ];
        for (i, f) in variants.into_iter().enumerate() {
            let t = insert_task(&db, &Task { id: 0, name: format!("t{i}"), description: None, frequency: f.clone(), slots: vec![], priority_level: PriorityLevel::Low, active: true }).unwrap();
            let all = list_tasks(&db).unwrap();
            let got = all.iter().find(|x| x.id == t.id).unwrap();
            assert_eq!(got.frequency, f, "频率变体 {i} 往返不一致");
        }
    }
}
