use rusqlite::{params, Connection, OptionalExtension};
use serde::Serialize;

use crate::chroma::SimilarResult;

#[derive(Debug, Clone, Serialize)]
pub struct DedupeCheckResult {
    pub invoice_id: i64,
    pub candidates: Vec<DedupeCandidate>,
    pub has_exact_duplicate: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct DedupeCandidate {
    pub id: i64,
    pub candidate_invoice_id: i64,
    pub seller_name: Option<String>,
    pub invoice_number: Option<String>,
    pub issue_date: Option<String>,
    pub total_amount: Option<String>,
    pub score: f64,
    pub reason: String,
    pub status: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DedupeError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("extractor error: {0}")]
    Extractor(#[from] crate::extractor::ExtractorError),
}

#[derive(Debug, serde::Deserialize)]
pub struct ResolveDuplicateRequest {
    pub dedupe_id: i64,
    pub action: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolveDuplicateResult {
    pub action: String,
    pub deleted_invoice_id: Option<i64>,
}

pub fn check_invoice_duplicates(
    conn: &Connection,
    invoice_id: i64,
) -> Result<DedupeCheckResult, DedupeError> {
    detect_field_duplicates(conn, invoice_id)?;

    let mut stmt = conn.prepare(
        "SELECT
            dc.id, dc.candidate_invoice_id, dc.score, dc.reason, dc.status,
            inv.seller_name, inv.invoice_number, inv.issue_date, inv.total_amount
        FROM dedupe_candidates dc
        JOIN invoices inv ON inv.id = dc.candidate_invoice_id
        WHERE dc.invoice_id = ?1
        ORDER BY dc.score DESC",
    )?;

    let candidates: Vec<DedupeCandidate> = stmt
        .query_map([invoice_id], |row| {
            Ok(DedupeCandidate {
                id: row.get(0)?,
                candidate_invoice_id: row.get(1)?,
                score: row.get(2)?,
                reason: row.get(3)?,
                status: row.get(4)?,
                seller_name: row.get(5)?,
                invoice_number: row.get(6)?,
                issue_date: row.get(7)?,
                total_amount: row.get(8)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;

    let has_exact_duplicate = candidates.iter().any(|c| c.score >= 95.0);

    Ok(DedupeCheckResult {
        invoice_id,
        candidates,
        has_exact_duplicate,
    })
}

pub fn resolve_duplicate(
    conn: &Connection,
    request: ResolveDuplicateRequest,
) -> Result<ResolveDuplicateResult, DedupeError> {
    let valid_actions = [
        "confirm",
        "ignore",
        "keep_current",
        "keep_other",
        "keep_both",
    ];
    if !valid_actions.contains(&request.action.as_str()) {
        return Ok(ResolveDuplicateResult {
            action: request.action,
            deleted_invoice_id: None,
        });
    }

    // Look up the dedupe candidate pair
    let (invoice_id, candidate_invoice_id): (i64, i64) = conn.query_row(
        "SELECT invoice_id, candidate_invoice_id FROM dedupe_candidates WHERE id = ?1",
        [request.dedupe_id],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    match request.action.as_str() {
        "confirm" | "ignore" => {
            conn.execute(
                "UPDATE dedupe_candidates SET status = ?2, resolved_at = CURRENT_TIMESTAMP WHERE id = ?1",
                params![request.dedupe_id, request.action],
            )?;
            if request.action == "confirm" {
                conn.execute(
                    "UPDATE invoices SET duplicate_status = 'probable_duplicate', updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
                    [invoice_id],
                )?;
            } else {
                recalc_duplicate_status(conn, invoice_id)?;
            }
            Ok(ResolveDuplicateResult {
                action: request.action,
                deleted_invoice_id: None,
            })
        }
        "keep_current" => {
            // Delete the candidate (other) invoice, mark current as unique
            let tx = conn.unchecked_transaction()?;
            crate::extractor::batch_delete_invoices(&tx, &[candidate_invoice_id])?;
            // Clean up any remaining dedupe candidates referencing deleted invoice
            tx.execute(
                "DELETE FROM dedupe_candidates WHERE invoice_id = ?1 OR candidate_invoice_id = ?1",
                [candidate_invoice_id],
            )?;
            recalc_duplicate_status(&tx, invoice_id)?;
            tx.commit()?;
            Ok(ResolveDuplicateResult {
                action: "keep_current".into(),
                deleted_invoice_id: Some(candidate_invoice_id),
            })
        }
        "keep_other" => {
            // Delete the current invoice, return candidate ID for navigation
            let tx = conn.unchecked_transaction()?;
            crate::extractor::batch_delete_invoices(&tx, &[invoice_id])?;
            tx.execute(
                "DELETE FROM dedupe_candidates WHERE invoice_id = ?1 OR candidate_invoice_id = ?1",
                [invoice_id],
            )?;
            recalc_duplicate_status(&tx, candidate_invoice_id)?;
            tx.commit()?;
            Ok(ResolveDuplicateResult {
                action: "keep_other".into(),
                deleted_invoice_id: Some(invoice_id),
            })
        }
        "keep_both" => {
            // Mark bidirectional candidates as not_duplicate
            let tx = conn.unchecked_transaction()?;
            tx.execute(
                "UPDATE dedupe_candidates SET status = 'not_duplicate', resolved_at = CURRENT_TIMESTAMP
                 WHERE (invoice_id = ?1 AND candidate_invoice_id = ?2)
                    OR (invoice_id = ?2 AND candidate_invoice_id = ?1)",
                params![invoice_id, candidate_invoice_id],
            )?;
            recalc_duplicate_status(&tx, invoice_id)?;
            recalc_duplicate_status(&tx, candidate_invoice_id)?;
            tx.commit()?;
            Ok(ResolveDuplicateResult {
                action: "keep_both".into(),
                deleted_invoice_id: None,
            })
        }
        _ => unreachable!(),
    }
}

fn detect_field_duplicates(conn: &Connection, invoice_id: i64) -> Result<(), DedupeError> {
    let (invoice_code, invoice_number, issue_date, total_amount, seller_name, _buyer_name): (
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
        Option<String>,
    ) = conn.query_row(
        "SELECT invoice_code, invoice_number, issue_date, total_amount, seller_name, buyer_name
        FROM invoices WHERE id = ?1",
        [invoice_id],
        |row| {
            Ok((
                row.get(0)?,
                row.get(1)?,
                row.get(2)?,
                row.get(3)?,
                row.get(4)?,
                row.get(5)?,
            ))
        },
    )?;

    let mut candidates: Vec<(i64, f64, &str)> = Vec::new();

    // Rule 1: invoice_code + invoice_number exact match → score 95
    if let (Some(ref code), Some(ref num)) = (&invoice_code, &invoice_number) {
        if !code.is_empty() && !num.is_empty() {
            let mut stmt = conn.prepare(
                "SELECT id FROM invoices
                WHERE id != ?1 AND invoice_code = ?2 AND invoice_number = ?3",
            )?;
            let ids: Vec<i64> = stmt
                .query_map(params![invoice_id, code, num], |row| row.get(0))?
                .collect::<Result<Vec<_>, _>>()?;
            for id in ids {
                candidates.push((id, 95.0, "field_match"));
            }
        }
    }

    // Rule 2: at least 3 of {number, date, amount, seller} match → score 80
    let number_match = invoice_number.as_deref().filter(|s| !s.is_empty());
    let date_match = issue_date.as_deref().filter(|s| !s.is_empty());
    let amount_match = total_amount.as_deref().filter(|s| !s.is_empty());
    let seller_match = seller_name.as_deref().filter(|s| !s.is_empty());

    if number_match.is_some()
        || date_match.is_some()
        || amount_match.is_some()
        || seller_match.is_some()
    {
        let mut sql = String::from(
            "SELECT id, invoice_number, issue_date, total_amount, seller_name FROM invoices WHERE id != ?1",
        );
        let mut vals: Vec<Box<dyn rusqlite::types::ToSql>> = vec![Box::new(invoice_id)];

        // Exclude already-looked-at exact matches
        let existing_ids: Vec<i64> = candidates.iter().map(|(id, _, _)| *id).collect();
        for (_i, id) in existing_ids.iter().enumerate() {
            sql.push_str(&format!(" AND id != ?{}", vals.len() + 1));
            vals.push(Box::new(*id));
        }

        let mut stmt = conn.prepare(&sql)?;
        let refs: Vec<&dyn rusqlite::types::ToSql> = vals.iter().map(|v| v.as_ref()).collect();
        let rows: Vec<(
            i64,
            Option<String>,
            Option<String>,
            Option<String>,
            Option<String>,
        )> = stmt
            .query_map(refs.as_slice(), |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;

        for (id, row_num, row_date, row_amount, row_seller) in rows {
            let mut matches = 0u32;

            if let (Some(n1), Some(n2)) = (number_match, &row_num) {
                if n1 == n2 {
                    matches += 1;
                }
            }
            if let (Some(d1), Some(d2)) = (date_match, &row_date) {
                if d1 == d2 {
                    matches += 1;
                }
            }
            if let (Some(a1), Some(a2)) = (amount_match, &row_amount) {
                if a1 == a2 {
                    matches += 1;
                }
            }
            if let (Some(s1), Some(s2)) = (seller_match, &row_seller) {
                if s1 == s2 {
                    matches += 1;
                }
            }

            let score = if matches >= 3 {
                80.0
            } else if matches >= 2 {
                60.0
            } else {
                continue;
            };

            candidates.push((id, score, "field_match"));
        }
    }

    // Upsert candidates and update duplicate_status
    for (candidate_id, score, reason) in &candidates {
        conn.execute(
            "INSERT INTO dedupe_candidates (invoice_id, candidate_invoice_id, score, reason, status)
            VALUES (?1, ?2, ?3, ?4, 'open')
            ON CONFLICT(invoice_id, candidate_invoice_id) DO UPDATE SET
                score = excluded.score,
                reason = excluded.reason,
                status = CASE WHEN dedupe_candidates.status IN ('confirm', 'ignore') THEN dedupe_candidates.status ELSE 'open' END",
            params![invoice_id, candidate_id, score, reason],
        )?;
    }

    recalc_duplicate_status(conn, invoice_id)?;

    Ok(())
}

pub fn detect_semantic_duplicates(
    conn: &Connection,
    invoice_id: i64,
    similar: &[SimilarResult],
) -> Result<(), DedupeError> {
    for result in similar {
        if result.invoice_id == invoice_id {
            continue;
        }

        let score = if result.similarity >= 0.92 {
            80.0
        } else if result.similarity >= 0.85 {
            60.0
        } else {
            continue;
        };

        conn.execute(
            "INSERT INTO dedupe_candidates (invoice_id, candidate_invoice_id, score, reason, status)
            VALUES (?1, ?2, ?3, 'semantic', 'open')
            ON CONFLICT(invoice_id, candidate_invoice_id) DO UPDATE SET
                score = MAX(dedupe_candidates.score, excluded.score),
                reason = CASE WHEN dedupe_candidates.score >= excluded.score THEN dedupe_candidates.reason ELSE 'semantic' END,
                status = CASE WHEN dedupe_candidates.status IN ('confirm', 'ignore') THEN dedupe_candidates.status ELSE 'open' END",
            params![invoice_id, result.invoice_id, score],
        )?;
    }

    recalc_duplicate_status(conn, invoice_id)?;
    Ok(())
}

fn recalc_duplicate_status(conn: &Connection, invoice_id: i64) -> Result<(), DedupeError> {
    let max_score: Option<f64> = conn
        .query_row(
            "SELECT MAX(score) FROM dedupe_candidates
            WHERE invoice_id = ?1 AND status = 'open'",
            [invoice_id],
            |row| row.get(0),
        )
        .optional()?
        .flatten();

    let status = match max_score {
        Some(score) if score >= 95.0 => "probable_duplicate",
        Some(score) if score >= 80.0 => "possible_duplicate",
        Some(_) => "possible_duplicate",
        None => "unique",
    };

    conn.execute(
        "UPDATE invoices SET duplicate_status = ?2, updated_at = CURRENT_TIMESTAMP WHERE id = ?1",
        params![invoice_id, status],
    )?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::run_migrations;

    fn setup_db() -> Connection {
        let mut conn = Connection::open_in_memory().expect("open sqlite");
        run_migrations(&mut conn).expect("migrate");
        conn
    }

    fn insert_invoice(
        conn: &Connection,
        code: &str,
        number: &str,
        date: &str,
        amount: &str,
        seller: &str,
    ) -> i64 {
        conn.execute(
            "INSERT INTO invoices (raw_file_id, invoice_type, invoice_code, invoice_number, issue_date, seller_name, total_amount, currency, status, duplicate_status)
            VALUES (1, '增值税电子普通发票', ?1, ?2, ?3, ?4, ?5, 'CNY', 'recognized', 'unknown')",
            params![code, number, date, amount, seller],
        )
        .expect("insert");
        conn.last_insert_rowid()
    }

    fn ensure_raw_file(conn: &Connection) -> i64 {
        let exists: Option<i64> = conn
            .query_row("SELECT id FROM raw_files WHERE id = 1", [], |row| {
                row.get(0)
            })
            .optional()
            .expect("check");
        if exists.is_some() {
            return 1;
        }
        conn.execute(
            "INSERT INTO raw_files (id, sha256, md5, original_name, current_name, extension, mime_type, byte_size, storage_path)
            VALUES (1, 'sha', 'md5', 'test.jpg', 'test.jpg', 'jpg', 'image/jpeg', 100, '/tmp/test.jpg')",
            [],
        )
        .expect("insert raw");
        1
    }

    #[test]
    fn detects_exact_invoice_code_number_match() {
        let conn = setup_db();
        ensure_raw_file(&conn);

        let id1 = insert_invoice(
            &conn,
            "CODE001",
            "NUM001",
            "2026-04-01",
            "100.00",
            "SellerA",
        );
        let _id2 = insert_invoice(
            &conn,
            "CODE001",
            "NUM001",
            "2026-04-01",
            "100.00",
            "SellerA",
        );

        detect_field_duplicates(&conn, id1).expect("detect");
        let result = check_invoice_duplicates(&conn, id1).expect("check");

        assert!(result.has_exact_duplicate);
        assert!(!result.candidates.is_empty());
        assert!(result.candidates[0].score >= 95.0);
    }

    #[test]
    fn resolves_duplicate_as_confirmed() {
        let conn = setup_db();
        ensure_raw_file(&conn);

        let id1 = insert_invoice(
            &conn,
            "CODE002",
            "NUM002",
            "2026-05-01",
            "200.00",
            "SellerB",
        );
        let _id2 = insert_invoice(
            &conn,
            "CODE002",
            "NUM002",
            "2026-05-01",
            "200.00",
            "SellerB",
        );

        detect_field_duplicates(&conn, id1).expect("detect");
        let result = check_invoice_duplicates(&conn, id1).expect("check");
        let dedupe_id = result.candidates[0].id;

        resolve_duplicate(
            &conn,
            ResolveDuplicateRequest {
                dedupe_id,
                action: "confirm".into(),
            },
        )
        .expect("resolve");

        let status: String = conn
            .query_row(
                "SELECT duplicate_status FROM invoices WHERE id = ?1",
                [id1],
                |row| row.get(0),
            )
            .expect("read status");
        assert_eq!(status, "probable_duplicate");
    }
}
