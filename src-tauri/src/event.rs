use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};

// ---------------------------------------------------------------------------
// Data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventRow {
    pub id: i64,
    pub event_type: String,
    pub title: String,
    pub description: String,
    pub status: String,
    pub reference_type: Option<String>,
    pub reference_id: Option<i64>,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EventListResult {
    pub events: Vec<EventRow>,
    pub total_count: i64,
    pub page: i64,
    pub page_size: i64,
    pub total_pages: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NotificationRow {
    pub id: i64,
    pub level: String,
    pub title: String,
    pub message: String,
    pub is_read: bool,
    pub reference_type: Option<String>,
    pub reference_id: Option<i64>,
    pub created_at: String,
}

#[derive(Debug, thiserror::Error)]
pub enum EventError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

// ---------------------------------------------------------------------------
// Events
// ---------------------------------------------------------------------------

pub fn create_event(
    conn: &Connection,
    event_type: &str,
    title: &str,
    description: &str,
    status: &str,
    reference_type: Option<&str>,
    reference_id: Option<i64>,
    metadata_json: Option<&str>,
) -> Result<i64, EventError> {
    conn.execute(
        "INSERT INTO events (event_type, title, description, status, reference_type, reference_id, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![event_type, title, description, status, reference_type, reference_id, metadata_json],
    )?;
    Ok(conn.last_insert_rowid())
}

fn query_events(
    conn: &Connection,
    where_clause: &str,
    page_size: i64,
    offset: i64,
    filter: Option<&str>,
) -> Result<Vec<EventRow>, EventError> {
    if let Some(f) = filter {
        let sql = format!(
            "SELECT id, event_type, title, description, status, reference_type, reference_id, metadata_json, created_at
             FROM events {where_clause}
             ORDER BY created_at DESC
             LIMIT ?2 OFFSET ?3"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![f, page_size, offset], map_event)?;
        let result = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(result)
    } else {
        let sql = format!(
            "SELECT id, event_type, title, description, status, reference_type, reference_id, metadata_json, created_at
             FROM events
             ORDER BY created_at DESC
             LIMIT ?1 OFFSET ?2"
        );
        let mut stmt = conn.prepare(&sql)?;
        let rows = stmt.query_map(params![page_size, offset], map_event)?;
        let result = rows.collect::<Result<Vec<_>, _>>()?;
        Ok(result)
    }
}

pub fn list_events(
    conn: &Connection,
    page: i64,
    page_size: i64,
    event_type: Option<&str>,
) -> Result<EventListResult, EventError> {
    let page = page.max(1);
    let page_size = page_size.clamp(1, 100);

    let (where_clause, filter) = if let Some(t) = event_type {
        ("WHERE event_type = ?1".to_owned(), Some(t.to_owned()))
    } else {
        (String::new(), None)
    };

    let count_sql = format!("SELECT COUNT(*) FROM events {where_clause}");
    let total_count: i64 = if let Some(ref f) = filter {
        conn.query_row(&count_sql, [f], |row| row.get(0))?
    } else {
        conn.query_row(&count_sql, [], |row| row.get(0))?
    };

    let total_pages = (total_count + page_size - 1) / page_size;
    let offset = (page - 1) * page_size;

    let events = query_events(conn, &where_clause, page_size, offset, filter.as_deref())?;

    Ok(EventListResult {
        events,
        total_count,
        page,
        page_size,
        total_pages,
    })
}

fn map_event(row: &rusqlite::Row<'_>) -> rusqlite::Result<EventRow> {
    Ok(EventRow {
        id: row.get(0)?,
        event_type: row.get(1)?,
        title: row.get(2)?,
        description: row.get(3)?,
        status: row.get(4)?,
        reference_type: row.get(5)?,
        reference_id: row.get(6)?,
        metadata_json: row.get(7)?,
        created_at: row.get(8)?,
    })
}

// ---------------------------------------------------------------------------
// Notifications
// ---------------------------------------------------------------------------

pub fn create_notification(
    conn: &Connection,
    level: &str,
    title: &str,
    message: &str,
    reference_type: Option<&str>,
    reference_id: Option<i64>,
) -> Result<i64, EventError> {
    conn.execute(
        "INSERT INTO notifications (level, title, message, reference_type, reference_id)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        params![level, title, message, reference_type, reference_id],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn list_notifications(conn: &Connection) -> Result<Vec<NotificationRow>, EventError> {
    let mut stmt = conn.prepare(
        "SELECT id, level, title, message, is_read, reference_type, reference_id, created_at
         FROM notifications
         ORDER BY created_at DESC
         LIMIT 100",
    )?;
    let rows = stmt
        .query_map([], |row| {
            Ok(NotificationRow {
                id: row.get(0)?,
                level: row.get(1)?,
                title: row.get(2)?,
                message: row.get(3)?,
                is_read: row.get::<_, i32>(4)? != 0,
                reference_type: row.get(5)?,
                reference_id: row.get(6)?,
                created_at: row.get(7)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(rows)
}

pub fn get_unread_notification_count(conn: &Connection) -> Result<i64, EventError> {
    let count: i64 =
        conn.query_row("SELECT COUNT(*) FROM notifications WHERE is_read = 0", [], |row| {
            row.get(0)
        })?;
    Ok(count)
}

pub fn mark_notification_read(conn: &Connection, id: i64) -> Result<(), EventError> {
    conn.execute("UPDATE notifications SET is_read = 1 WHERE id = ?1", [id])?;
    Ok(())
}

pub fn mark_all_notifications_read(conn: &Connection) -> Result<(), EventError> {
    conn.execute("UPDATE notifications SET is_read = 1 WHERE is_read = 0", [])?;
    Ok(())
}

pub fn dismiss_notification(conn: &Connection, id: i64) -> Result<(), EventError> {
    conn.execute("DELETE FROM notifications WHERE id = ?1", [id])?;
    Ok(())
}

pub fn delete_all_events(conn: &Connection) -> Result<usize, EventError> {
    let count = conn.execute("DELETE FROM events", [])?;
    Ok(count)
}

pub fn delete_all_notifications(conn: &Connection) -> Result<usize, EventError> {
    let count = conn.execute("DELETE FROM notifications", [])?;
    Ok(count)
}

// ---------------------------------------------------------------------------
// Integration helpers
// ---------------------------------------------------------------------------

/// Record an import event when files are imported.
pub fn record_import_event(
    conn: &Connection,
    file_count: usize,
    success_count: usize,
    duplicate_count: usize,
    failure_count: usize,
    source_paths: &[String],
) -> Result<(), EventError> {
    let status = if failure_count == 0 { "completed" } else { "completed" };
    let metadata = serde_json::json!({ "source_paths": source_paths });
    create_event(
        conn,
        "import",
        &format!("导入 {file_count} 个文件"),
        &format!("成功 {success_count}，重复 {duplicate_count}，失败 {failure_count}"),
        status,
        None,
        None,
        Some(&metadata.to_string()),
    )?;
    Ok(())
}

/// Record a recognition event.
pub fn record_recognition_event(
    conn: &Connection,
    invoice_id: i64,
    invoice_title: &str,
    success: bool,
    duration_ms: u128,
    model: &str,
    page_count: usize,
) -> Result<(), EventError> {
    if success {
        create_event(
            conn,
            "recognition",
            &format!("识别发票: {invoice_title}"),
            &format!("模型 {model}，耗时 {duration_ms}ms，{page_count} 页"),
            "completed",
            Some("invoice"),
            Some(invoice_id),
            None,
        )?;
    } else {
        create_event(
            conn,
            "recognition",
            &format!("识别失败: {invoice_title}"),
            &format!("模型 {model}，耗时 {duration_ms}ms"),
            "failed",
            Some("invoice"),
            Some(invoice_id),
            None,
        )?;
    }
    Ok(())
}

/// Record an agent action event.
pub fn record_agent_event(
    conn: &Connection,
    _action: &str,
    title: &str,
    description: &str,
    reference_type: Option<&str>,
    reference_id: Option<i64>,
) -> Result<(), EventError> {
    create_event(
        conn,
        "agent",
        title,
        description,
        "completed",
        reference_type,
        reference_id,
        None,
    )?;
    Ok(())
}

/// Create a notification for completed async tasks.
pub fn notify_task_completed(
    conn: &Connection,
    title: &str,
    message: &str,
    reference_type: Option<&str>,
    reference_id: Option<i64>,
) -> Result<(), EventError> {
    create_notification(conn, "info", title, message, reference_type, reference_id)?;
    Ok(())
}

/// Create a warning notification.
pub fn notify_warning(
    conn: &Connection,
    title: &str,
    message: &str,
) -> Result<(), EventError> {
    create_notification(conn, "warning", title, message, None, None)?;
    Ok(())
}

/// Create an error notification.
pub fn notify_error(
    conn: &Connection,
    title: &str,
    message: &str,
) -> Result<(), EventError> {
    create_notification(conn, "error", title, message, None, None)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::run_migrations;

    fn setup() -> Connection {
        let mut conn = Connection::open_in_memory().expect("open");
        run_migrations(&mut conn).expect("migrate");
        conn
    }

    #[test]
    fn create_and_list_events() {
        let conn = setup();
        create_event(&conn, "import", "导入 3 个文件", "成功 3 个", "completed", None, None, Some(r#"{"source_paths":["a.pdf","b.jpg"]}"#))
            .expect("create");
        create_event(
            &conn,
            "recognition",
            "识别发票",
            "ok",
            "completed",
            Some("invoice"),
            Some(1),
            None,
        )
        .expect("create");

        let result = list_events(&conn, 1, 20, None).expect("list");
        assert_eq!(result.total_count, 2);
        assert_eq!(result.events.len(), 2);
    }

    #[test]
    fn filter_events_by_type() {
        let conn = setup();
        create_event(&conn, "import", "t1", "d", "completed", None, None, None).expect("create");
        create_event(&conn, "agent", "t2", "d", "completed", None, None, None).expect("create");

        let result = list_events(&conn, 1, 20, Some("import")).expect("list");
        assert_eq!(result.total_count, 1);
    }

    #[test]
    fn notifications_crud() {
        let conn = setup();
        let id = create_notification(&conn, "info", "任务完成", "识别已完成", None, None)
            .expect("create");
        assert_eq!(get_unread_notification_count(&conn).expect("count"), 1);

        mark_notification_read(&conn, id).expect("mark");
        assert_eq!(get_unread_notification_count(&conn).expect("count"), 0);

        dismiss_notification(&conn, id).expect("dismiss");
        assert_eq!(list_notifications(&conn).expect("list").len(), 0);
    }

    #[test]
    fn test_mark_all_notifications_read() {
        let conn = setup();
        create_notification(&conn, "info", "t1", "m", None, None).expect("create");
        create_notification(&conn, "warning", "t2", "m", None, None).expect("create");
        assert_eq!(get_unread_notification_count(&conn).expect("count"), 2);

        super::mark_all_notifications_read(&conn).expect("mark all");
        assert_eq!(get_unread_notification_count(&conn).expect("count"), 0);
    }

    #[test]
    fn list_events_pagination() {
        let conn = setup();
        for i in 0..5 {
            create_event(&conn, "import", &format!("t{i}"), "d", "completed", None, None, None)
                .expect("create");
        }
        let page1 = list_events(&conn, 1, 2, None).expect("page1");
        assert_eq!(page1.events.len(), 2);
        assert_eq!(page1.total_pages, 3);
    }
}
