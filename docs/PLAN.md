# InvoiceVault 开发计划

## 1. 开发原则

- 本地优先：所有 RAW 文件、SQLite 数据、索引和日志默认保存在用户本机。
- 模块化：导入、识别、存储、去重、向量、Agent、导出互相解耦。
- 可替换：LLM Provider、VectorStore、Exporter 通过接口封装。
- 可恢复：后台任务有状态，异常退出后可恢复或重试。
- 小步交付：先完成可用闭环，再扩展 Agent 和高级统计。

## 2. 推荐目录结构

```text
InvoiceVault/
  docs/
    PRD.md
    PLAN.md
  src/
    main.tsx
    app/
    components/
    features/
      import/
      invoices/
      dashboard/
      agent/
      settings/
    lib/
  src-tauri/
    Cargo.toml
    tauri.conf.json
    src/
      lib.rs                 # crate root, run(), setup_tray(), single-instance
      main.rs                # binary entry point
      bin/mcp_server.rs      # standalone MCP server binary
      commands/              # Tauri command handlers (106 commands, 13 domain files)
      │  mod.rs, window.rs, invoice.rs, export.rs, import.rs,
      │  watcher.rs, email.rs, agent.rs, event.rs, config.rs,
      │  recognize.rs, semantic.rs, util.rs
      app_core/              # AppState, config, paths, archive, constants
      agent/                 # Agent tool registration, task/artifact management
      chroma/                # ChromaDB HTTP client
      dedupe/                # duplicate detection engine
      diag.rs                # end-to-end diagnostic tooling
      document/              # PDF rendering, image normalization
      email_manager.rs       # IMAP/POP3 email integration
      embedding/             # local ONNX embedding (BGE model)
      event.rs               # event/notification recording
      exporter/              # CSV/Excel/PDF export
      extractor/             # LLM invoice extraction, CRUD, dashboard, usage
      importer/              # import job queue, file hash dedup
      llm/                   # OpenAI-compatible LLM client
      mcp/                   # MCP JSON-RPC server
      process_utils.rs       # process utilities
      raw_store/             # raw file storage, thumbnails
      scnet_ocr.rs           # SCNet OCR integration
      storage/               # SQLite migrations
      template_engine/       # Excel template export engine (10 files)
      watcher/               # directory watcher with debounce
    migrations/
  tests/
  TODO.md
```

## 3. 里程碑

### M0：项目脚手架与基础设施

目标：建立可运行的 Tauri 应用和工程规范。

状态：已完成基础闭环。当前已具备 Tauri 2 + React + TypeScript 项目结构、Rust 后端模块、应用数据目录创建、SQLite 连接和迁移机制、前后端 `app_health` command 通信。

任务：

- 初始化 Tauri + 前端项目。
- 配置 Rust workspace 或模块结构。
- 配置格式化、lint、测试命令。
- 建立应用数据目录获取逻辑。
- 建立统一错误类型和日志。
- 建立 SQLite 连接和迁移机制。

交付：

- 应用可启动。
- 前后端能通过 Tauri command 通信。
- SQLite 数据库可创建和迁移。

### M1：本地存储与导入闭环

目标：用户可以导入文件，系统保存 RAW 并生成导入任务。

状态：已完成。已实现 RAW 原始格式固定目录存储、PDF/PNG/JPG/JPEG 文件选择、拖拽和路径导入、SHA256/MD5 去重、`import_jobs` 写入和导入队列展示。

任务：

- 实现 `raw_store` 原始格式固定目录存储。
- 实现文件类型、大小、hash 校验。
- 实现 `import_jobs` 表和任务状态机。
- 实现 GUI 手动选择和拖拽导入。
- 实现导入队列 UI。
- 实现重复文件 hash 检测。

交付：

- PDF/PNG/JPG/JPEG 可导入。
- RAW 文件与导入记录可追踪。
- 同一文件重复导入可提示。

### M2：文档处理与 LLM 识别

目标：将导入文件转换为可识别页面，并通过 LLM 生成结构化发票。

状态：完成基础闭环。已接入 OpenAI-compatible Chat Completions 连接测试和前端 Provider 配置表单；已完成图片 RAW 多模态识别、PDF 按页渲染识别、识别结果 JSON 校验、图片标准化、缩略图和结构化入库基础。

任务：

- 实现 PDF 按页渲染或页面图像提取。
- 实现图片标准化和压缩。
- 实现 LLM Provider 配置 UI。
- 实现 OpenAI-compatible client。
- 实现连接测试和视觉模型能力测试。
- 完善 PDF 按页发票识别 prompt 和多模态 LLM 调用编排。
- 完善 `invoices`、`invoice_items`、`extraction_runs` 写入与前端流程集成。

交付：

- 单页图片发票可识别入库。
- 多页 PDF 可按页生成候选发票。
- 识别失败可查看原因并重试。

### M3：发票列表、详情和编辑

目标：用户可以查看、检索和修正识别结果。

