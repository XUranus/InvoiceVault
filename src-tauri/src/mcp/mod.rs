use std::io::{self, BufRead, Write};
use std::path::{Path, PathBuf};

use chrono::{Datelike, Local};
use rusqlite::Connection;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

use crate::extractor;
use crate::exporter;

// ---------------------------------------------------------------------------
// JSON-RPC 2.0 types
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct JsonRpcRequest {
    #[serde(default)]
    id: Option<Value>,
    method: String,
    #[serde(default)]
    params: Option<Value>,
}

#[derive(Debug, Serialize)]
struct JsonRpcResponse {
    jsonrpc: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    error: Option<JsonRpcError>,
}

#[derive(Debug, Serialize)]
struct JsonRpcError {
    code: i32,
    message: String,
}

// ---------------------------------------------------------------------------
// MCP types
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
struct McpTool {
    name: &'static str,
    description: &'static str,
    #[serde(rename = "inputSchema")]
    input_schema: Value,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type")]
enum McpContent {
    #[serde(rename = "text")]
    Text { text: String },
}

#[derive(Debug, Serialize)]
struct McpToolResult {
    content: Vec<McpContent>,
    #[serde(rename = "isError", skip_serializing_if = "std::ops::Not::not")]
    is_error: bool,
}

// ---------------------------------------------------------------------------
// Tool definitions
// ---------------------------------------------------------------------------

fn tool_definitions() -> Vec<McpTool> {
    vec![
        McpTool {
            name: "search_invoices",
            description: "搜索发票，支持关键词、日期范围、销售方、发票类型、类别、状态等筛选条件。返回分页结果。",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "query": {"type": "string", "description": "关键词搜索，匹配销售方、购买方、发票号码、备注等"},
                    "date_from": {"type": "string", "description": "开始日期 YYYY-MM-DD"},
                    "date_to": {"type": "string", "description": "结束日期 YYYY-MM-DD"},
                    "seller_name": {"type": "string"},
                    "buyer_name": {"type": "string"},
                    "invoice_number": {"type": "string"},
                    "invoice_type": {"type": "string"},
                    "category": {"type": "string"},
                    "status": {"type": "string"},
                    "duplicate_status": {"type": "string"},
                    "amount_min": {"type": "string"},
                    "amount_max": {"type": "string"},
                    "sort_by": {"type": "string"},
                    "sort_order": {"type": "string"},
                    "page": {"type": "integer"},
                    "page_size": {"type": "integer"}
                }
            }),
        },
        McpTool {
            name: "get_invoice_detail",
            description: "获取单张发票的完整详情，包括所有字段和明细行。",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "invoice_id": {"type": "integer", "description": "发票 ID"}
                },
                "required": ["invoice_id"]
            }),
        },
        McpTool {
            name: "get_dashboard_stats",
            description: "获取仪表盘统计数据：发票总数、金额合计、月度趋势、类型分布、状态分布、Top 供应商排名。",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "date_from": {"type": "string", "description": "开始日期 YYYY-MM-DD，可选"},
                    "date_to": {"type": "string", "description": "结束日期 YYYY-MM-DD，可选"}
                }
            }),
        },
        McpTool {
            name: "get_current_date_context",
            description: "获取当前日期上下文，用于解析这个月、上个月、本季度等相对时间表达。",
            input_schema: json!({"type": "object", "properties": {}}),
        },
        McpTool {
            name: "get_invoice_field_catalog",
            description: "获取发票字段字典，包括可导出字段 key、中文名、别名和数据类型。用于把用户说的列名映射为导出字段。",
            input_schema: json!({"type": "object", "properties": {}}),
        },
        McpTool {
            name: "export_invoices",
            description: "导出发票为 CSV 或 Excel 文件。输出到指定路径。",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "format": {"type": "string", "enum": ["csv", "xlsx"], "description": "导出格式"},
                    "output_path": {"type": "string", "description": "输出文件路径"},
                    "invoice_ids": {"type": "array", "items": {"type": "integer"}, "description": "发票 ID 列表，为空则导出全部"},
                    "columns": {"type": "array", "items": {"type": "string"}, "description": "导出字段 key 列表"},
                    "date_from": {"type": "string"},
                    "date_to": {"type": "string"}
                },
                "required": ["format", "output_path"]
            }),
        },
        McpTool {
            name: "create_export_preview",
            description: "预览一次发票导出，返回匹配行数、导出列和前几行样例。",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "invoice_ids": {"type": "array", "items": {"type": "integer"}},
                    "columns": {"type": "array", "items": {"type": "string"}},
                    "date_from": {"type": "string"},
                    "date_to": {"type": "string"},
                    "limit": {"type": "integer"}
                }
            }),
        },
        McpTool {
            name: "update_invoice",
            description: "更新发票的字段信息。",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "id": {"type": "integer", "description": "发票 ID"},
                    "invoice_type": {"type": "string"},
                    "invoice_code": {"type": "string"},
                    "invoice_number": {"type": "string"},
                    "issue_date": {"type": "string"},
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
        },
        McpTool {
            name: "merge_invoices",
            description: "将多张发票合并为一张。要求所有发票属于同一文件。",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "target_invoice_id": {"type": "integer", "description": "目标发票 ID"},
                    "source_invoice_ids": {"type": "array", "items": {"type": "integer"}, "description": "源发票 ID 列表"}
                },
                "required": ["target_invoice_id", "source_invoice_ids"]
            }),
        },
        McpTool {
            name: "export_pdf_report",
            description: "将选中发票导出为 PDF 报表。报表包含汇总表和每张发票的详情页。",
            input_schema: json!({
                "type": "object",
                "properties": {
                    "output_path": {"type": "string", "description": "输出 PDF 文件路径"},
                    "invoice_ids": {"type": "array", "items": {"type": "integer"}},
                    "date_from": {"type": "string"},
                    "date_to": {"type": "string"}
                },
                "required": ["output_path"]
            }),
        },
    ]
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn json_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(|v| v.as_str())
}

