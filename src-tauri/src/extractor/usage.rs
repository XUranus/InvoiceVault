//! LLM 使用量统计模块。
//!
//! 记录和查询 LLM 调用的 token 消耗情况。

use rusqlite::Connection;
use serde::Serialize;

use super::ExtractorError;

/// 插入一条 LLM 使用量日志记录。
pub fn insert_usage_log(
    conn: &Connection,
    operation: &str,
    model: &str,
    prompt_tokens: i64,
    completion_tokens: i64,
    total_tokens: i64,
) -> Result<(), ExtractorError> {
    conn.execute(
        "INSERT INTO usage_log (operation, model, prompt_tokens, completion_tokens, total_tokens)
         VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![
            operation,
            model,
            prompt_tokens,
            completion_tokens,
            total_tokens
        ],
    )?;
    Ok(())
}

/// LLM 使用量统计汇总，包含总调用次数、token 消耗和本月数据。
#[derive(Debug, Clone, Serialize)]
pub struct LlmUsageStats {
    pub total_calls: i64,
    pub llm_calls: i64,
    pub embedding_calls: i64,
    pub total_prompt_tokens: i64,
    pub total_completion_tokens: i64,
    pub total_tokens: i64,
    pub this_month_calls: i64,
    pub this_month_tokens: i64,
}

/// 查询 LLM 使用量统计，支持可选的日期范围过滤。
pub fn get_llm_usage(
    conn: &Connection,
    date_from: Option<&str>,
    date_to: Option<&str>,
) -> Result<LlmUsageStats, ExtractorError> {
    let mut clauses: Vec<String> = Vec::new();
    let mut params: Vec<String> = Vec::new();

    if let Some(from) = date_from {
        clauses.push(format!("created_at >= ?{}", params.len() + 1));
        params.push(from.to_owned());
    }
    if let Some(to) = date_to {
        clauses.push(format!("created_at <= ?{}", params.len() + 1));
        params.push(format!("{to} 23:59:59"));
    }

    let where_clause = if clauses.is_empty() {
        String::new()
    } else {
        format!(" WHERE {}", clauses.join(" AND "))
    };

    let (
        total_calls,
        llm_calls,
        embedding_calls,
        total_prompt_tokens,
        total_completion_tokens,
        total_tokens,
    ): (i64, i64, i64, i64, i64, i64) = conn.query_row(
        &format!(
            "SELECT
                COALESCE(COUNT(*), 0),
                COALESCE(SUM(CASE WHEN operation = 'llm_recognition' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN operation = 'embedding' THEN 1 ELSE 0 END), 0),
                COALESCE(SUM(prompt_tokens), 0),
                COALESCE(SUM(completion_tokens), 0),
                COALESCE(SUM(total_tokens), 0)
             FROM usage_log{where_clause}"
        ),
        rusqlite::params_from_iter(params.iter()),
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

    let this_month_start = chrono::Local::now().format("%Y-%m-01").to_string();
    let (this_month_calls, this_month_tokens): (i64, i64) = conn.query_row(
        "SELECT COALESCE(COUNT(*), 0), COALESCE(SUM(total_tokens), 0)
         FROM usage_log WHERE created_at >= ?1",
        [&this_month_start],
        |row| Ok((row.get(0)?, row.get(1)?)),
    )?;

    Ok(LlmUsageStats {
        total_calls,
        llm_calls,
        embedding_calls,
        total_prompt_tokens,
        total_completion_tokens,
        total_tokens,
        this_month_calls,
        this_month_tokens,
    })
}