状态：已完成基础闭环。当前已支持分页、筛选、排序、详情预览、字段编辑、明细行增删改、字段级校验和自定义 Badge 标签选择；编辑历史 UI 仍待补齐。

任务：

- 实现发票列表分页查询。
- 实现筛选和排序。
- 实现详情页原件预览。
- 实现字段编辑和明细行编辑。
- 实现保存校验。
- 实现编辑历史和审计日志基础能力。

交付：

- 用户可从列表进入详情并修改字段。
- 修改后列表和详情同步刷新。
- 错误字段有明确提示。

### M4：去重系统

目标：系统能发现重复导入或疑似重复发票。

状态：已完成基础闭环。当前已支持文件 hash 去重、字段级重复候选、重复候选管理和语义相似度参与去重。

任务：

- 实现文件 hash 去重。
- 实现字段级重复匹配。
- 实现重复候选表和状态管理。
- 实现详情页重复候选提示。
- 实现确认重复、忽略、合并基础流程。
- 抽象 `VectorStore` trait，为语义去重预留接口。

交付：

- 完全相同文件自动拦截。
- 发票代码、号码、日期、金额一致时提示疑似重复。
- 用户可以处理重复候选。

### M5：目录监听

目标：支持后台监听目录并自动导入新文件。

状态：已完成。当前已支持监听目录配置、后台 watcher、防抖、文件稳定等待、启停和删除。

任务：

- 实现 `watch_dirs` 表。
- 实现目录监听服务。
- 实现文件稳定等待和事件去抖动。
- 实现监听目录配置 UI。
- 实现监听服务状态展示和错误恢复。

交付：

- 新文件放入监听目录后自动导入。
- 用户可暂停、恢复和删除监听目录。

### M6：向量索引与语义能力

目标：引入 ChromaDB 支持语义搜索和语义去重。

状态：已完成基础闭环。当前已支持 ChromaDB HTTP sidecar 配置、embedding 配置、发票文本 embedding 写入/更新、语义搜索和语义去重；ChromaDB 不可用时核心导入、识别和编辑功能仍可运行。

任务：

- 实现 ChromaDB 启动/连接策略。
- 实现 `VectorStore` 的 ChromaDB adapter。
- 实现 embedding 文本构建。
- 实现发票入库、编辑后向量同步。
- 实现语义搜索。
- 实现 embedding 相似度参与去重评分。

交付：

- 用户可进行语义搜索。
- 疑似重复判断包含语义相似度。
- ChromaDB 不可用时应用核心功能仍可运行。

### M7：Dashboard 和导出

目标：提供统计视图和常用导出能力。

状态：基础闭环完成。Dashboard、CSV 导出和 Excel 导出已实现；模板引擎（`template_engine/`）已完成 parser、region、binder、cloner、writer 等核心模块，支持按 Excel 模板批量导出。导出失败清单和更完整的导出任务日志仍待补齐。

任务：

- 实现 Dashboard 指标查询。
- 实现金额趋势、类型分布、销售方排行。
- 实现 CSV 导出。
- 实现 Excel 导出。
- 实现按模板批量导出 PDF/图片。
- 实现导出任务日志。

交付：

- 用户可按月份或筛选结果导出 Excel。
- Dashboard 能展示核心统计。
- 批量导出失败有错误清单。

### M8：Agent 聊天窗口

目标：通过自然语言执行查询、统计和导出任务。

状态：已完成基础闭环。当前已支持 Agent 会话和消息表、工具调用 schema、查询、详情、Dashboard 统计、CSV/Excel 导出、字段更新、确认面板和工具审计日志；流式输出（`send_agent_message_stream`、`confirm_agent_action_stream`）已实现。MCP Server 已作为独立二进制（`bin/mcp_server.rs`）完整实现，通过 stdio JSON-RPC 暴露 11 个工具。任务取消、图表生成、批量 PDF/图片导出和更丰富的工具集仍待补齐。

任务：

- 实现 Agent 会话和消息表。
- 定义工具调用 schema。
- 实现只读工具：查询、详情、统计。
- 实现导出工具：CSV/Excel。
- 实现批量 PDF/图片导出工具。
- 实现确认面板。
- 实现工具执行日志。
- 实现 Agent 任务取消和错误处理。

交付：

- 用户可通过对话查询发票。
- 用户可通过对话导出 Excel 或生成图表数据。
- 高风险操作必须经过确认。

### M9：资源控制与打包发布

目标：满足个人笔记本运行约束并完成跨平台打包。

状态：进行中。当前已具备 Linux/KDE 托盘、应用图标、关闭隐藏到托盘、日志导出和基础存储清理；规模测试、内存测试、缓存预算和 Windows/Linux 安装包验收仍待完成。

任务：

- 做 100、1000、10000 发票记录规模测试。
- 做大 PDF 和批量导入内存测试。
- 限制 LLM 并发和图像尺寸。
- 实现缓存清理和存储占用展示。
- 完成 Windows 和 Linux 打包。
- 编写用户使用文档和故障排查文档。