fn json_i64_vec(args: &Value, key: &str) -> Option<Vec<i64>> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_i64()).collect())
}

fn json_string_vec(args: &Value, key: &str) -> Option<Vec<String>> {
    args.get(key)
        .and_then(|v| v.as_array())
        .map(|arr| arr.iter().filter_map(|v| v.as_str().map(String::from)).collect())
}

fn success_text(text: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id: None,
        result: Some(
            serde_json::to_value(McpToolResult {
                content: vec![McpContent::Text { text }],
                is_error: false,
            })
            .unwrap_or(Value::Null),
        ),
        error: None,
    }
}

fn error_text(text: String) -> JsonRpcResponse {
    JsonRpcResponse {
        jsonrpc: "2.0",
        id: None,
        result: Some(
            serde_json::to_value(McpToolResult {
                content: vec![McpContent::Text { text }],
                is_error: true,
            })
            .unwrap_or(Value::Null),
        ),
        error: None,
    }
}

// ---------------------------------------------------------------------------
// Tool execution
// ---------------------------------------------------------------------------

fn execute_tool(
    tool_name: &str,
    args: &Value,
    conn: &Connection,
    app_data_dir: &Path,
) -> JsonRpcResponse {
    match tool_name {
        "search_invoices" => {
            let params = extractor::InvoiceSearchParams {
                query: json_str(args, "query").map(String::from),
                invoice_type: json_str(args, "invoice_type").map(String::from),
                seller_name: json_str(args, "seller_name").map(String::from),
                buyer_name: json_str(args, "buyer_name").map(String::from),
                invoice_number: json_str(args, "invoice_number").map(String::from),
                date_from: json_str(args, "date_from").map(String::from),
                date_to: json_str(args, "date_to").map(String::from),
                amount_min: json_str(args, "amount_min").map(String::from),
                amount_max: json_str(args, "amount_max").map(String::from),
                category: json_str(args, "category").map(String::from),
                tag: None,
                status: json_str(args, "status").map(String::from),
                duplicate_status: json_str(args, "duplicate_status").map(String::from),
                sort_by: json_str(args, "sort_by").map(String::from),
                sort_order: json_str(args, "sort_order").map(String::from),
                page: args.get("page").and_then(|v| v.as_i64()),
                page_size: args.get("page_size").and_then(|v| v.as_i64()),
            };
            match extractor::search_invoices(conn, params) {
                Ok(result) => success_text(serde_json::to_string_pretty(&result).unwrap_or_default()),
                Err(e) => error_text(format!("搜索失败: {e}")),
            }
        }
        "get_invoice_detail" => {
            let Some(id) = args.get("invoice_id").and_then(|v| v.as_i64()) else {
                return error_text("缺少 invoice_id 参数".into());
            };
            let thumbnails_dir = app_data_dir.join("thumbnails");
            match extractor::get_invoice_detail(conn, &thumbnails_dir, id) {
                Ok(detail) => success_text(serde_json::to_string_pretty(&detail).unwrap_or_default()),
                Err(e) => error_text(format!("获取详情失败: {e}")),
            }
        }
        "get_dashboard_stats" => {
            let date_from = json_str(args, "date_from");
            let date_to = json_str(args, "date_to");
            match extractor::get_dashboard_stats(conn, date_from, date_to) {
                Ok(stats) => success_text(serde_json::to_string_pretty(&stats).unwrap_or_default()),
                Err(e) => error_text(format!("获取统计失败: {e}")),
            }
        }
        "get_current_date_context" => {
            let now = Local::now().date_naive();
            let month_start = now.with_day(1).unwrap_or(now);
            let next_month = if now.month() == 12 {
                chrono::NaiveDate::from_ymd_opt(now.year() + 1, 1, 1).unwrap_or(month_start)
            } else {
                chrono::NaiveDate::from_ymd_opt(now.year(), now.month() + 1, 1).unwrap_or(month_start)
            };
            let month_end = next_month.pred_opt().unwrap_or(now);
            let content = json!({
                "today": now.to_string(),
                "current_month": {
                    "date_from": month_start.to_string(),
                    "date_to": month_end.to_string()
                },
                "year": now.year(),
                "month": now.month()
            });
            success_text(serde_json::to_string_pretty(&content).unwrap_or_default())
        }
        "get_invoice_field_catalog" => {
            let catalog = exporter::export_column_catalog();
            success_text(serde_json::to_string_pretty(&catalog).unwrap_or_default())
        }
        "export_invoices" => {
            let Some(format) = json_str(args, "format") else {
                return error_text("缺少 format 参数".into());
            };
            let Some(output_path) = json_str(args, "output_path") else {
                return error_text("缺少 output_path 参数".into());
            };
            let request = exporter::ExportInvoicesRequest {
                format: format.to_string(),
                output_path: output_path.to_string(),
                invoice_ids: json_i64_vec(args, "invoice_ids"),
                columns: json_string_vec(args, "columns"),
                date_from: json_str(args, "date_from").map(String::from),
                date_to: json_str(args, "date_to").map(String::from),
            };
            match exporter::export_invoices(conn, request) {
                Ok(result) => success_text(serde_json::to_string_pretty(&result).unwrap_or_default()),
                Err(e) => error_text(format!("导出失败: {e}")),
            }
        }
        "create_export_preview" => {
            let request = exporter::ExportPreviewRequest {
                invoice_ids: json_i64_vec(args, "invoice_ids"),
                columns: json_string_vec(args, "columns"),
                date_from: json_str(args, "date_from").map(String::from),
                date_to: json_str(args, "date_to").map(String::from),
                limit: args.get("limit").and_then(|v| v.as_u64()).map(|v| v as usize),
            };
            match exporter::preview_export(conn, request) {
                Ok(result) => success_text(serde_json::to_string_pretty(&result).unwrap_or_default()),
                Err(e) => error_text(format!("预览失败: {e}")),
            }
        }
        "update_invoice" => {
            let Some(id) = args.get("id").and_then(|v| v.as_i64()) else {
                return error_text("缺少 id 参数".into());
            };
            let request = extractor::UpdateInvoiceRequest {
                id,
                invoice_type: args.get("invoice_type").map(|v| v.as_str().map(String::from)),
                invoice_code: args.get("invoice_code").map(|v| v.as_str().map(String::from)),
                invoice_number: args.get("invoice_number").map(|v| v.as_str().map(String::from)),
                issue_date: args.get("issue_date").map(|v| v.as_str().map(String::from)),
                seller_name: args.get("seller_name").map(|v| v.as_str().map(String::from)),
                seller_tax_id: args.get("seller_tax_id").map(|v| v.as_str().map(String::from)),
                buyer_name: args.get("buyer_name").map(|v| v.as_str().map(String::from)),
                buyer_tax_id: args.get("buyer_tax_id").map(|v| v.as_str().map(String::from)),
                currency: args.get("currency").map(|v| v.as_str().map(String::from)),
                amount_without_tax: args.get("amount_without_tax").map(|v| v.as_str().map(String::from)),
                tax_amount: args.get("tax_amount").map(|v| v.as_str().map(String::from)),
                total_amount: args.get("total_amount").map(|v| v.as_str().map(String::from)),
                category: args.get("category").map(|v| v.as_str().map(String::from)),
                remarks: args.get("remarks").map(|v| v.as_str().map(String::from)),
                confidence: None,
                status: args.get("status").map(|v| v.as_str().map(String::from)),
            };
            // update_invoice takes &mut Connection, but we only have &Connection
            // Open a second connection for writes
            let db_path = app_data_dir.join("invoicevault.sqlite3");
            match rusqlite::Connection::open(&db_path) {
                Ok(mut write_conn) => match extractor::update_invoice(&mut write_conn, request) {
                    Ok(result) => success_text(serde_json::to_string_pretty(&result).unwrap_or_default()),
                    Err(e) => error_text(format!("更新失败: {e}")),
                },
                Err(e) => error_text(format!("打开数据库失败: {e}")),
            }
        }
        "merge_invoices" => {
            let Some(target_id) = args.get("target_invoice_id").and_then(|v| v.as_i64()) else {
                return error_text("缺少 target_invoice_id 参数".into());
            };
            let Some(source_ids) = json_i64_vec(args, "source_invoice_ids") else {
                return error_text("缺少 source_invoice_ids 参数".into());
            };
            let db_path = app_data_dir.join("invoicevault.sqlite3");
            match rusqlite::Connection::open(&db_path) {
                Ok(mut write_conn) => match extractor::merge_invoices(&mut write_conn, target_id, source_ids) {
                    Ok(result) => success_text(serde_json::to_string_pretty(&result).unwrap_or_default()),
                    Err(e) => error_text(format!("合并失败: {e}")),
                },
                Err(e) => error_text(format!("打开数据库失败: {e}")),
            }
        }
        "export_pdf_report" => {
            let Some(output_path) = json_str(args, "output_path") else {
                return error_text("缺少 output_path 参数".into());
            };
            let request = exporter::PdfReportRequest {
                output_path: output_path.to_string(),
                invoice_ids: json_i64_vec(args, "invoice_ids"),
                date_from: json_str(args, "date_from").map(String::from),
                date_to: json_str(args, "date_to").map(String::from),
                thumbnails_dir: Some(app_data_dir.join("thumbnails").to_string_lossy().to_string()),
            };
            match exporter::export_pdf_report(conn, request) {
                Ok(result) => success_text(serde_json::to_string_pretty(&result).unwrap_or_default()),
                Err(e) => error_text(format!("导出 PDF 失败: {e}")),
            }
        }
        _ => error_text(format!("未知工具: {tool_name}")),
    }
}

