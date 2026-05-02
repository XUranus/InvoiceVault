use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use crate::llm::{headers, LlmError, LlmProviderConfig};

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

pub struct ToolDefinition {
    pub name: &'static str,
    pub description: &'static str,
    pub parameters: serde_json::Value,
    #[allow(dead_code)]
    pub is_read_only: bool,
    #[allow(dead_code)]
    pub requires_confirmation: bool,
}

pub fn agent_tools() -> Vec<ToolDefinition> {
    vec![
        ToolDefinition {
            name: "search_invoices",
            description: "搜索发票，支持关键词、日期范围、销售方、发票类型、类别、状态等筛选条件。返回分页结果。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "关键词搜索，匹配销售方、购买方、发票号码、备注等"},
                    "date_from": {"type": "string", "description": "开始日期 YYYY-MM-DD"},
                    "date_to": {"type": "string", "description": "结束日期 YYYY-MM-DD"},
                    "seller_name": {"type": "string", "description": "销售方名称"},
                    "invoice_type": {"type": "string", "description": "发票类型"},
                    "category": {"type": "string", "description": "消费类别"},
                    "status": {"type": "string", "description": "状态: pending_confirmation/recognized/reviewed/flagged"},
                    "page": {"type": "integer", "description": "页码，默认 1"},
                    "page_size": {"type": "integer", "description": "每页条数，默认 20"}
                }
            }),
            is_read_only: true,
            requires_confirmation: false,
        },
        ToolDefinition {
            name: "get_invoice_detail",
            description: "获取单张发票的完整详情，包括所有字段和明细行。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "invoice_id": {"type": "integer", "description": "发票 ID"}
                },
                "required": ["invoice_id"]
            }),
            is_read_only: true,
            requires_confirmation: false,
        },
        ToolDefinition {
            name: "get_dashboard_stats",
            description: "获取仪表盘统计数据：发票总数、金额合计、月度趋势、类型分布、状态分布、Top 供应商排名。支持按日期范围筛选。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "date_from": {"type": "string", "description": "开始日期 YYYY-MM-DD，可选"},
                    "date_to": {"type": "string", "description": "结束日期 YYYY-MM-DD，可选"}
                }
            }),
            is_read_only: true,
            requires_confirmation: false,
        },
        ToolDefinition {
            name: "export_invoices",
            description: "导出筛选的发票为 CSV 或 Excel 文件。需要用户选择保存位置。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "format": {"type": "string", "description": "导出格式: csv 或 xlsx"},
                    "invoice_ids": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "要导出的发票 ID 列表，为空则导出全部"
                    }
                },
                "required": ["format"]
            }),
            is_read_only: false,
            requires_confirmation: true,
        },
        ToolDefinition {
            name: "update_invoice",
            description: "更新发票的字段信息。修改前需要用户确认。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "发票 ID"},
                    "invoice_type": {"type": "string"},
                    "invoice_code": {"type": "string"},
                    "invoice_number": {"type": "string"},
                    "issue_date": {"type": "string", "description": "开票日期 YYYY-MM-DD"},
                    "seller_name": {"type": "string"},
                    "seller_tax_id": {"type": "string"},
                    "buyer_name": {"type": "string"},
                    "buyer_tax_id": {"type": "string"},
                    "currency": {"type": "string"},
                    "amount_without_tax": {"type": "string"},
                    "tax_amount": {"type": "string"},
                    "total_amount": {"type": "string"},
                    "category": {"type": "string"},
                    "remarks": {"type": "string"},
                    "status": {"type": "string"}
                },
                "required": ["id"]
            }),
            is_read_only: false,
            requires_confirmation: true,
        },
    ]
}

