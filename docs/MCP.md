# InvoiceVault MCP Server

## 概述

`invoicevault-mcp` 是一个独立的 CLI 程序，通过 [Model Context Protocol](https://modelcontextprotocol.io) (MCP) 暴露 InvoiceVault 的核心能力。它使用 stdio 传输，直接打开 InvoiceVault 的 SQLite 数据库，无需启动 Tauri 桌面应用。

### 架构

```
Claude Code / Codex    ←——stdio JSON-RPC——→    invoicevault-mcp    ←——SQLite——→    invoicevault.sqlite3
```

- **独立二进制**：`src-tauri/src/bin/mcp_server.rs`
- **协议实现**：`src-tauri/src/mcp/mod.rs` — JSON-RPC 2.0 + MCP tools
- **数据访问**：直接复用 `extractor`、`exporter`、`storage` 模块
- **传输方式**：stdio（stdin 读请求，stdout 写响应）

## 构建

```bash
cd src-tauri
cargo build --release --bin invoicevault-mcp
```

产物在 `src-tauri/target/release/invoicevault-mcp`。

## 配置

### Claude Code

在 `~/.claude/settings.json` 中添加 `mcpServers`：

```json
{
  "mcpServers": {
    "invoicevault": {
      "command": "/absolute/path/to/invoicevault-mcp"
    }
  }
}
```

或在项目根目录创建 `.claude/mcp.json`：

```json
{
  "mcpServers": {
    "invoicevault": {
      "command": "/absolute/path/to/invoicevault-mcp"
    }
  }
}
```

### 命令行参数

```
invoicevault-mcp [OPTIONS]

Options:
  --db <path>        SQLite 数据库路径（默认：自动检测）
  --app-data <path>  App 数据目录路径（默认：~/.local/share/com.invoicevault.desktop/）
  -h, --help         显示帮助
```

默认情况下，服务器从以下路径读取：

| 文件 | 默认路径 |
|---|---|
| SQLite 数据库 | `~/.local/share/com.invoicevault.desktop/invoicevault.sqlite3` |
| 缩略图目录 | `~/.local/share/com.invoicevault.desktop/thumbnails/` |
| 原始文件目录 | `~/.local/share/com.invoicevault.desktop/raw/` |

## MCP 协议

### 支持的能力

| 能力 | 状态 |
|---|---|
| `tools` | 支持（11 个工具） |
| `resources` | 不支持 |
| `prompts` | 不支持 |

### 协议版本

`2025-03-26`

### 初始化

```
→ {"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"claude-code","version":"1.0"}}}
← {"jsonrpc":"2.0","id":1,"result":{"protocolVersion":"2025-03-26","capabilities":{"tools":{}},"serverInfo":{"name":"invoicevault","version":"0.1.0"}}}
```

### 列出工具

```
→ {"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}
← {"jsonrpc":"2.0","id":2,"result":{"tools":[...]}}
```

### 调用工具

```
→ {"jsonrpc":"2.0","id":3,"method":"tools/call","params":{"name":"search_invoices","arguments":{"query":"办公用品","page_size":10}}}
← {"jsonrpc":"2.0","id":3,"result":{"content":[{"type":"text","text":"{...}"}]}}
```

## 工具参考

### search_invoices

搜索发票，支持多维筛选。

**参数：**

| 参数 | 类型 | 说明 |
|---|---|---|
| `query` | string | 关键词（匹配销售方、购买方、发票号码、备注） |
| `date_from` | string | 开始日期 `YYYY-MM-DD` |
| `date_to` | string | 结束日期 `YYYY-MM-DD` |
| `seller_name` | string | 销售方名称 |
| `buyer_name` | string | 购买方名称 |
| `invoice_number` | string | 发票号码 |
| `invoice_type` | string | 发票类型 |
| `category` | string | 消费类别 |
| `status` | string | 状态：`pending_confirmation` / `recognized` / `reviewed` / `flagged` |
| `duplicate_status` | string | 重复状态 |
| `amount_min` | string | 最小金额 |
| `amount_max` | string | 最大金额 |
| `sort_by` | string | 排序字段 |
| `sort_order` | string | `asc` 或 `desc` |
| `page` | integer | 页码（默认 1） |
| `page_size` | integer | 每页条数（默认 50，最大 500） |

**返回：** `{invoices: InvoiceSummary[], total_count, page, page_size, total_pages}`

### get_invoice_detail

获取单张发票完整详情。

**参数：**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `invoice_id` | integer | 是 | 发票 ID |

**返回：** 完整发票对象，包含明细行 `items[]`、原始文件信息、缩略图路径、提取元数据。

### get_dashboard_stats

获取仪表盘统计数据。

**参数：**

| 参数 | 类型 | 说明 |
|---|---|---|
| `date_from` | string | 开始日期（可选） |
| `date_to` | string | 结束日期（可选） |

**返回：** 发票总数、金额合计、月度趋势、类型分布、状态分布、Top 供应商排名。

### get_current_date_context

获取当前日期上下文，用于解析"本月"、"上个月"等相对时间表达。

**参数：** 无

**返回：** `{today, current_month: {date_from, date_to}, year, month}`

### get_invoice_field_catalog

获取可导出字段字典，用于把中文列名映射为导出字段 key。

**参数：** 无

**返回：** 字段列表 `[{key, label, aliases, data_type}]`

### export_invoices

导出发票为 CSV 或 Excel 文件。

**参数：**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `format` | string | 是 | `csv` 或 `xlsx` |
| `output_path` | string | 是 | 输出文件路径 |
| `invoice_ids` | integer[] | 否 | 发票 ID 列表（为空则导出全部） |
| `columns` | string[] | 否 | 导出字段 key 列表 |
| `date_from` | string | 否 | 开始日期 |
| `date_to` | string | 否 | 结束日期 |

**返回：** `{file_path, row_count, format, byte_size, columns}`

### create_export_preview

预览导出结果，不实际写文件。

**参数：**

| 参数 | 类型 | 说明 |
|---|---|---|
| `invoice_ids` | integer[] | 发票 ID 列表 |
| `columns` | string[] | 导出字段 key 列表 |
| `date_from` | string | 开始日期 |
| `date_to` | string | 结束日期 |
| `limit` | integer | 样例行数（默认 5） |

**返回：** `{row_count, columns, sample_rows}`

### update_invoice

更新发票字段。

**参数：**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `id` | integer | 是 | 发票 ID |
| `invoice_type` | string | 否 | 发票类型 |
| `invoice_code` | string | 否 | 发票代码 |
| `invoice_number` | string | 否 | 发票号码 |
| `issue_date` | string | 否 | 开票日期 |
| `seller_name` | string | 否 | 销售方 |
| `seller_tax_id` | string | 否 | 销售方税号 |
| `buyer_name` | string | 否 | 购买方 |
| `buyer_tax_id` | string | 否 | 购买方税号 |
| `currency` | string | 否 | 币种 |
| `amount_without_tax` | string | 否 | 不含税金额 |
| `tax_amount` | string | 否 | 税额 |
| `total_amount` | string | 否 | 价税合计 |
| `category` | string | 否 | 消费类别 |
| `remarks` | string | 否 | 备注 |
| `status` | string | 否 | 状态 |

**返回：** 更新后的发票摘要和验证错误列表。

### merge_invoices

合并多张发票为一张（要求属于同一文件）。

**参数：**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `target_invoice_id` | integer | 是 | 保留的目标发票 ID |
| `source_invoice_ids` | integer[] | 是 | 要合并的源发票 ID 列表 |

**返回：** `{merged_invoice, merged_from_ids, total_items_merged}`

### export_pdf_report

导出 PDF 报表（含汇总表和详情页）。

**参数：**

| 参数 | 类型 | 必填 | 说明 |
|---|---|---|---|
| `output_path` | string | 是 | 输出 PDF 路径 |
| `invoice_ids` | integer[] | 否 | 发票 ID 列表 |
| `date_from` | string | 否 | 开始日期 |
| `date_to` | string | 否 | 结束日期 |

**返回：** `{file_path, invoice_count, byte_size}`

## 实现细节

### 文件结构

```
src-tauri/
├── src/
│   ├── bin/
│   │   └── mcp_server.rs      # MCP 服务器入口
│   ├── mcp/
│   │   └── mod.rs             # MCP 协议实现 + 工具调度
│   └── lib.rs                 # 模块声明（pub mod mcp, extractor, exporter, storage）
└── Cargo.toml                 # [[bin]] invoicevault-mcp
```

### 协议实现

`mcp/mod.rs` 实现了最小化的 MCP tools-only 服务器：

1. **JSON-RPC 2.0**：解析 `JsonRpcRequest`，生成 `JsonRpcResponse`，支持请求/响应/通知
2. **MCP 初始化**：返回 `protocolVersion: "2025-03-26"` 和 `capabilities.tools`
3. **工具注册**：11 个工具定义，每个带 `name`、`description`、`inputSchema`
4. **工具调度**：`execute_tool()` 根据工具名解析参数，调用 `extractor` / `exporter` 函数，序列化结果

### 数据访问

- **只读查询**（search, detail, stats, preview）：以 `&Connection` 调用现有函数
- **写操作**（update, merge）：打开新的 `&mut Connection` 执行
- **文件导出**（CSV, Excel, PDF）：直接写入指定路径
- **缩略图**：从 `app_data_dir/thumbnails/` 读取

### 错误处理

所有工具错误通过 `McpToolResult { is_error: true }` 返回，不会导致服务器崩溃。JSON 解析错误返回 `-32700`，未知方法返回 `-32601`。

## 调试

手动测试 MCP 服务器：

```bash
# 初始化
echo '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}' | ./invoicevault-mcp

# 列出工具
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{"protocolVersion":"2025-03-26","capabilities":{},"clientInfo":{"name":"test","version":"1.0"}}}\n{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}\n' | ./invoicevault-mcp

# 搜索发票
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"search_invoices","arguments":{}}}\n' | ./invoicevault-mcp
```

服务器启动日志写入 stderr，不会干扰 MCP 协议的 stdout 通信。
