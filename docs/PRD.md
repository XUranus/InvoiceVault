# InvoiceVault PRD

## 1. 项目概述

InvoiceVault 是一个跨平台桌面端发票处理 Agent，用于导入、识别、去重、检索、统计和导出个人或小团队的发票数据。软件以本地优先为原则：原始文件和结构化数据保存在本机，用户可配置 OpenAI-compatible 多模态大语言模型完成发票解析和 Agent 任务执行。

目标平台：

- Windows 10/11
- 主流 Linux 桌面发行版

核心技术栈：

- 桌面框架：Rust + Tauri
- 前端：Tauri WebView + React + TypeScript + Vite
- 结构化存储：SQLite
- 向量存储：ChromaDB
- LLM：OpenAI-compatible Chat Completions / Responses 风格接口，优先支持视觉输入

当前实现快照（2026-05-27）：

- 已完成 Tauri 2 + React + Rust 基础应用、SQLite 迁移（版本 9）、RAW 归档、手动导入、目录监听、图片/PDF 识别、列表详情、编辑、去重、Dashboard、CSV/Excel 导出、ChromaDB 语义搜索和语义去重。
- 已完成 Agent 基础闭环：会话、消息、工具调用、查询、详情、统计、CSV/Excel 导出、字段更新确认和工具审计日志。
- 已完成 Agent 流式输出（SSE streaming）和任务取消。
- 已完成 Excel 模板导出引擎：支持 AST 模板解析、条件区域、循环区域、汇总公式和跨 Sheet 引用。
- 已完成 SCNet 高精度 OCR 交叉验证：VLM 识别结果与 SCNet 结果自动合并。
- 已完成 MCP Server：支持工具注册、资源管理和外部客户端调用。
- 已完成事件/通知中心、日志导出、基础存储清理、自定义 Badge 标签、Linux/KDE 托盘和应用图标适配。
- 已完成跨平台构建：Windows、macOS、Linux x86_64、Linux aarch64 四平台 GitHub Actions CI/CD。
- 仍待完成编辑历史 UI、资源规模测试、加密选项。

## 2. 可行性结论

整体需求可以实现，技术选型基本合理，但需要重点控制以下风险：

1. ChromaDB 桌面分发复杂度较高。ChromaDB 常见部署形态是 Python 包或 HTTP 服务，打包到 Tauri 桌面端会增加安装包、运行时和内存占用。建议做成可替换的 `VectorStore` 接口，MVP 可先支持 ChromaDB HTTP sidecar，后续可替换为 `sqlite-vec`、`sqlite-vss` 或远程向量服务。
2. 500MB 存储上限要求需要限定 RAW 文件策略。如果用户导入大量高清图片或多页 PDF，仅原文件就可能超过 500MB。建议保留原始 PDF/图片格式并按固定目录归档，同时使用 hash 去重、缩略图压缩、可配置归档策略，并在 UI 显示存储预算。
3. 多模态 LLM 识别成本和稳定性取决于用户配置的模型。OpenAI-compatible 接口并不保证所有模型都支持图片输入、JSON schema、工具调用或长上下文。需要能力探测和降级策略。
4. 发票去重不能只依赖语义相似度。应采用多层级判定：文件 hash、页面图像 hash、发票号码/代码/日期/金额/销售方/购买方、以及 embedding 语义相似度。最终结果应允许用户确认。
5. Agent 任务执行需要工具边界。聊天窗口不应直接执行任意文件操作，而是通过受控工具查询、导出、统计、批量重命名，并在覆盖、删除、批量导出等操作前确认。

## 3. 产品目标

### 3.1 业务目标

- 降低个人和小团队整理发票的人工成本。
- 将图片/PDF 发票转为可检索、可统计、可导出的结构化数据。
- 通过重复检测减少重复导入和重复报销风险。
- 通过 AI Agent 支持自然语言查询、统计和导出。

### 3.2 用户目标

- 拖入或选择发票文件后自动识别。
- 配置监听目录后，系统能后台自动导入新发票。
- 能快速检查识别结果并修正错误字段。
- 能按月份、类型、金额、销售方、购买方等条件筛选和导出。
- 能用自然语言完成常见任务，例如导出 Excel、拆分 PDF、统计图表。

