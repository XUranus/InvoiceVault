use rusqlite::Connection;

const MIGRATIONS: &[(i64, &str)] = &[
    (1, include_str!("../../migrations/0001_initial.sql")),
    (
        2,
        include_str!("../../migrations/0002_raw_current_name.sql"),
    ),
    (
        3,
        include_str!("../../migrations/0003_dedupe_candidates.sql"),
    ),
    (4, include_str!("../../migrations/0004_watch_dirs.sql")),
    (5, include_str!("../../migrations/0005_chroma_support.sql")),
    (6, include_str!("../../migrations/0006_agent_support.sql")),
    (
        7,
        include_str!("../../migrations/0007_invoice_embeddings.sql"),
    ),
    (
        8,
        include_str!("../../migrations/0008_events_notifications.sql"),
    ),
    (9, include_str!("../../migrations/0009_invoice_badges.sql")),
    (
        10,
        include_str!("../../migrations/0010_agent_attachments.sql"),
    ),
    (
        11,
        include_str!("../../migrations/0011_agent_tasks_artifacts.sql"),
    ),
    (
        12,
        include_str!("../../migrations/0012_invoice_extra_fields.sql"),
    ),
    (
        13,
        include_str!("../../migrations/0013_invoice_viewed_at.sql"),
    ),
    (
        14,
        include_str!("../../migrations/0014_usage_log.sql"),
    ),
    (
        15,
        include_str!("../../migrations/0015_email_sources.sql"),
    ),
    (
        16,
        include_str!("../../migrations/0016_pop3_support.sql"),
    ),
    (
        17,
        include_str!("../../migrations/0017_events_add_is_read.sql"),
    ),
    (
        18,
        include_str!("../../migrations/0018_agent_message_tool_call_id.sql"),
    ),
    (
        19,
        include_str!("../../migrations/0019_import_source_type.sql"),
    ),
];

#[derive(Debug, thiserror::Error)]
pub enum StorageError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
}

pub fn run_migrations(conn: &mut Connection) -> Result<(), StorageError> {
    conn.execute_batch(
        "CREATE TABLE IF NOT EXISTS schema_migrations (
            version INTEGER PRIMARY KEY,
            applied_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
        );",
    )?;

    let tx = conn.transaction()?;
    for (version, sql) in MIGRATIONS {
        let already_applied: bool = tx.query_row(
            "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE version = ?1)",
            [version],
            |row| row.get(0),
        )?;

        if !already_applied {
            tx.execute_batch(sql)?;
            tx.execute(
                "INSERT INTO schema_migrations (version) VALUES (?1)",
                [version],
            )?;
        }
    }
    tx.commit()?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migrations_are_idempotent() {
        let mut conn = Connection::open_in_memory().expect("open in-memory sqlite");

        run_migrations(&mut conn).expect("first migration run");
        run_migrations(&mut conn).expect("second migration run");

        let version: i64 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |row| row.get(0),
            )
            .expect("read migration version");

        assert_eq!(version, 19);
    }
}
