# InvoiceVault 开发说明

## 当前状态

当前已完成 M8 基础闭环，正在进入批量导出、资源控制和打包验收阶段：

- 前端使用 Vite + React + TypeScript，侧边栏导航多页面布局。
- 桌面端使用 Tauri 2 + Rust，后端按导入、识别、存储、去重、导出、Agent、事件通知、ChromaDB 和 embedding 等模块拆分。
- 后端启动时创建应用数据目录、RAW 目录、缩略图目录和 SQLite 数据库。
- SQLite 使用内置迁移，当前迁移版本为 `9`。
- 前端通过 Tauri command `app_health` 读取基础设施状态。
- 后端支持 PDF/PNG/JPG/JPEG 路径导入，导入时会计算 SHA256/MD5、按 `raw/YYYY/MM/文件名` 保留原始格式归档 PDF/图片，并记录导入任务。
- 前端导入页面支持原生文件选择器、拖拽和路径输入，展示导入历史和任务详情。
- 前端支持 OpenAI-compatible LLM Provider 连接测试；配置会写入本机应用数据目录，不写入仓库。
- 后端已建立发票识别结果 JSON 校验与结构化入库基础，支持把校验后的识别结果写入 `invoices`、`invoice_items` 和 `extraction_runs`。
- 前端导入队列已支持对图片 RAW 文件触发多模态识别，并展示已入库发票摘要。
- PDF RAW 文件识别会通过本机 `pdftoppm` 渲染为 JPEG 页面缓存，再逐页调用多模态识别。
- 图片和 PDF 页面在发送给 LLM 前会通过本机 `magick` 生成标准化 JPEG，同时生成预览缩略图；RAW 原文件不被修改。
- 发票库页面支持分页、关键词搜索、日期/金额/状态筛选、多字段排序。
- 发票详情页展示完整字段、明细行、缩略图预览，支持字段编辑和明细行 CRUD。
- 字段级重复检测自动运行：发票代码+号码完全匹配（95分）、多字段相似度打分。
- 重复候选管理支持确认和忽略操作。
- CSV 和 Excel 导出（通过 `rust_xlsxwriter` + `csv` crate）。
- 目录监听和自动导入：在设置页面配置监听目录，后台线程检测文件变化，防抖后自动触发导入。前端通过 `watcher-import` 事件实时感知。
- Dashboard 统计面板：发票总数、金额合计、月度趋势折线图、类型分布饼图、状态分布柱状图、Top5 供应商排名。后端聚合查询，前端使用 recharts 渲染。
- ChromaDB 向量索引：外部 ChromaDB HTTP sidecar，VectorStore 抽象接口。支持发票文本 embedding 写入、语义相似度去重（0.85/0.92 两档阈值）、自然语言语义搜索。Embedding 通过 OpenAI-compatible `/embeddings` 端点生成。
- Agent 聊天窗口：支持会话创建/删除、历史消息、OpenAI-compatible tool calling、查询发票、读取详情、读取 Dashboard 统计、CSV/Excel 导出和字段更新。
- Agent 写操作确认：导出和字段更新需要 UI 确认；工具调用结果写入 `audit_logs`，导出操作也会记录事件。
- 事件和通知中心：导入、识别、导出、配置变化、清理等后台行为可记录为事件或通知。
- 设置页已覆盖 LLM、Embedding、ChromaDB、识别并发、监听目录、外部依赖检查、日志导出、存储清理和自定义 Badge 配置。
- 自定义 Badge：配置保存在 `badge_config.json`，发票详情页可按分组为单张发票选择一个标签值，选择结果写入 `invoice_badges`。
- Linux/KDE 桌面集成：应用使用 `icons/app.png` 作为打包图标，`icons/tray.png` 作为托盘图标；启动显示托盘，关闭主窗口时隐藏到托盘，托盘菜单提供“工作台”、版本信息和退出。

## 常用命令

```bash
npm install
npm run build
cd src-tauri && cargo fmt --check
cd src-tauri && cargo test
cd src-tauri && cargo check
npm run tauri build -- --no-bundle
```

PDF 识别依赖 Poppler `pdftoppm`：

```bash
pdftoppm -v
```

图片标准化和缩略图依赖 ImageMagick `magick`：

```bash
magick -version
```

LLM 真实连接测试默认被忽略，需要本机临时环境变量：

```bash
cd src-tauri
RECEIPTIER_LLM_BASE_URL=... RECEIPTIER_LLM_MODEL=... RECEIPTIER_LLM_API_KEY=... \
  cargo test live_llm_connection_from_env -- --ignored
```

样本图片真实识别测试：

```bash
cd src-tauri
RECEIPTIER_LLM_BASE_URL=... RECEIPTIER_LLM_MODEL=... RECEIPTIER_LLM_API_KEY=... \
  cargo test live_invoice_image_recognition_from_env -- --ignored
```

开发运行：

```bash
npm run tauri dev
```

## 本地配置

不要把 LLM API Key 写入仓库。当前 LLM、Embedding、识别并发和 Badge 配置会写入本机应用数据目录中的 JSON 配置文件；真实连接测试仍建议使用临时环境变量。

当前主要待补齐能力：

- 批量 PDF/图片按模板导出。
- Agent 流式输出、任务取消、更多工具和确认状态持久化。
- 编辑历史 UI 和审计日志查询 UI。
- 资源占用测试、缓存策略和跨平台打包验收。