### 3.3 非目标

MVP 不包含：

- 企业级多人协作和权限系统。
- 云同步和多设备同步。
- 税务系统直连或报销系统直连。
- 完全离线本地 OCR/本地大模型。
- 对所有国家和地区票据的完整税务合规校验。

## 4. 用户画像

1. 个人用户：需要管理日常购物、差旅、医疗、交通等发票。
2. 自由职业者：需要按月或按项目整理发票并导出给会计。
3. 小团队财务助理：需要批量处理供应商票据、去重、统计和归档。

## 5. 核心工作流

### 5.1 手动导入

1. 用户在 GUI 中点击导入或拖拽文件。
2. 系统校验文件类型和大小。
3. 系统计算文件 hash，检查是否已导入。
4. 系统保存 RAW 文件到本地归档目录，保留原始 PDF/图片格式。
5. PDF 被拆分为页面或图片输入；图片被标准化。
6. 系统调用 LLM 识别发票字段。
7. 系统写入 SQLite 结构化记录，并写入向量索引。
8. UI 展示识别状态、置信度、重复候选和待确认项。

### 5.2 目录监听导入

1. 用户在设置中添加监听目录。
2. 用户选择监听文件类型、递归策略、文件稳定等待时间、是否导入后移动或保留。
3. 后台 watcher 监听新文件或变更事件。
4. 文件稳定后进入导入队列。
5. 后续流程与手动导入一致。

### 5.3 人工修正

1. 用户打开发票详情。
2. 左侧展示原始文件预览，右侧展示结构化字段。
3. 用户修改字段并保存。
4. 系统记录修改前后 diff、编辑时间和编辑来源。
5. 系统更新检索索引和统计缓存。

### 5.4 Agent 对话

1. 用户输入自然语言请求。
2. Agent 解析意图并选择内部工具。
3. 只读任务直接执行，例如查询、统计、生成图表。
4. 写入或导出任务生成执行计划并请求用户确认。
5. 确认后执行任务，展示结果文件路径、统计结果或错误说明。

## 6. 功能需求

### 6.1 发票导入

#### 6.1.1 GUI 手动导入

优先级：P0

要求：

- 支持选择单个文件、多个文件和目录。
- 支持拖拽上传。
- 支持格式：PDF、PNG、JPG、JPEG。
- 导入任务应进入后台队列，不阻塞 UI。
- 显示每个文件的导入状态：等待中、处理中、识别中、已完成、疑似重复、失败。
- 失败任务可重试。

验收标准：

- 用户可一次导入至少 100 个文件。
- UI 可实时显示进度。
- 同一文件重复导入时能提示已存在。

#### 6.1.2 目录监听

优先级：P0

要求：

- 用户可配置多个监听目录。
- 每个监听目录可配置：
  - 文件扩展名白名单。
  - 是否递归。
  - 文件稳定等待时间，默认 3 秒。
  - 是否导入历史已有文件。
  - 导入后是否移动到归档目录。
- 支持软件后台运行时自动导入。
- 支持暂停和恢复监听。

验收标准：

- 将 PDF/PNG/JPG 放入监听目录后，系统能自动导入。
- 正在写入的大文件不会被提前读取。
- watcher 异常时 UI 能显示错误并允许重启。

#### 6.1.3 多页 PDF 与多发票文件

优先级：P0

要求：

- 一个 PDF 可包含多页。
- 一个文件可包含多张发票。
- MVP 默认按页面识别，每页生成一个候选发票。
- 后续版本支持多页合并为同一发票、单页多发票切分。

验收标准：

- 多页 PDF 至少能按页生成多个识别结果。
- 用户可在 UI 中合并或拆分识别结果。

### 6.2 发票识别

优先级：P0

要求：

- 调用用户配置的 OpenAI-compatible 多模态模型解析发票。
- 对每张候选发票生成结构化 JSON。
- 记录模型名称、请求时间、耗时、token 使用量、原始响应摘要和错误信息。
- 支持识别失败重试。
- 支持字段置信度和待确认状态。
- 对模型输出做 schema 校验和类型修正。

推荐字段：