// ---------------------------------------------------------------------------
// Request handling
// ---------------------------------------------------------------------------

fn handle_request(req: JsonRpcRequest, conn: &Connection, app_data_dir: &Path) -> Option<JsonRpcResponse> {
    // Notifications (no id) return no response
    if req.id.is_none() && req.method.starts_with("notifications/") {
        return None;
    }

    let resp = match req.method.as_str() {
        "initialize" => JsonRpcResponse {
            jsonrpc: "2.0",
            id: req.id.clone(),
            result: Some(json!({
                "protocolVersion": "2025-03-26",
                "capabilities": {
                    "tools": {}
                },
                "serverInfo": {
                    "name": "invoicevault",
                    "version": env!("CARGO_PKG_VERSION")
                }
            })),
            error: None,
        },
        "tools/list" => {
            let tools = tool_definitions();
            JsonRpcResponse {
                jsonrpc: "2.0",
                id: req.id.clone(),
                result: Some(json!({ "tools": tools })),
                error: None,
            }
        }
        "tools/call" => {
            let params = req.params.unwrap_or(Value::Null);
            let tool_name = params.get("name").and_then(|v| v.as_str()).unwrap_or("");
            let args = params.get("arguments").cloned().unwrap_or(Value::Null);
            let mut resp = execute_tool(tool_name, &args, conn, app_data_dir);
            resp.id = req.id.clone();
            return Some(resp);
        }
        _ => JsonRpcResponse {
            jsonrpc: "2.0",
            id: req.id.clone(),
            result: None,
            error: Some(JsonRpcError {
                code: -32601,
                message: format!("Method not found: {}", req.method),
            }),
        },
    };

    Some(resp)
}

// ---------------------------------------------------------------------------
// Entry point
// ---------------------------------------------------------------------------

pub fn run_server(conn: Connection, app_data_dir: PathBuf) {
    let stdin = io::stdin();
    let mut stdout = io::stdout();

    for line in stdin.lock().lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }

        let req: JsonRpcRequest = match serde_json::from_str(&line) {
            Ok(r) => r,
            Err(e) => {
                let resp = JsonRpcResponse {
                    jsonrpc: "2.0",
                    id: None,
                    result: None,
                    error: Some(JsonRpcError {
                        code: -32700,
                        message: format!("Parse error: {e}"),
                    }),
                };
                let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap_or_default());
                let _ = stdout.flush();
                continue;
            }
        };

        if let Some(resp) = handle_request(req, &conn, &app_data_dir) {
            let _ = writeln!(stdout, "{}", serde_json::to_string(&resp).unwrap_or_default());
            let _ = stdout.flush();
        }
    }
}
