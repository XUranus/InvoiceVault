use std::sync::{Arc, Mutex};
use std::time::Duration;

use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

use crate::llm::{
    body_to_value, headers, write_llm_audit_record, LlmAuditConfig, LlmAuditRecord, LlmError,
    LlmProviderConfig,
};

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
                    "buyer_name": {"type": "string", "description": "购买方名称"},
                    "invoice_number": {"type": "string", "description": "发票号码"},
                    "invoice_type": {"type": "string", "description": "发票类型，如增值税普通发票/增值税专用发票"},
                    "category": {"type": "string", "description": "消费类别"},
                    "status": {"type": "string", "description": "状态: pending_confirmation/recognized/reviewed/flagged"},
                    "duplicate_status": {"type": "string", "description": "重复状态"},
                    "amount_min": {"type": "string", "description": "最小金额"},
                    "amount_max": {"type": "string", "description": "最大金额"},
                    "sort_by": {"type": "string", "description": "排序字段"},
                    "sort_order": {"type": "string", "description": "asc 或 desc"},
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
            name: "get_current_date_context",
            description: "获取当前日期上下文，用于解析这个月、上个月、本季度等相对时间表达。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            is_read_only: true,
            requires_confirmation: false,
        },
        ToolDefinition {
            name: "get_invoice_field_catalog",
            description: "获取发票字段字典，包括可导出字段 key、中文名、别名和数据类型。用于把用户说的列名映射为导出字段。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            is_read_only: true,
            requires_confirmation: false,
        },
        ToolDefinition {
            name: "list_message_attachments",
            description: "列出当前会话中用户上传的附件。用户提到表格、上传文件、模板时，先使用此工具查找附件 ID。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            is_read_only: true,
            requires_confirmation: false,
        },
        ToolDefinition {
            name: "inspect_spreadsheet",
            description: "检查上传的 CSV/XLSX 表格，返回工作表、表头、列名和前几行样例。用于理解用户提供的导出模板格式。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "attachment_id": {"type": "integer", "description": "附件 ID"},
                    "max_rows": {"type": "integer", "description": "最多读取的样例行数，默认 5"}
                },
                "required": ["attachment_id"]
            }),
            is_read_only: true,
            requires_confirmation: false,
        },
        ToolDefinition {
            name: "export_invoices",
            description: "导出筛选的发票为 CSV 或 Excel 文件。支持自定义导出列和日期范围。需要用户选择保存位置。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "format": {"type": "string", "enum": ["csv", "xlsx"], "description": "导出格式: csv 或 xlsx"},
                    "invoice_ids": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "要导出的发票 ID 列表。为空时按日期范围或全部导出"
                    },
                    "columns": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "导出字段 key 列表，例如只导出发票代码时传 invoice_code。可先调用 get_invoice_field_catalog 获取字段字典"
                    },
                    "date_from": {"type": "string", "description": "开始日期 YYYY-MM-DD"},
                    "date_to": {"type": "string", "description": "结束日期 YYYY-MM-DD"}
                },
                "required": ["format"]
            }),
            is_read_only: false,
            requires_confirmation: true,
        },
        ToolDefinition {
            name: "create_export_preview",
            description: "预览一次发票导出，返回匹配行数、导出列和前几行样例。复杂导出前应先使用此工具让用户确认。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "invoice_ids": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "要预览的发票 ID 列表。为空时按日期范围或全部预览"
                    },
                    "columns": {
                        "type": "array",
                        "items": {"type": "string"},
                        "description": "导出字段 key 列表"
                    },
                    "date_from": {"type": "string", "description": "开始日期 YYYY-MM-DD"},
                    "date_to": {"type": "string", "description": "结束日期 YYYY-MM-DD"},
                    "limit": {"type": "integer", "description": "样例行数，默认 5"}
                }
            }),
            is_read_only: true,
            requires_confirmation: false,
        },
        ToolDefinition {
            name: "export_invoices_with_template",
            description: "按上传表格模板的表头列顺序导出发票。当前版本复用模板列结构，不复制样式/公式/合并单元格。需要用户选择保存位置。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "attachment_id": {"type": "integer", "description": "模板表格附件 ID"},
                    "format": {"type": "string", "enum": ["xlsx", "csv"], "description": "导出格式，默认 xlsx"},
                    "invoice_ids": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "要导出的发票 ID 列表。为空时按日期范围或全部导出"
                    },
                    "date_from": {"type": "string", "description": "开始日期 YYYY-MM-DD"},
                    "date_to": {"type": "string", "description": "结束日期 YYYY-MM-DD"}
                },
                "required": ["attachment_id"]
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
        ToolDefinition {
            name: "merge_invoices",
            description: "将多张发票合并为一张。用于多页 PDF 识别后将多个页面合并为一张完整发票。要求所有发票属于同一文件。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "target_invoice_id": {"type": "integer", "description": "目标发票 ID（合并后的保留对象）"},
                    "source_invoice_ids": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "要合并到目标的源发票 ID 列表"
                    }
                },
                "required": ["target_invoice_id", "source_invoice_ids"]
            }),
            is_read_only: false,
            requires_confirmation: true,
        },
        ToolDefinition {
            name: "export_pdf_report",
            description: "将选中发票导出为 PDF 报表。报表包含汇总表和每张发票的详情页（含缩略图）。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "invoice_ids": {
                        "type": "array",
                        "items": {"type": "integer"},
                        "description": "要导出的发票 ID 列表。为空时按日期范围或全部导出"
                    },
                    "date_from": {"type": "string", "description": "开始日期 YYYY-MM-DD"},
                    "date_to": {"type": "string", "description": "结束日期 YYYY-MM-DD"}
                }
            }),
            is_read_only: false,
            requires_confirmation: true,
        },
        ToolDefinition {
            name: "get_badge_config",
            description: "获取当前自定义标签（Badge）配置，包括所有分组名称和可选项。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            is_read_only: true,
            requires_confirmation: false,
        },
        ToolDefinition {
            name: "set_badge_config",
            description: "设置自定义标签（Badge）配置，替换所有分组和选项。修改前需要用户确认。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "groups": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "name": {"type": "string", "description": "分组名称"},
                                "options": {
                                    "type": "array",
                                    "items": {"type": "string"},
                                    "description": "该分组下的选项列表"
                                }
                            },
                            "required": ["name", "options"]
                        },
                        "description": "标签分组列表"
                    }
                },
                "required": ["groups"]
            }),
            is_read_only: false,
            requires_confirmation: true,
        },
        ToolDefinition {
            name: "set_invoice_badge",
            description: "为指定发票设置标签值。value 传 null 表示取消该标签。修改前需要用户确认。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "invoice_id": {"type": "integer", "description": "发票 ID"},
                    "group_name": {"type": "string", "description": "标签分组名称"},
                    "value": {"type": ["string", "null"], "description": "标签值，null 表示取消"}
                },
                "required": ["invoice_id", "group_name"]
            }),
            is_read_only: false,
            requires_confirmation: true,
        },
        ToolDefinition {
            name: "get_price_config",
            description: "获取当前 LLM 和 Embedding 的价格配置（每千 token 价格）。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            is_read_only: true,
            requires_confirmation: false,
        },
        ToolDefinition {
            name: "set_price_config",
            description: "修改 LLM 和 Embedding 的价格配置（每千 token 价格）。修改前需要用户确认。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "llm_input_price_per_1k": {"type": "number", "description": "LLM 输入每千 token 价格（美元）"},
                    "llm_output_price_per_1k": {"type": "number", "description": "LLM 输出每千 token 价格（美元）"},
                    "embedding_input_price_per_1k": {"type": "number", "description": "Embedding 输入每千 token 价格（美元）"},
                    "embedding_output_price_per_1k": {"type": "number", "description": "Embedding 输出每千 token 价格（美元）"}
                }
            }),
            is_read_only: false,
            requires_confirmation: true,
        },
        ToolDefinition {
            name: "get_recognition_status",
            description: "获取识别任务队列状态，包括待处理数、运行中数和最大并发数。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            is_read_only: true,
            requires_confirmation: false,
        },
        ToolDefinition {
            name: "set_recognition_concurrency",
            description: "设置识别任务的最大并发数。修改前需要用户确认。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "max_concurrent": {"type": "integer", "description": "最大并发数，建议 1-5"}
                },
                "required": ["max_concurrent"]
            }),
            is_read_only: false,
            requires_confirmation: true,
        },
        ToolDefinition {
            name: "get_theme",
            description: "获取当前主题设置（亮色/暗色）。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            is_read_only: true,
            requires_confirmation: false,
        },
        ToolDefinition {
            name: "set_theme",
            description: "切换主题为亮色或暗色模式。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "theme": {"type": "string", "enum": ["light", "dark"], "description": "主题: light 亮色, dark 暗色"}
                },
                "required": ["theme"]
            }),
            is_read_only: false,
            requires_confirmation: false,
        },
        ToolDefinition {
            name: "export_logs",
            description: "导出应用日志文件到指定路径。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "output_path": {"type": "string", "description": "导出文件的保存路径"}
                },
                "required": ["output_path"]
            }),
            is_read_only: false,
            requires_confirmation: true,
        },
        ToolDefinition {
            name: "export_backup",
            description: "导出数据库和配置文件的备份包到指定路径。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {
                    "output_path": {"type": "string", "description": "备份文件的保存路径"}
                },
                "required": ["output_path"]
            }),
            is_read_only: false,
            requires_confirmation: true,
        },
        ToolDefinition {
            name: "cleanup_storage",
            description: "清理孤立文件和过期数据，释放存储空间。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            is_read_only: false,
            requires_confirmation: true,
        },
        ToolDefinition {
            name: "get_app_info",
            description: "获取应用版本、数据目录路径和数据库状态等系统信息。",
            parameters: serde_json::json!({
                "type": "object",
                "properties": {}
            }),
            is_read_only: true,
            requires_confirmation: false,
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
    pub tool_call_id: Option<String>,
    pub created_at: String,
    pub attachments: Vec<AgentAttachment>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentAttachment {
    pub id: i64,
    pub session_id: i64,
    pub message_id: Option<i64>,
    pub original_name: String,
    pub mime_type: Option<String>,
    pub byte_size: i64,
    pub storage_path: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentTask {
    pub id: i64,
    pub session_id: i64,
    pub tool_name: String,
    pub status: String,
    pub input_json: Option<String>,
    pub result_json: Option<String>,
    pub error_message: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentArtifact {
    pub id: i64,
    pub session_id: i64,
    pub task_id: Option<i64>,
    pub artifact_type: String,
    pub title: String,
    pub file_path: Option<String>,
    pub mime_type: Option<String>,
    pub byte_size: Option<i64>,
    pub metadata_json: Option<String>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AgentResponse {
    pub messages: Vec<AgentMessageRow>,
    pub pending_confirmation: Option<PendingConfirmation>,
}

pub type AgentStreamSink = Arc<dyn Fn(AgentStreamEvent) + Send + Sync>;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum AgentStreamEvent {
    Started,
    AssistantDelta {
        delta: String,
    },
    ToolCall {
        tool_name: String,
    },
    ToolResult {
        tool_name: String,
    },
    PendingConfirmation {
        pending_confirmation: PendingConfirmation,
    },
    Finished,
    Error {
        message: String,
    },
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
    #[serde(skip_serializing_if = "Option::is_none")]
    stream: Option<bool>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolChatResponse {
    choices: Vec<ToolChatChoice>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolChatChoice {
    message: ToolChatMessage,
}

#[derive(Debug, Serialize, Deserialize)]
struct ToolChatMessage {
    #[allow(dead_code)]
    role: Option<String>,
    content: Option<String>,
    tool_calls: Option<Vec<ToolCall>>,
}

#[derive(Debug, Deserialize)]
struct ToolChatStreamChunk {
    choices: Vec<ToolChatStreamChoice>,
}

#[derive(Debug, Deserialize)]
struct ToolChatStreamChoice {
    delta: ToolChatStreamDelta,
}

#[derive(Debug, Deserialize)]
struct ToolChatStreamDelta {
    #[allow(dead_code)]
    role: Option<String>,
    content: Option<String>,
    tool_calls: Option<Vec<ToolCallDelta>>,
}

#[derive(Debug, Deserialize)]
struct ToolCallDelta {
    index: usize,
    id: Option<String>,
    #[serde(rename = "type")]
    type_: Option<String>,
    function: Option<ToolCallFunctionDelta>,
}

#[derive(Debug, Deserialize)]
struct ToolCallFunctionDelta {
    name: Option<String>,
    arguments: Option<String>,
}

#[derive(Debug, Default)]
struct ToolCallAccumulator {
    id: Option<String>,
    type_: Option<String>,
    name: String,
    arguments: String,
}

// ---------------------------------------------------------------------------
// System prompt
// ---------------------------------------------------------------------------

const SYSTEM_PROMPT: &str = r#"你是 InvoiceVault 发票处理助手，只能使用内置工具完成用户的请求。

规则：
- 查询发票时使用 search_invoices 工具，根据用户意图设置筛选条件
- 需要发票详情时使用 get_invoice_detail 工具
- 统计信息使用 get_dashboard_stats 工具
- 用户要求导出时，先用 get_invoice_field_catalog 映射列名；需要相对日期时先用 get_current_date_context；如果用户上传了表格模板，先用 list_message_attachments 和 inspect_spreadsheet 理解表头
- 复杂导出前先调用 create_export_preview，向用户说明匹配行数、列和样例；确认后再调用 export_invoices 或 export_invoices_with_template
- export_invoices 支持 columns，自定义列必须传字段 key。例如”只包含发票代码”应传 columns=[“invoice_code”]
- 修改发票信息使用 update_invoice 工具
- 自定义标签管理：使用 get_badge_config 获取当前配置，set_badge_config 修改配置（增删分组和选项），set_invoice_badge 给发票设置标签
- 系统设置：使用 get_price_config/set_price_config 管理 LLM 价格配置；get_recognition_status/set_recognition_concurrency 管理识别并发数
- 主题切换：使用 get_theme/set_theme 切换亮色/暗色主题
- 维护操作：export_logs 导出日志，export_backup 导出备份，cleanup_storage 清理存储空间，get_app_info 查看系统版本信息
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
        "SELECT id, session_id, role, content, tool_call_json, tool_call_id, created_at
         FROM agent_messages
         WHERE session_id = ?1
         ORDER BY id ASC",
    )?;
    let mut msgs = stmt
        .query_map([session_id], |row| {
            Ok(AgentMessageRow {
                id: row.get(0)?,
                session_id: row.get(1)?,
                role: row.get(2)?,
                content: row.get(3)?,
                tool_call_json: row.get(4)?,
                tool_call_id: row.get(5)?,
                created_at: row.get(6)?,
                attachments: Vec::new(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    attach_attachments(conn, &mut msgs)?;
    Ok(msgs)
}

pub fn insert_attachment(
    conn: &Connection,
    session_id: i64,
    original_name: &str,
    mime_type: Option<&str>,
    byte_size: i64,
    storage_path: &str,
) -> Result<AgentAttachment, AgentError> {
    conn.execute(
        "INSERT INTO agent_attachments (session_id, original_name, mime_type, byte_size, storage_path) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![session_id, original_name, mime_type, byte_size, storage_path],
    )?;
    let id = conn.last_insert_rowid();
    get_attachment(conn, id)
}

pub fn get_attachment(conn: &Connection, id: i64) -> Result<AgentAttachment, AgentError> {
    conn.query_row(
        "SELECT id, session_id, message_id, original_name, mime_type, byte_size, storage_path, created_at FROM agent_attachments WHERE id = ?1",
        [id],
        map_attachment,
    ).map_err(AgentError::from)
}

pub fn list_session_attachments(
    conn: &Connection,
    session_id: i64,
) -> Result<Vec<AgentAttachment>, AgentError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, message_id, original_name, mime_type, byte_size, storage_path, created_at
         FROM agent_attachments
         WHERE session_id = ?1
         ORDER BY id DESC",
    )?;
    let attachments = stmt
        .query_map([session_id], map_attachment)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(attachments)
}

pub fn create_task(
    conn: &Connection,
    session_id: i64,
    tool_name: &str,
    input_json: Option<&str>,
) -> Result<AgentTask, AgentError> {
    conn.execute(
        "INSERT INTO agent_tasks (session_id, tool_name, status, input_json)
         VALUES (?1, ?2, 'running', ?3)",
        rusqlite::params![session_id, tool_name, input_json],
    )?;
    let id = conn.last_insert_rowid();
    get_task(conn, id)
}

pub fn complete_task(
    conn: &Connection,
    task_id: i64,
    status: &str,
    result_json: Option<&str>,
    error_message: Option<&str>,
) -> Result<AgentTask, AgentError> {
    conn.execute(
        "UPDATE agent_tasks
         SET status = ?1,
             result_json = ?2,
             error_message = ?3,
             updated_at = datetime('now'),
             completed_at = datetime('now')
         WHERE id = ?4",
        rusqlite::params![status, result_json, error_message, task_id],
    )?;
    get_task(conn, task_id)
}

pub fn get_task(conn: &Connection, id: i64) -> Result<AgentTask, AgentError> {
    conn.query_row(
        "SELECT id, session_id, tool_name, status, input_json, result_json, error_message,
                created_at, updated_at, completed_at
         FROM agent_tasks
         WHERE id = ?1",
        [id],
        map_task,
    )
    .map_err(AgentError::from)
}

pub fn list_session_tasks(
    conn: &Connection,
    session_id: i64,
) -> Result<Vec<AgentTask>, AgentError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, tool_name, status, input_json, result_json, error_message,
                created_at, updated_at, completed_at
         FROM agent_tasks
         WHERE session_id = ?1
         ORDER BY id DESC",
    )?;
    let tasks = stmt
        .query_map([session_id], map_task)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(tasks)
}

pub fn insert_artifact(
    conn: &Connection,
    session_id: i64,
    task_id: Option<i64>,
    artifact_type: &str,
    title: &str,
    file_path: Option<&str>,
    mime_type: Option<&str>,
    byte_size: Option<i64>,
    metadata_json: Option<&str>,
) -> Result<AgentArtifact, AgentError> {
    conn.execute(
        "INSERT INTO agent_artifacts
            (session_id, task_id, artifact_type, title, file_path, mime_type, byte_size, metadata_json)
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
        rusqlite::params![
            session_id,
            task_id,
            artifact_type,
            title,
            file_path,
            mime_type,
            byte_size,
            metadata_json
        ],
    )?;
    let id = conn.last_insert_rowid();
    get_artifact(conn, id)
}

pub fn get_artifact(conn: &Connection, id: i64) -> Result<AgentArtifact, AgentError> {
    conn.query_row(
        "SELECT id, session_id, task_id, artifact_type, title, file_path, mime_type,
                byte_size, metadata_json, created_at
         FROM agent_artifacts
         WHERE id = ?1",
        [id],
        map_artifact,
    )
    .map_err(AgentError::from)
}

pub fn list_session_artifacts(
    conn: &Connection,
    session_id: i64,
) -> Result<Vec<AgentArtifact>, AgentError> {
    let mut stmt = conn.prepare(
        "SELECT id, session_id, task_id, artifact_type, title, file_path, mime_type,
                byte_size, metadata_json, created_at
         FROM agent_artifacts
         WHERE session_id = ?1
         ORDER BY id DESC",
    )?;
    let artifacts = stmt
        .query_map([session_id], map_artifact)?
        .collect::<Result<Vec<_>, _>>()?;
    Ok(artifacts)
}

pub fn delete_artifact(
    conn: &Connection,
    session_id: i64,
    artifact_id: i64,
) -> Result<(), AgentError> {
    conn.execute(
        "DELETE FROM agent_artifacts WHERE id = ?1 AND session_id = ?2",
        rusqlite::params![artifact_id, session_id],
    )?;
    Ok(())
}

fn link_attachments_to_message(
    conn: &Connection,
    session_id: i64,
    message_id: i64,
    attachment_ids: &[i64],
) -> Result<(), AgentError> {
    for id in attachment_ids {
        conn.execute(
            "UPDATE agent_attachments SET message_id = ?1 WHERE id = ?2 AND session_id = ?3",
            rusqlite::params![message_id, id, session_id],
        )?;
    }
    Ok(())
}

fn attach_attachments(
    conn: &Connection,
    messages: &mut [AgentMessageRow],
) -> Result<(), AgentError> {
    for message in messages {
        let mut stmt = conn.prepare(
            "SELECT id, session_id, message_id, original_name, mime_type, byte_size, storage_path, created_at
             FROM agent_attachments
             WHERE message_id = ?1
             ORDER BY id ASC",
        )?;
        message.attachments = stmt
            .query_map([message.id], map_attachment)?
            .collect::<Result<Vec<_>, _>>()?;
    }
    Ok(())
}

fn map_attachment(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentAttachment> {
    Ok(AgentAttachment {
        id: row.get(0)?,
        session_id: row.get(1)?,
        message_id: row.get(2)?,
        original_name: row.get(3)?,
        mime_type: row.get(4)?,
        byte_size: row.get(5)?,
        storage_path: row.get(6)?,
        created_at: row.get(7)?,
    })
}

fn map_task(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentTask> {
    Ok(AgentTask {
        id: row.get(0)?,
        session_id: row.get(1)?,
        tool_name: row.get(2)?,
        status: row.get(3)?,
        input_json: row.get(4)?,
        result_json: row.get(5)?,
        error_message: row.get(6)?,
        created_at: row.get(7)?,
        updated_at: row.get(8)?,
        completed_at: row.get(9)?,
    })
}

fn map_artifact(row: &rusqlite::Row<'_>) -> rusqlite::Result<AgentArtifact> {
    Ok(AgentArtifact {
        id: row.get(0)?,
        session_id: row.get(1)?,
        task_id: row.get(2)?,
        artifact_type: row.get(3)?,
        title: row.get(4)?,
        file_path: row.get(5)?,
        mime_type: row.get(6)?,
        byte_size: row.get(7)?,
        metadata_json: row.get(8)?,
        created_at: row.get(9)?,
    })
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
        "INSERT INTO agent_messages (session_id, role, content, tool_call_json, tool_call_id) VALUES (?1, ?2, ?3, ?4, ?5)",
        rusqlite::params![session_id, msg.role, content, tool_call_json, msg.tool_call_id],
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
        tool_call_id: msg.tool_call_id.clone(),
        created_at: String::new(),
        attachments: Vec::new(),
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
    audit: Option<&LlmAuditConfig>,
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
    let started_at = chrono::Utc::now();
    let started = std::time::Instant::now();

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
        stream: None,
    };
    let endpoint = format!("{base_url}/chat/completions");
    let request_json = serde_json::to_value(&request)?;

    let response = match client
        .post(&endpoint)
        .headers(headers(api_key)?)
        .json(&request)
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            write_llm_audit_record(
                audit,
                LlmAuditRecord {
                    started_at,
                    operation: "agent_chat",
                    endpoint: &endpoint,
                    model,
                    duration_ms: started.elapsed().as_millis(),
                    status: None,
                    request: request_json,
                    response: None,
                    error: Some(err.to_string()),
                },
            );
            return Err(err.into());
        }
    };

    let status = response.status();
    let status_code = status.as_u16();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        error!(
            "Agent: LLM HTTP {status}: {}",
            crate::llm::truncate(&body, 200)
        );
        write_llm_audit_record(
            audit,
            LlmAuditRecord {
                started_at,
                operation: "agent_chat",
                endpoint: &endpoint,
                model,
                duration_ms: started.elapsed().as_millis(),
                status: Some(status_code),
                request: request_json,
                response: Some(body_to_value(&body)),
                error: Some(format!("HTTP {status_code}")),
            },
        );
        return Err(LlmError::ProviderStatus {
            status: status_code,
            body: crate::llm::truncate(&body, 500),
        }
        .into());
    }

    let body_text = response.text().await?;
    write_llm_audit_record(
        audit,
        LlmAuditRecord {
            started_at,
            operation: "agent_chat",
            endpoint: &endpoint,
            model,
            duration_ms: started.elapsed().as_millis(),
            status: Some(status_code),
            request: request_json,
            response: Some(body_to_value(&body_text)),
            error: None,
        },
    );
    let body: ToolChatResponse = serde_json::from_str(&body_text)?;
    Ok(body)
}

async fn send_chat_request_stream(
    messages: Vec<LlmMessage>,
    config: &LlmProviderConfig,
    stream_sink: &AgentStreamSink,
    audit: Option<&LlmAuditConfig>,
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
    let started_at = chrono::Utc::now();
    let started = std::time::Instant::now();

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
        stream: Some(true),
    };
    let endpoint = format!("{base_url}/chat/completions");
    let request_json = serde_json::to_value(&request)?;

    let mut request_headers = headers(api_key)?;
    request_headers.insert(
        reqwest::header::ACCEPT,
        reqwest::header::HeaderValue::from_static("text/event-stream"),
    );

    let mut response = match client
        .post(&endpoint)
        .headers(request_headers)
        .json(&request)
        .send()
        .await
    {
        Ok(response) => response,
        Err(err) => {
            write_llm_audit_record(
                audit,
                LlmAuditRecord {
                    started_at,
                    operation: "agent_chat_stream",
                    endpoint: &endpoint,
                    model,
                    duration_ms: started.elapsed().as_millis(),
                    status: None,
                    request: request_json,
                    response: None,
                    error: Some(err.to_string()),
                },
            );
            return Err(err.into());
        }
    };

    let status = response.status();
    let status_code = status.as_u16();
    if !status.is_success() {
        let body = response.text().await.unwrap_or_default();
        error!(
            "Agent stream: LLM HTTP {status}: {}",
            crate::llm::truncate(&body, 200)
        );
        write_llm_audit_record(
            audit,
            LlmAuditRecord {
                started_at,
                operation: "agent_chat_stream",
                endpoint: &endpoint,
                model,
                duration_ms: started.elapsed().as_millis(),
                status: Some(status_code),
                request: request_json,
                response: Some(body_to_value(&body)),
                error: Some(format!("HTTP {status_code}")),
            },
        );
        return Err(LlmError::ProviderStatus {
            status: status_code,
            body: crate::llm::truncate(&body, 500),
        }
        .into());
    }

    let mut buffer = String::new();
    let mut content = String::new();
    let mut tool_calls: Vec<ToolCallAccumulator> = Vec::new();
    let mut done = false;
    let mut stream_events: Vec<serde_json::Value> = Vec::new();

    while let Some(chunk) = response.chunk().await? {
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        buffer = buffer.replace("\r\n", "\n");

        while let Some(index) = buffer.find("\n\n") {
            let event = buffer[..index].to_owned();
            buffer = buffer[index + 2..].to_owned();
            if process_sse_event(
                &event,
                &mut content,
                &mut tool_calls,
                stream_sink,
                &mut stream_events,
            )? {
                done = true;
                break;
            }
        }

        if done {
            break;
        }
    }

    if !buffer.trim().is_empty() {
        let _ = process_sse_event(
            &buffer,
            &mut content,
            &mut tool_calls,
            stream_sink,
            &mut stream_events,
        )?;
    }

    let tool_calls = tool_calls
        .into_iter()
        .enumerate()
        .filter_map(|(index, call)| {
            if call.name.is_empty() {
                return None;
            }
            Some(ToolCall {
                id: call.id.unwrap_or_else(|| format!("call_{index}")),
                type_: call.type_.unwrap_or_else(|| "function".to_owned()),
                function: ToolCallFunction {
                    name: call.name,
                    arguments: call.arguments,
                },
            })
        })
        .collect::<Vec<_>>();

    let response = ToolChatResponse {
        choices: vec![ToolChatChoice {
            message: ToolChatMessage {
                role: Some("assistant".to_owned()),
                content: if content.is_empty() {
                    None
                } else {
                    Some(content)
                },
                tool_calls: if tool_calls.is_empty() {
                    None
                } else {
                    Some(tool_calls)
                },
            },
        }],
    };
    write_llm_audit_record(
        audit,
        LlmAuditRecord {
            started_at,
            operation: "agent_chat_stream",
            endpoint: &endpoint,
            model,
            duration_ms: started.elapsed().as_millis(),
            status: Some(status_code),
            request: request_json,
            response: Some(serde_json::json!({
                "stream_events": stream_events,
                "assembled": response,
            })),
            error: None,
        },
    );
    Ok(response)
}

fn process_sse_event(
    event: &str,
    content: &mut String,
    tool_calls: &mut Vec<ToolCallAccumulator>,
    stream_sink: &AgentStreamSink,
    audit_events: &mut Vec<serde_json::Value>,
) -> Result<bool, AgentError> {
    for raw_line in event.lines() {
        let line = raw_line.trim_start();
        let Some(data) = line.strip_prefix("data:") else {
            continue;
        };
        let data = data.trim_start();
        if data.is_empty() {
            continue;
        }
        if data == "[DONE]" {
            return Ok(true);
        }

        audit_events.push(body_to_value(data));
        let chunk: ToolChatStreamChunk = serde_json::from_str(data)?;
        for choice in chunk.choices {
            if let Some(delta) = choice.delta.content {
                if !delta.is_empty() {
                    content.push_str(&delta);
                    stream_sink(AgentStreamEvent::AssistantDelta { delta });
                }
            }

            if let Some(deltas) = choice.delta.tool_calls {
                for delta in deltas {
                    while tool_calls.len() <= delta.index {
                        tool_calls.push(ToolCallAccumulator::default());
                    }
                    let call = &mut tool_calls[delta.index];
                    if let Some(id) = delta.id {
                        call.id = Some(id);
                    }
                    if let Some(type_) = delta.type_ {
                        call.type_ = Some(type_);
                    }
                    if let Some(function) = delta.function {
                        if let Some(name) = function.name {
                            if !name.is_empty() {
                                call.name.push_str(&name);
                                stream_sink(AgentStreamEvent::ToolCall {
                                    tool_name: call.name.clone(),
                                });
                            }
                        }
                        if let Some(arguments) = function.arguments {
                            call.arguments.push_str(&arguments);
                        }
                    }
                }
            }
        }
    }

    Ok(false)
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
    attachment_ids: Vec<i64>,
    attachment_context: Option<String>,
    config: &LlmProviderConfig,
    audit: Option<LlmAuditConfig>,
    execute_tool: Arc<dyn Fn(&str, &serde_json::Value) -> ToolExecResult + Send + Sync>,
) -> Result<AgentResponse, AgentError> {
    run_agent_turn_impl(
        db,
        session_id,
        user_message,
        attachment_ids,
        attachment_context,
        config,
        audit,
        execute_tool,
        None,
    )
    .await
}

pub async fn run_agent_turn_stream(
    db: &Mutex<Connection>,
    session_id: i64,
    user_message: &str,
    attachment_ids: Vec<i64>,
    attachment_context: Option<String>,
    config: &LlmProviderConfig,
    audit: Option<LlmAuditConfig>,
    execute_tool: Arc<dyn Fn(&str, &serde_json::Value) -> ToolExecResult + Send + Sync>,
    stream_sink: AgentStreamSink,
) -> Result<AgentResponse, AgentError> {
    run_agent_turn_impl(
        db,
        session_id,
        user_message,
        attachment_ids,
        attachment_context,
        config,
        audit,
        execute_tool,
        Some(stream_sink),
    )
    .await
}

async fn run_agent_turn_impl(
    db: &Mutex<Connection>,
    session_id: i64,
    user_message: &str,
    attachment_ids: Vec<i64>,
    attachment_context: Option<String>,
    config: &LlmProviderConfig,
    audit: Option<LlmAuditConfig>,
    execute_tool: Arc<dyn Fn(&str, &serde_json::Value) -> ToolExecResult + Send + Sync>,
    stream_sink: Option<AgentStreamSink>,
) -> Result<AgentResponse, AgentError> {
    let mut new_messages: Vec<AgentMessageRow> = Vec::new();

    // Save the visible user message, but send attachment context to the model.
    let user_msg = LlmMessage {
        role: "user".to_owned(),
        content: Some(user_message.to_owned()),
        tool_calls: None,
        tool_call_id: None,
    };
    let llm_user_msg = LlmMessage {
        role: "user".to_owned(),
        content: Some(match attachment_context {
            Some(ctx) if !ctx.is_empty() => format!("{user_message}\n\n{ctx}"),
            _ => user_message.to_owned(),
        }),
        tool_calls: None,
        tool_call_id: None,
    };
    {
        let conn = db.lock().expect("db lock");
        let saved = save_message(&conn, &user_msg, session_id)?;
        link_attachments_to_message(&conn, session_id, saved.id, &attachment_ids)?;
        let mut saved_messages = vec![saved];
        attach_attachments(&conn, &mut saved_messages)?;
        new_messages.extend(saved_messages);
        update_session_title(&conn, session_id)?;
    }

    // Load history for context (last 20 messages)
    let history = {
        let conn = db.lock().expect("db lock");
        get_recent_messages(&conn, session_id, 20)?
    };

    // Build initial message list
    let mut llm_messages = build_llm_messages(&history);
    llm_messages.push(llm_user_msg);

    // Tool calling loop
    let result = run_agent_loop_from_inner(
        db,
        session_id,
        llm_messages,
        config,
        audit.as_ref(),
        execute_tool,
        stream_sink,
    )
    .await?;
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
    audit: Option<LlmAuditConfig>,
    execute_tool: Arc<dyn Fn(&str, &serde_json::Value) -> ToolExecResult + Send + Sync>,
) -> Result<AgentResponse, AgentError> {
    continue_agent_turn_impl(
        db,
        session_id,
        confirmed,
        extra_params,
        config,
        audit,
        execute_tool,
        None,
    )
    .await
}

pub async fn continue_agent_turn_stream(
    db: &Mutex<Connection>,
    session_id: i64,
    confirmed: bool,
    extra_params: Option<serde_json::Value>,
    config: &LlmProviderConfig,
    audit: Option<LlmAuditConfig>,
    execute_tool: Arc<dyn Fn(&str, &serde_json::Value) -> ToolExecResult + Send + Sync>,
    stream_sink: AgentStreamSink,
) -> Result<AgentResponse, AgentError> {
    continue_agent_turn_impl(
        db,
        session_id,
        confirmed,
        extra_params,
        config,
        audit,
        execute_tool,
        Some(stream_sink),
    )
    .await
}

async fn continue_agent_turn_impl(
    db: &Mutex<Connection>,
    session_id: i64,
    confirmed: bool,
    extra_params: Option<serde_json::Value>,
    config: &LlmProviderConfig,
    audit: Option<LlmAuditConfig>,
    execute_tool: Arc<dyn Fn(&str, &serde_json::Value) -> ToolExecResult + Send + Sync>,
    stream_sink: Option<AgentStreamSink>,
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

        let rest = run_agent_loop_from_inner(
            db,
            session_id,
            llm_messages,
            config,
            audit.as_ref(),
            execute_tool,
            stream_sink,
        )
        .await?;
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

        if let Some(sink) = &stream_sink {
            sink(AgentStreamEvent::ToolCall {
                tool_name: tc.function.name.clone(),
            });
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
        if let Some(sink) = &stream_sink {
            sink(AgentStreamEvent::ToolResult {
                tool_name: tc.function.name.clone(),
            });
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
        if let Some(sink) = &stream_sink {
            sink(AgentStreamEvent::ToolResult {
                tool_name: tc.function.name.clone(),
            });
        }
        llm_messages.push(tool_msg);
    }

    // Continue loop
    let rest = run_agent_loop_from_inner(
        db,
        session_id,
        llm_messages,
        config,
        audit.as_ref(),
        execute_tool,
        stream_sink,
    )
    .await?;
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
    audit: Option<&LlmAuditConfig>,
    execute_tool: Arc<dyn Fn(&str, &serde_json::Value) -> ToolExecResult + Send + Sync>,
    stream_sink: Option<AgentStreamSink>,
) -> Result<AgentResponse, AgentError> {
    let mut new_messages: Vec<AgentMessageRow> = Vec::new();
    const MAX_ITERATIONS: usize = 5;

    for _iteration in 0..MAX_ITERATIONS {
        let response = match &stream_sink {
            Some(sink) => {
                send_chat_request_stream(llm_messages.clone(), config, sink, audit).await?
            }
            None => send_chat_request(llm_messages.clone(), config, audit).await?,
        };
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
                if let Some(sink) = &stream_sink {
                    sink(AgentStreamEvent::ToolCall {
                        tool_name: tc.function.name.clone(),
                    });
                }
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
                        if let Some(sink) = &stream_sink {
                            sink(AgentStreamEvent::ToolResult {
                                tool_name: tc.function.name.clone(),
                            });
                        }
                        tool_result_msgs.push(tool_msg);
                    }
                    ToolExecResult::ConfirmationRequired {
                        tool_name,
                        arguments,
                        message,
                    } => {
                        let pending_confirmation = PendingConfirmation {
                            tool_name,
                            arguments,
                            message,
                        };
                        if let Some(sink) = &stream_sink {
                            sink(AgentStreamEvent::PendingConfirmation {
                                pending_confirmation: pending_confirmation.clone(),
                            });
                        }
                        return Ok(AgentResponse {
                            messages: new_messages,
                            pending_confirmation: Some(pending_confirmation),
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
                        if let Some(sink) = &stream_sink {
                            sink(AgentStreamEvent::ToolResult {
                                tool_name: tc.function.name.clone(),
                            });
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
        "SELECT id, session_id, role, content, tool_call_json, tool_call_id, created_at
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
                tool_call_id: row.get(5)?,
                created_at: row.get(6)?,
                attachments: Vec::new(),
            })
        })?
        .collect::<Result<Vec<_>, _>>()?;
    msgs.reverse();
    attach_attachments(conn, &mut msgs)?;
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
            tool_call_id: m.tool_call_id.clone(),
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