- `invoice_type`：发票类型。
- `invoice_code`：发票代码。
- `invoice_number`：发票号码。
- `issue_date`：开票日期。
- `seller_name`：销售方名称。
- `seller_tax_id`：销售方税号。
- `buyer_name`：购买方名称。
- `buyer_tax_id`：购买方税号。
- `currency`：币种。
- `amount_without_tax`：不含税金额。
- `tax_amount`：税额。
- `total_amount`：价税合计。
- `category`：消费类别。
- `items`：明细行。
- `remarks`：备注。
- `source_page_range`：源文件页码范围。
- `confidence`：整体置信度。

验收标准：

- 对清晰图片或 PDF 页面能生成结构化字段。
- 字段缺失时不会阻断入库，而是标记为待确认。
- 非发票文件能被识别为无效文档或低置信度文档。

### 6.3 存储

优先级：P0

要求：

- RAW 文件保存在本地应用数据目录，不直接依赖原路径。
- 结构化数据保存在 SQLite。
- 向量数据保存在 ChromaDB。
- 所有入库记录包含创建时间、更新时间、来源、处理状态。
- 数据库 schema 使用迁移工具管理。

RAW 文件策略：

- 使用固定归档路径：`raw/<year>/<month>/<current_name>`，文件仍保持导入时的 PDF/PNG/JPG/JPEG 原始格式。
- 数据库同时维护导入时原始文件名 `original_name` 和存储后的当前文件名 `current_name`。
- 相同文件通过 hash 去重，只存一份 RAW，多个导入记录引用同一文件。
- 为预览生成压缩缩略图，缩略图可清理重建。
- 支持用户配置存储上限和清理策略。

验收标准：

- 删除原始导入路径后，软件仍可预览已导入发票。
- 同一 RAW 文件重复导入不会重复占用存储。
- SQLite 可完整恢复结构化记录。

### 6.4 LLM 配置

优先级：P0

要求：

- 用户可配置多个 LLM Provider。
- 每个 Provider 支持：
  - 名称。
  - Base URL。
  - API Key。
  - Chat/vision model。
  - Embedding model。
  - 超时时间。
  - 最大重试次数。
  - 是否支持图片输入。
  - 是否支持 JSON schema。
  - 是否支持工具调用。
- 提供连接测试和模型能力测试。
- API Key 使用系统安全凭据存储，避免明文写入普通配置文件。

验收标准：

- 用户可新增、编辑、删除 LLM 配置。
- 配置错误时，识别任务显示明确错误。
- 可选择默认识别模型和默认 Agent 模型。

### 6.5 重复检测

优先级：P0

要求：

重复检测采用多层级策略：

- 文件级：MD5/SHA256 完全匹配。
- 页面级：渲染图片 hash 或感知 hash。
- 字段级：发票代码、号码、日期、金额、销售方、购买方。
- 内容级：明细、备注和全文摘要的 embedding 相似度。
- 时间级：导入时间和开票时间接近性。

重复状态：

- `unique`：未发现重复。
- `exact_duplicate`：文件 hash 完全重复。
- `probable_duplicate`：字段或语义高度相似。
- `possible_duplicate`：部分字段相似。
- `not_duplicate`：用户确认不是重复。

验收标准：

- 完全相同文件重复导入时自动拦截。
- 发票号码、日期、金额一致时提示高风险重复。
- 用户可确认、忽略或合并重复候选。

### 6.6 Agent 聊天窗口

优先级：P1

要求：

- 支持自然语言查询发票。
- 支持导出 Excel。
- 支持拆分/逐张导出 PDF。
- 支持生成统计结果和图表。
- 支持解释执行计划和执行结果。
- 支持任务取消。

内置工具：

- `search_invoices`：按条件查询发票。
- `get_invoice_detail`：读取单张发票详情。
- `export_invoices`：导出 CSV/Excel。
- `export_pdf_batch`：批量导出 PDF（规划）。
- `generate_chart`：生成图表数据（规划）。
- `get_dashboard_stats`：读取统计指标。
- `update_invoice`：更新发票字段，需要用户确认。

安全要求：

- Agent 不允许执行任意 shell 命令。
- 文件导出路径必须经过用户选择或在应用导出目录内。
- 覆盖文件、批量修改、删除、移动 RAW 文件前必须确认。
- 所有 Agent 操作写入审计日志。