交付：

- 内存峰值目标小于 2GB。
- 默认缓存和索引策略尽量控制在 500MB 内。
- Windows/Linux 安装包可运行。

## 4. 技术实施建议

### 4.1 Tauri 与前端

- 前端建议使用 TypeScript，避免复杂业务逻辑散落在 UI 组件中。
- Tauri command 只暴露稳定、有限的后端接口。
- 长任务使用事件推送更新进度，不通过单次 command 阻塞等待。
- 发票详情页预览不要一次性加载所有页面原图，使用缩略图和按需加载。

### 4.2 SQLite

- 使用迁移工具管理 schema。
- 金额使用整数分或 decimal 字符串，避免浮点误差。
- 常用筛选字段建立索引：`issue_date`、`seller_name`、`buyer_name`、`invoice_number`、`total_amount`、`status`、`duplicate_status`。
- 导入和识别写入使用事务。

### 4.3 ChromaDB

推荐先实现接口：

```text
trait VectorStore {
  upsert_invoice_embedding(invoice_id, document, metadata)
  search_similar(document_or_embedding, limit, filter)
  delete_invoice(invoice_id)
  health_check()
}
```

实现策略：

- MVP 阶段允许禁用向量能力。
- V1 阶段支持连接本地或外部 ChromaDB HTTP 服务。
- 如果桌面打包体积和资源占用不可接受，替换为 SQLite 向量扩展。

### 4.4 LLM

- 将识别 prompt、Agent prompt、JSON schema 版本化。
- 每个模型配置都要记录能力：视觉输入、工具调用、JSON 输出。
- 识别请求要有超时、重试和失败记录。
- 图片输入前进行压缩，避免 token/请求成本失控。
- 对模型输出进行严格 schema 校验，不可信任模型直接写库。

### 4.5 PDF 和图片处理

- PDF 首选按页渲染为图片传给视觉模型。
- 如果 PDF 内含可选中文本，可先提取文本，和页面图片一起提供给模型。
- 大 PDF 按页流式处理。
- 缩略图单独缓存，可随时重建。

### 4.6 Agent

- Agent 不接触底层数据库连接，只调用工具层。
- 工具层做权限、参数校验和审计。
- 导出、批量更新、删除等操作必须确认。
- 每次 Agent 执行都保存计划、工具调用和结果。

## 5. 测试计划

### 5.1 单元测试

- 文件 hash 与 RAW 存储路径。
- 数据库 repository。
- 金额和日期解析。
- JSON schema 校验。
- 重复检测评分。
- 导出文件名模板。

### 5.2 集成测试

- 手动导入一张图片发票。
- 导入多页 PDF。
- 重复文件导入。
- 识别失败重试。
- 编辑发票后更新统计和向量。
- Agent 查询和导出。

### 5.3 性能测试

- 100 个文件批量导入。
- 1000 条记录列表分页和筛选。
- 10000 条记录 Dashboard 聚合。
- 100 页 PDF 的内存峰值。
- ChromaDB 启动、查询和同步耗时。

### 5.4 人工验收测试

- Windows 安装和运行。
- Linux 安装和运行。
- LLM 配置错误提示。
- 断网场景。
- 应用异常退出后的任务恢复。

## 6. 风险与缓解

| 风险 | 影响 | 缓解 |
| --- | --- | --- |
| ChromaDB 打包复杂 | 安装包大，运行时依赖复杂 | 抽象 VectorStore，允许禁用或外部服务 |
| LLM 输出不稳定 | 字段错误，入库失败 | JSON schema 校验、置信度、人工审核 |
| RAW 文件超出 500MB | 存储预算失控 | hash 去重、压缩缩略图、占用提示、清理策略 |
| 多页 PDF 语义切分困难 | 一张发票拆成多条或多张发票合成一条 | MVP 按页处理，提供手动合并/拆分 |
| watcher 误触发 | 导入半写入文件 | 稳定等待、文件锁检测、重试 |
| OpenAI-compatible 差异 | 不同供应商接口能力不一致 | Provider 能力探测和降级 |

## 7. 优先级建议

先做：

- 导入。
- RAW 存储。
- SQLite 入库。
- LLM 识别。
- 列表和编辑。
- 基础去重。

后做：

- ChromaDB。
- Agent。
- Dashboard。
- 高级导出。

原因：

- 没有稳定入库闭环，Agent 和 Dashboard 都缺少可靠数据基础。
- ChromaDB 和语义去重能提升体验，但不是最小可用产品的阻塞项。
- 手动编辑是发票识别产品的刚需，应早于复杂统计。

## 8. Definition of Done

每个功能完成时至少满足：

- 有可运行 UI 或可调用后端接口。
- 有错误处理和日志。
- 有基础测试或人工验收记录。
- 不阻塞 UI 主线程。
- 数据写入可恢复或可重试。
- 文档中更新使用方式和限制。