// ---------------------------------------------------------------------------
// Public data types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentSession {
    pub id: i64,
    pub title: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentMessageRow {
    pub id: i64,
    pub session_id: i64,
    pub role: String,
    pub content: String,
    pub tool_call_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub messages: Vec<AgentMessageRow>,
    pub pending_confirmation: Option<PendingConfirmation>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingConfirmation {
    pub tool_name: String,
    pub arguments: serde_json::Value,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConfirmRequest {
    pub session_id: i64,
    pub confirmed: bool,
    pub extra_params: Option<serde_json::Value>,
}

// ---------------------------------------------------------------------------
// Tool execution result
// ---------------------------------------------------------------------------

pub enum ToolExecResult {
    Success {
        content: String,
    },
    ConfirmationRequired {
        tool_name: String,
        arguments: serde_json::Value,
        message: String,
    },
    Error {
        message: String,
    },
}

// ---------------------------------------------------------------------------
// Error type
// ---------------------------------------------------------------------------

#[derive(Debug, thiserror::Error)]
#[allow(dead_code)]
pub enum AgentError {
    #[error("database error: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("LLM error: {0}")]
    Llm(#[from] LlmError),
    #[error("request error: {0}")]
    Request(#[from] reqwest::Error),
    #[error("no assistant response")]
    NoAssistantResponse,
    #[error("too many tool call iterations")]
    TooManyIterations,
    #[error("serialization error: {0}")]
    Serialization(#[from] serde_json::Error),
    #[error("no pending confirmation for this session")]
    NoPendingConfirmation,
    #[error("session not found")]
    SessionNotFound,
}

// ---------------------------------------------------------------------------
// Internal LLM message types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCall {
    pub id: String,
    #[serde(rename = "type")]
    pub type_: String,
    pub function: ToolCallFunction,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolCallFunction {
    pub name: String,
    pub arguments: String,
}

#[derive(Debug, Clone, Serialize)]
struct LlmMessage {
    role: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    content: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_calls: Option<Vec<ToolCall>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    tool_call_id: Option<String>,
}

#[derive(Debug, Serialize)]
struct ToolDef {
    #[serde(rename = "type")]
    type_: String,
    function: FunctionDef,
}

#[derive(Debug, Serialize)]
struct FunctionDef {
    name: String,
    description: String,
    parameters: serde_json::Value,
}

#[derive(Debug, Serialize)]
struct ToolChatRequest<'a> {
    model: &'a str,
    messages: Vec<LlmMessage>,
    tools: Vec<ToolDef>,
    #[serde(rename = "tool_choice")]
    tool_choice: &'a str,
    temperature: f32,
    max_tokens: u16,
}

#[derive(Debug, Deserialize)]
struct ToolChatResponse {
    choices: Vec<ToolChatChoice>,
}

#[derive(Debug, Deserialize)]
struct ToolChatChoice {
    message: ToolChatMessage,
}

#[derive(Debug, Deserialize)]
struct ToolChatMessage {
    #[allow(dead_code)]
    role: Option<String>,
    content: Option<String>,
    tool_calls: Option<Vec<ToolCall>>,
}

// ---------------------------------------------------------------------------
// System prompt
// ---------------------------------------------------------------------------

const SYSTEM_PROMPT: &str = r#"你是 InvoiceVault 发票处理助手，只能使用内置工具完成用户的请求。

规则：
- 查询发票时使用 search_invoices 工具，根据用户意图设置筛选条件
- 需要发票详情时使用 get_invoice_detail 工具
- 统计信息使用 get_dashboard_stats 工具
- 用户要求导出时，使用 export_invoices 工具，告知将要导出的数量和格式
- 修改发票信息使用 update_invoice 工具
- 工具返回什么数据就如实汇报，不要虚构或编造数据
- 如果用户请求超出你的工具能力范围，如实说明并给出建议
- 回答使用中文，简洁清晰
- 涉及金额时保留两位小数"#;

// ---------------------------------------------------------------------------
// Database operations
// ---------------------------------------------------------------------------

pub fn create_session(conn: &Connection, title: Option<&str>) -> Result<AgentSession, AgentError> {
    let title = title.unwrap_or("新对话");
    conn.execute("INSERT INTO agent_sessions (title) VALUES (?1)", [title])?;
    let id = conn.last_insert_rowid();
    Ok(AgentSession {
        id,
        title: title.to_owned(),
        created_at: String::new(),
        updated_at: String::new(),
    })
}

pub fn list_sessions(conn: &Connection) -> Result<Vec<AgentSession>, AgentError> {
    let mut stmt = conn.prepare(
        "SELECT id, title, created_at, updated_at FROM agent_sessions ORDER BY updated_at DESC",
    )?;
    let sessions = stmt
        .query_map([], |row| {
            Ok(AgentSession {
                id: row.get(0)?,
                title: row.get(1)?,
                created_at: row.get(2)?,
                updated_at: row.get(3)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(sessions)
}

pub fn get_session_messages(
    conn: &Connection,
    session_id: i64,
) -> Result<Vec<AgentMessageRow>, AgentError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, role, content, tool_call_json, created_at
         FROM agent_messages
         WHERE session_id = ?1
         ORDER BY id ASC",
    )?;
    let msgs = stmt
        .query_map([session_id], |row| {
            Ok(AgentMessageRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                tool_call_json: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(msgs)
}

pub fn delete_session(conn: &Connection, session_id: i64) -> Result<(), AgentError> {
    conn.execute(
        "DELETE FROM agent_messages WHERE session_id = ?1",
        [session_id],
    )?;
    conn.execute("DELETE FROM agent_sessions WHERE id = ?1", [session_id])?;
    Ok(())
}

fn save_message(
    conn: &Connection,
    msg: &LlmMessage,
    session_id: i64,
) -> Result<AgentMessageRow, AgentError> {
    let tool_call_json = msg
        .tool_calls
        .as_ref()
        .map(|tc| serde_json::to_string(tc))
        .transpose()?;
    let content = msg.content.clone().unwrap_or_default();
    conn.execute(
        "INSERT INTO agent_messages (session_id, role, content, tool_call_json) VALUES (?1, ?2, ?3, ?4)",
        rusqlite::params![session_id, msg.role, content, tool_call_json],
    )?;
    let id = conn.last_insert_rowid();
    conn.execute(
        "UPDATE agent_sessions SET updated_at = datetime('now') WHERE id = ?1",
        [session_id],
    )?;
    Ok(AgentMessageRow {
        id,
        session_id,
        role: msg.role.clone(),
        content,
        tool_call_json,
        created_at: String::new(),
    })
}

pub fn write_audit_log(
    conn: &Connection,
    actor: &str,
    action: &str,
    target_type: Option<&str>,
    target_id: Option<i64>,
    payload: Option<&str>,
) -> Result<(), AgentError> {
    conn.execute(
        "INSERT INTO audit_logs (actor, action, target_type, target_id, payload_json) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![actor, action, target_type, target_id, payload],
    )?;
    Ok(())
}

fn update_session_title(conn: &Connection, session_id: i64) -> Result<(), AgentError> {
    // Auto-title: use first user message (max 30 chars)
    let title: Option<String> = conn.query_row(
        "SELECT content FROM agent_messages WHERE session_id = ?1 AND role = 'user' ORDER BY id ASC LIMIT 1",
        [session_id],
        |row| row.get(0),
    ).ok();
    if let Some(title) = title {
        let short: String = title.chars().take(30).collect();
        conn.execute(
            "UPDATE agent_sessions SET title = ?1 WHERE id = ?2 AND title = '新对话'",
            rusqlite::params![short, session_id],
        )?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// LLM chat with tools
// ---------------------------------------------------------------------------

async fn send_chat_request(
    messages: Vec<LlmMessage>,
    config: &LlmProviderConfig,
) -> Result<ToolChatResponse, AgentError> {
    let base_url = config.base_url.trim().trim_end_matches('/');
    let api_key = config.api_key.trim();
    let model = config.model.trim();

    if base_url.is_empty() {
        return Err(LlmError::MissingBaseUrl.into());
    }
    if api_key.is_empty() {
        return Err(LlmError::MissingApiKey.into());
    }
    if model.is_empty() {
        return Err(LlmError::MissingModel.into());
    }

    let timeout = Duration::from_secs(config.timeout_seconds.unwrap_or(60).clamp(1, 300));
    let client = reqwest::Client::builder().timeout(timeout).build()?;

    let tools: Vec<ToolDef> = agent_tools()
        .into_iter()
        .map(|t| ToolDef {
            type_: "function".to_owned(),
            function: FunctionDef {
                name: t.name.to_owned(),
                description: t.description.to_owned(),
                parameters: t.parameters,
            },
        })
        .collect();

    let request = ToolChatRequest {
        model,
        messages,
        tools,
        tool_choice: "auto",
        temperature: 0.0,
        max_tokens: 2000,
    };

    let response = client
        .post(format!("{base_url}/chat/completions"))
        .headers(headers(api_key)?)
        .json(&request)
        .send()
        .await?;

    let status = response.status();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        error!("Agent: LLM HTTP {status}: {}", crate::llm::truncate(&body, 200));
        return Err(LlmError::ProviderStatus {
            status: status.as_u16(),
            body: crate::llm::truncate(&body, 500),
        }
        .into());
    }

    let body: ToolChatResponse = response.json().await?;
    Ok(body)
}

// ---------------------------------------------------------------------------
// Agent chat loop
// ---------------------------------------------------------------------------

/// Run one turn of the agent chat loop (may involve multiple tool-calling round trips).
/// Returns all new messages to persist plus any pending confirmation.
pub async fn run_agent_turn(
    db: &Mutex<Connection>,
    session_id: i64,
    user_message: &str,
    config: &LlmProviderConfig,
    execute_tool: Arc<dyn Fn(&str, &serde_json::Value) -> ToolExecResult + Send + Sync>,
) -> Result<AgentResponse, AgentError> {
    let mut new_messages: Vec<AgentMessageRow> = Vec::new();

    // Save user message
    let user_msg = LlmMessage {
        role: "user".to_owned(),
        content: Some(user_message.to_owned()),
        tool_calls: None,
        tool_call_id: None,
    };
    {
        let conn = db.lock().expect("db lock");
        new_messages.push(save_message(&conn, &user_msg, session_id)?);
        update_session_title(&conn, session_id)?;
    }

    // Load history for context (last 20 messages)
    let history = {
        let conn = db.lock().expect("db lock");
        get_recent_messages(&conn, session_id, 20)?
    };

    // Build initial message list
    let mut llm_messages = build_llm_messages(&history);
    llm_messages.push(user_msg.clone());

    // Tool calling loop
    let result =
        run_agent_loop_from_inner(db, session_id, llm_messages, config, execute_tool).await?;
    new_messages.extend(result.messages);
    Ok(AgentResponse {
        messages: new_messages,
        pending_confirmation: result.pending_confirmation,
    })
}

/// Continue agent turn after user confirmation or rejection.
/// Loads conversation from DB, appends tool result, and continues the loop.
pub async fn continue_agent_turn(
    db: &Mutex<Connection>,
    session_id: i64,
    confirmed: bool,
    extra_params: Option<serde_json::Value>,
    config: &LlmProviderConfig,
    execute_tool: Arc<dyn Fn(&str, &serde_json::Value) -> ToolExecResult + Send + Sync>,
) -> Result<AgentResponse, AgentError> {
    let mut new_messages: Vec<AgentMessageRow> = Vec::new();

    // Load all messages for this session
    let history = {
        let conn = db.lock().expect("db lock");
        get_recent_messages(&conn, session_id, 30)?
    };
    let mut llm_messages = build_llm_messages(&history);

    // Find the last pending tool call from the most recent assistant message
    let last_tool_call = history
        .iter()
        .rev()
        .find(|m| m.role == "assistant" && m.tool_call_json.is_some())
        .and_then(|m| {
            m.tool_call_json
                .as_ref()
                .and_then(|json| serde_json::from_str::<Vec<ToolCall>>(json).ok())
                .and_then(|calls| calls.into_iter().last())
        });

    let Some(tc) = last_tool_call else {
        // No pending tool call found; just do a fresh turn with a system note
        let note = if confirmed {
            "已确认执行之前的操作。"
        } else {
            "已取消之前的操作。"
        };
        let user_msg = LlmMessage {
            role: "user".to_owned(),
            content: Some(note.to_owned()),
            tool_calls: None,
            tool_call_id: None,
        };
        {
            let conn = db.lock().expect("db lock");
            new_messages.push(save_message(&conn, &user_msg, session_id)?);
        }
        llm_messages.push(user_msg);

        let rest =
            run_agent_loop_from_inner(db, session_id, llm_messages, config, execute_tool).await?;
        new_messages.extend(rest.messages);
        return Ok(AgentResponse {
            messages: new_messages,
            pending_confirmation: rest.pending_confirmation,
        });
    };

    if confirmed {
        let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)?;

        // Merge _confirmed flag and extra_params into arguments
        let mut final_args = merge_json(args, serde_json::json!({"_confirmed": true}));
        if let Some(ref extra) = extra_params {
            final_args = merge_json(final_args, extra.clone());
        }

        let result = execute_tool(&tc.function.name, &final_args);

        let tool_content = match result {
            ToolExecResult::Success { content } => {
                {
                    let conn = db.lock().expect("db lock");
                    write_audit_log(
                        &conn,
                        "agent",
                        &format!("tool:{}", tc.function.name),
                        Some("invoice"),
                        final_args
                            .get("id")
                            .or(final_args.get("invoice_id"))
                            .and_then(|v| v.as_i64()),
                        Some(&content),
                    )?;
                }
                content
            }
            ToolExecResult::Error { message } => {
                {
                    let conn = db.lock().expect("db lock");
                    write_audit_log(
                        &conn,
                        "agent",
                        &format!("tool:{}:error", tc.function.name),
                        None,
                        None,
                        Some(&message),
                    )?;
                }
                format!("Error: {message}")
            }
            ToolExecResult::ConfirmationRequired { .. } => "操作需要额外确认，请重试。".to_owned(),
        };

        let tool_msg = LlmMessage {
            role: "tool".to_owned(),
            content: Some(tool_content),
            tool_calls: None,
            tool_call_id: Some(tc.id.clone()),
        };
        {
            let conn = db.lock().expect("db lock");
            new_messages.push(save_message(&conn, &tool_msg, session_id)?);
        }
        llm_messages.push(tool_msg);
    } else {
        // User rejected
        {
            let conn = db.lock().expect("db lock");
            write_audit_log(
                &conn,
                "agent",
                &format!("tool:{}:rejected", tc.function.name),
                None,
                None,
                None,
            )?;
        }

        let tool_msg = LlmMessage {
            role: "tool".to_owned(),
            content: Some("用户取消了此操作。".to_owned()),
            tool_calls: None,
            tool_call_id: Some(tc.id.clone()),
        };
        {
            let conn = db.lock().expect("db lock");
            new_messages.push(save_message(&conn, &tool_msg, session_id)?);
        }
        llm_messages.push(tool_msg);
    }

    // Continue loop
    let rest =
        run_agent_loop_from_inner(db, session_id, llm_messages, config, execute_tool).await?;
    new_messages.extend(rest.messages);
    Ok(AgentResponse {
        messages: new_messages,
        pending_confirmation: rest.pending_confirmation,
    })
}

/// Continue loop from a given message state (does NOT save a new user message first).
async fn run_agent_loop_from_inner(
    db: &Mutex<Connection>,
    session_id: i64,
    mut llm_messages: Vec<LlmMessage>,
    config: &LlmProviderConfig,
    execute_tool: Arc<dyn Fn(&str, &serde_json::Value) -> ToolExecResult + Send + Sync>,
) -> Result<AgentResponse, AgentError> {
    let mut new_messages: Vec<AgentMessageRow> = Vec::new();
    const MAX_ITERATIONS: usize = 5;

    for _iteration in 0..MAX_ITERATIONS {
        let response = send_chat_request(llm_messages.clone(), config).await?;
        let choice = response
            .choices
            .into_iter()
            .next()
            .ok_or(AgentError::NoAssistantResponse)?;
        let msg = choice.message;

        if let Some(tool_calls) = msg.tool_calls {
            if tool_calls.is_empty() {
                let assistant_msg = LlmMessage {
                    role: "assistant".to_owned(),
                    content: msg.content,
                    tool_calls: None,
                    tool_call_id: None,
                };
                {
                    let conn = db.lock().expect("db lock");
                    new_messages.push(save_message(&conn, &assistant_msg, session_id)?);
                }
                return Ok(AgentResponse {
                    messages: new_messages,
                    pending_confirmation: None,
                });
            }

            let assistant_msg = LlmMessage {
                role: "assistant".to_owned(),
                content: msg.content,
                tool_calls: Some(tool_calls.clone()),
                tool_call_id: None,
            };
            {
                let conn = db.lock().expect("db lock");
                new_messages.push(save_message(&conn, &assistant_msg, session_id)?);
            }
            llm_messages.push(assistant_msg);

            let mut tool_result_msgs: Vec<LlmMessage> = Vec::new();
            for tc in &tool_calls {
                let args: serde_json::Value = serde_json::from_str(&tc.function.arguments)?;
                let result = execute_tool(&tc.function.name, &args);

                match result {
                    ToolExecResult::Success { content } => {
                        {
                            let conn = db.lock().expect("db lock");
                            write_audit_log(
                                &conn,
                                "agent",
                                &format!("tool:{}", tc.function.name),
                                Some("invoice"),
                                args.get("id")
                                    .or(args.get("invoice_id"))
                                    .and_then(|v| v.as_i64()),
                                Some(&content),
                            )?;
                        }
                        let tool_msg = LlmMessage {
                            role: "tool".to_owned(),
                            content: Some(content),
                            tool_calls: None,
                            tool_call_id: Some(tc.id.clone()),
                        };
                        {
                            let conn = db.lock().expect("db lock");
                            new_messages.push(save_message(&conn, &tool_msg, session_id)?);
                        }
                        tool_result_msgs.push(tool_msg);
                    }
                    ToolExecResult::ConfirmationRequired {
                        tool_name,
                        arguments,
                        message,
                    } => {
                        return Ok(AgentResponse {
                            messages: new_messages,
                            pending_confirmation: Some(PendingConfirmation {
                                tool_name,
                                arguments,
                                message,
                            }),
                        });
                    }
                    ToolExecResult::Error { message } => {
                        error!("Agent tool {} error: {message}", tc.function.name);
                        let tool_msg = LlmMessage {
                            role: "tool".to_owned(),
                            content: Some(format!("Error: {message}")),
                            tool_calls: None,
                            tool_call_id: Some(tc.id.clone()),
                        };
                        {
                            let conn = db.lock().expect("db lock");
                            new_messages.push(save_message(&conn, &tool_msg, session_id)?);
                        }
                        tool_result_msgs.push(tool_msg);
                    }
                }
            }
            llm_messages.extend(tool_result_msgs);
        } else {
            let assistant_msg = LlmMessage {
                role: "assistant".to_owned(),
                content: msg.content,
                tool_calls: None,
                tool_call_id: None,
            };
            {
                let conn = db.lock().expect("db lock");
                new_messages.push(save_message(&conn, &assistant_msg, session_id)?);
            }
            return Ok(AgentResponse {
                messages: new_messages,
                pending_confirmation: None,
            });
        }
    }

    warn!("Agent session {session_id}: too many iterations");
    Err(AgentError::TooManyIterations)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn get_recent_messages(
    conn: &Connection,
    session_id: i64,
    limit: usize,
) -> Result<Vec<AgentMessageRow>, AgentError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, role, content, tool_call_json, created_at
         FROM agent_messages
         WHERE session_id = ?1
         ORDER BY id DESC
         LIMIT ?2",
    )?;
    let mut msgs: Vec<AgentMessageRow> = stmt
        .query_map(rusqlite::params![session_id, limit as i64], |row| {
            Ok(AgentMessageRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                tool_call_json: row.get(4)?,
                created_at: row.get(5)?,
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    msgs.reverse();
    Ok(msgs)
}

fn build_llm_messages(history: &[AgentMessageRow]) -> Vec<LlmMessage> {
    let mut messages = vec![LlmMessage {
        role: "system".to_owned(),
        content: Some(SYSTEM_PROMPT.to_owned()),
        tool_calls: None,
        tool_call_id: None,
    }];

    for m in history {
        let tool_calls: Option<Vec<ToolCall>> = m
            .tool_call_json
            .as_ref()
            .and_then(|json| serde_json::from_str(json).ok());

        messages.push(LlmMessage {
            role: m.role.clone(),
            content: if m.content.is_empty() {
                None
            } else {
                Some(m.content.clone())
            },
            tool_calls,
            tool_call_id: None, // tool_call_id comes from the original tool call, not stored separately
        });
    }

    messages
}

fn merge_json(mut base: serde_json::Value, extra: serde_json::Value) -> serde_json::Value {
    if let (Some(base_obj), Some(extra_obj)) = (base.as_object_mut(), extra.as_object()) {
        for (key, value) in extra_obj {
            base_obj.insert(key.clone(), value.clone());
        }
    }
    base
}