验收标准：

- 用户输入“把 2026 年 6 月的发票导出为 Excel”时，系统能筛选并导出文件。
- 用户输入“统计这个月发票的类型，给出开支饼图”时，系统能生成图表。
- 执行写操作前 UI 显示确认面板。

### 6.7 Dashboard

优先级：P1

要求：

- 展示发票总数。
- 展示总金额、税额、月度金额趋势。
- 展示按类型、销售方、购买方、月份的统计。
- 展示待确认识别结果数量。
- 展示疑似重复数量。
- 展示存储占用和任务队列状态。

验收标准：

- Dashboard 能在应用启动后 2 秒内展示缓存统计。
- 导入或编辑发票后统计自动刷新。

### 6.8 发票内容编辑

优先级：P0

要求：

- 用户可编辑所有关键字段。
- 明细行支持增删改。
- 用户可配置自定义 Badge 分组，并在发票详情页为单张发票选择标签。
- 保存时做类型校验，例如日期、金额、税号格式。
- 保存后重新计算重复检测特征和向量索引。
- 保留编辑历史。

验收标准：

- 用户可修正识别错误并保存。
- 保存错误格式时 UI 给出字段级提示。
- 编辑后列表、详情、Dashboard 同步更新。

### 6.9 检索与列表

优先级：P0

要求：

- 发票列表支持分页、排序、筛选。
- 筛选维度包括日期、金额、发票类型、销售方、购买方、状态、重复状态。
- 支持全文搜索和语义搜索。
- 支持批量选择和批量导出。

验收标准：

- 10000 条发票记录下列表分页查询可用。
- 常用筛选条件响应时间小于 500ms。

### 6.10 导出

优先级：P1

要求：

- 支持 Excel 导出。
- 支持 CSV 导出。
- 支持按命名模板导出 PDF 或图片。
- 命名模板支持字段变量，例如 `{issue_date}_{seller_name}_{total_amount}_{invoice_number}`。
- 文件名自动处理非法字符和重名冲突。

验收标准：

- 可按月份导出 Excel。
- 可将筛选结果逐张导出为 PDF。
- 导出结果包含执行日志和失败清单。

## 7. 非功能需求

### 7.1 性能

目标约束：

- 存储占用默认控制在 500MB 内，但用户导入大量原始文件时允许提示超限。
- 内存峰值目标小于 2GB。
- 应用冷启动目标小于 5 秒。
- UI 主线程不执行耗时任务。
- 单个导入任务失败不影响队列中其他任务。

实现建议：

- PDF 渲染按页流式处理，避免一次性加载完整大文件。
- LLM 请求并发数默认 1，可配置到 2 或 3。
- 图像输入限制最大边长和压缩质量。
- 缩略图和缓存可清理重建。
- ChromaDB 作为可关闭/延迟启动组件。

### 7.2 稳定性

- 所有后台任务可恢复。
- 应用异常退出后，重启时扫描未完成任务并恢复或标记失败。
- SQLite 写入使用事务。
- 文件导入使用临时路径加原子重命名。
- watcher 事件去抖动。

### 7.3 安全与隐私

- API Key 使用系统 keyring 或平台安全存储。
- 默认不上传任何文件，只有用户配置 LLM 并启用识别时才调用外部服务。
- 提供“删除数据”功能，能删除 SQLite 记录、RAW 文件、缩略图和向量索引。
- 日志中不记录 API Key。
- 日志中默认不记录完整发票图片和完整 LLM 响应。

### 7.4 可维护性

- 后端按模块拆分。
- 所有外部服务通过 trait/interface 封装。
- 数据库迁移版本化。
- 识别 schema 版本化。
- Agent 工具有明确输入输出 schema。

## 8. 模块设计

### 8.1 前端模块

- `ImportView`：手动导入、任务队列、导入历史。
- `InvoiceListView`：列表、筛选、批量操作。
- `InvoiceDetailView`：原件预览、字段编辑、明细行编辑、重复候选。
- `DashboardView`：统计指标和图表。
- `AgentChatView`：对话、工具执行计划、确认面板、结果展示。
- `EventsView`：事件日志、工具执行和后台任务记录。
- `NotificationsView`：通知列表、未读状态和引用跳转。
- `SettingsView`：LLM 配置、Embedding、ChromaDB、监听目录、Badge、存储策略、系统状态。

### 8.2 Rust 后端模块

- `app_core`：应用生命周期、配置、错误类型。
- `storage`：SQLite 连接池、迁移、repository。
- `raw_store`：RAW 原始格式固定目录存储、缩略图、清理。
- `importer`：手动导入、任务队列、文件校验。
- `watcher`：目录监听、去抖动、稳定性检测。
- `document`：PDF 页面渲染、图片标准化、页面抽取。
- `llm`：OpenAI-compatible client、模型能力探测。
- `extractor`：发票识别 prompt、JSON schema、结果校验。
- `dedupe`：重复检测、多维相似度评分、候选管理。
- `chroma`：ChromaDB 适配器和可替换向量存储接口。
- `embedding`：OpenAI-compatible embedding 配置、连接测试和文本向量生成。
- `agent`：工具注册、任务规划、执行确认、审计日志、流式输出。
- `template_engine`：Excel 模板导出引擎，支持 AST 模板解析、条件区域、循环区域、汇总公式。
- `mcp`：MCP Server，支持工具注册、资源管理和外部客户端调用。
- `scnet_ocr`：SCNet 高精度 OCR 集成，支持增值税发票识别和 VLM 结果合并。
- `email_manager`：邮件发送管理，支持发票邮件通知。
- `exporter`：Excel/CSV 导出，后续扩展 PDF/图片批量导出。
- `event`：事件、通知和后台操作记录。

### 8.3 数据流

```text
File Input
  -> Import Queue
  -> Raw Store
  -> Document Processor
  -> LLM Extractor
  -> Schema Validation
  -> SQLite Invoice Records
  -> Dedupe Engine
  -> Vector Store
  -> UI / Agent / Dashboard
```

## 9. 数据模型草案

### 9.1 SQLite 表

`raw_files`

- `id`
- `sha256`
- `md5`
- `original_name`
- `mime_type`
- `size_bytes`
- `stored_path`
- `page_count`
- `created_at`

`import_jobs`

- `id`
- `source_type`
- `source_path`
- `raw_file_id`
- `status`
- `error_message`
- `created_at`
- `updated_at`

`invoices`

- `id`
- `raw_file_id`
- `source_page_start`
- `source_page_end`
- `invoice_type`
- `invoice_code`
- `invoice_number`
- `issue_date`
- `seller_name`
- `seller_tax_id`
- `buyer_name`
- `buyer_tax_id`
- `currency`
- `amount_without_tax`
- `tax_amount`
- `total_amount`
- `category`
- `status`
- `duplicate_status`
- `confidence`
- `schema_version`
- `created_at`
- `updated_at`

`invoice_items`

- `id`
- `invoice_id`
- `name`
- `spec`
- `unit`
- `quantity`
- `unit_price`
- `amount`
- `tax_rate`
- `tax_amount`
- `sort_order`

`extraction_runs`

- `id`
- `invoice_id`
- `raw_file_id`
- `provider_id`
- `model`
- `status`
- `prompt_version`
- `request_started_at`
- `request_finished_at`
- `latency_ms`
- `token_input`
- `token_output`
- `error_message`
- `raw_response_path`

`dedupe_candidates`

- `id`
- `invoice_id`
- `candidate_invoice_id`
- `score`
- `reason`
- `status`
- `created_at`
- `resolved_at`

`llm_providers`

- `id`
- `name`
- `base_url`
- `api_key_ref`
- `chat_model`
- `embedding_model`
- `supports_vision`
- `supports_json_schema`
- `supports_tools`
- `timeout_seconds`
- `max_retries`
- `created_at`
- `updated_at`

`watch_dirs`

- `id`
- `path`
- `extensions`
- `recursive`
- `enabled`
- `stable_wait_ms`
- `archive_after_import`
- `archive_path`
- `created_at`
- `updated_at`

`agent_sessions`

- `id`
- `title`
- `created_at`
- `updated_at`

`agent_messages`

- `id`
- `session_id`
- `role`
- `content`
- `tool_call_json`
- `created_at`

`audit_logs`

- `id`
- `actor`
- `action`
- `target_type`
- `target_id`
- `payload_json`
- `created_at`

### 9.2 ChromaDB Collection

Collection：`invoice_embeddings`

Metadata：

- `invoice_id`
- `invoice_type`
- `issue_date`
- `seller_name`
- `buyer_name`
- `total_amount`
- `schema_version`

Document 文本建议由以下内容拼接：

- 发票类型
- 发票号码和代码
- 销售方和购买方
- 金额与日期
- 明细行摘要
- 备注

## 10. LLM 识别输出 Schema 草案

```json
{
  "is_invoice": true,
  "invoice_type": "string|null",
  "invoice_code": "string|null",
  "invoice_number": "string|null",
  "issue_date": "YYYY-MM-DD|null",
  "seller": {
    "name": "string|null",
    "tax_id": "string|null"
  },
  "buyer": {
    "name": "string|null",
    "tax_id": "string|null"
  },
  "currency": "CNY",
  "amount_without_tax": "number|null",
  "tax_amount": "number|null",
  "total_amount": "number|null",
  "category": "string|null",
  "items": [
    {
      "name": "string|null",
      "spec": "string|null",
      "unit": "string|null",
      "quantity": "number|null",
      "unit_price": "number|null",
      "amount": "number|null",
      "tax_rate": "number|null",
      "tax_amount": "number|null"
    }
  ],
  "remarks": "string|null",
  "confidence": 0.0,
  "needs_review": true,
  "warnings": ["string"]
}
```

## 11. 重复检测评分建议

建议将重复检测分为硬规则和软规则：

硬规则：

- 文件 SHA256 一致：100 分，`exact_duplicate`。
- 发票代码、发票号码、开票日期、价税合计全部一致：95 分，`probable_duplicate`。

软规则：

- 销售方一致：加 10 分。
- 购买方一致：加 10 分。
- 日期相差小于 1 天：加 10 分。
- 金额差异小于 0.01：加 20 分。
- 明细文本 embedding 相似度大于 0.92：加 20 分。
- 页面感知 hash 高度相似：加 20 分。

阈值：

- `score >= 95`：高度重复。
- `80 <= score < 95`：疑似重复。
- `60 <= score < 80`：可能重复。

## 12. Agent 工具边界

Agent 只能调用应用内部注册工具。工具定义必须包含：

- 名称。
- 参数 JSON schema。
- 是否只读。
- 是否需要确认。
- 最大影响范围。
- 执行超时。
- 错误格式。

高风险工具：

- 批量更新发票字段。
- 删除记录。
- 覆盖导出文件。
- 移动或删除 RAW 文件。

这些工具必须通过 UI 确认面板，不允许模型自行确认。

## 13. 版本规划

### MVP

- Tauri 基础应用。
- SQLite schema 和迁移。
- 手动导入 PDF/PNG/JPG。
- RAW 文件存储。
- PDF 按页处理。
- OpenAI-compatible LLM 配置。
- 发票识别和结构化入库。
- 发票列表、详情和手动编辑。
- 文件 hash 和字段级去重。
- 基础导出 CSV/Excel。

### V1

- 目录监听。
- ChromaDB 向量索引。
- 语义重复检测。
- Dashboard。
- Agent 聊天窗口基础工具。
- Agent 流式输出（SSE streaming）和任务取消。
- SCNet 高精度 OCR 交叉验证。
- MCP Server（工具注册、资源管理）。
- 批量 PDF 导出。
- 编辑历史和审计日志。

### V2

- 多页发票合并和单页多发票切分。
- 更细粒度统计图表。
- 本地 OCR 或本地 embedding 可选支持。
- 存储清理和归档策略。
- Excel 模板导出引擎（AST 模板解析、条件区域、循环区域、汇总公式）。
- 编辑历史 UI。
- 资源规模测试和性能优化。

## 14. 开放问题

1. 首个目标发票类型是否以中国大陆增值税电子发票为主？
2. 是否需要支持英文或多语言票据？
3. 是否允许 LLM 请求携带完整发票图片到第三方服务？
4. 是否需要密码保护本地数据库或 RAW 文件？
5. ChromaDB 是否必须内置随应用启动，还是允许用户配置外部 ChromaDB 服务？
6. Excel 导出的默认模板是否需要兼容某个财务软件？
