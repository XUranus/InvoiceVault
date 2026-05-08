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

## 使用方式

1. 启动应用。
2. 在导入队列中选择或拖入 PDF/PNG/JPG/JPEG 文件。
3. 在设置页填写 LLM Provider 的 Base URL、Model 和 API Key。
4. 点击"测试连接"确认配置可用。
5. 对已导入的图片或 PDF 文件点击"识别"。
6. 识别成功后，结构化发票会出现在发票库列表。
7. 在发票详情页修正字段、维护明细行、处理重复候选或选择自定义 Badge。
8. 在 Agent 页面通过自然语言查询、统计或导出发票。

当前图片识别支持 `image/png` 和 `image/jpeg`。PDF 文件会先通过 `pdftoppm` 渲染为 JPEG 页面缓存，再逐页调用多模态识别。所有图片和 PDF 页面在发送给 LLM 前会通过 `magick` 生成标准化 JPEG，同时生成预览缩略图。RAW 归档文件不会被修改。

## 数据存储

应用数据保存在系统分配的应用数据目录中，后端启动时会创建：

- `invoicevault.sqlite3`：SQLite 数据库。
- `raw/`：原始 PDF/图片归档目录。
- `thumbnails/`：标准化识别图、PDF 页面缓存和预览缩略图目录。
- `logs/`：应用运行日志。
- `llm_config.json`、`embedding_config.json`、`recognition_config.json`、`badge_config.json`：本机应用配置文件。

RAW 文件归档策略：

```text
raw/YYYY/MM/current_name.ext
```

同名文件进入同一月份目录时，会自动追加 `-1`、`-2` 等后缀避免覆盖。数据库中同时保存 `original_name`（导入时原始文件名）、`current_name`（归档后的当前文件名）、`storage_path`（归档后的实际路径）、`sha256` 和 `md5`（用于文件级去重）。

## LLM 说明

LLM 配置保存在本机应用数据目录，不会写入仓库。不要把 API Key 提交到代码或文档。

图片识别和 Agent 对话均使用 OpenAI-compatible `/chat/completions`：

```text
POST {base_url}/chat/completions
```

模型需要支持多模态图片输入，并能返回 JSON。后端会从模型响应中提取 JSON 对象，校验后写入 SQLite。Agent 对话通过受控工具执行查询、统计、导出和字段更新，写操作会经过 UI 确认。

## 端到端诊断测试

设置页面提供「端到端诊断」按钮，用于验证 LLM 全链路是否可用（文本生成 → 图片识别 → 结果对比 → Embedding）。

诊断配置存储在 `{app_data_dir}/diagnostic_config.json`，首次运行自动创建默认配置。开发者需要手动编辑此文件来设置测试发票和 ground truth。

配置文件格式：

```json
{
  "test_image_path": "/absolute/path/to/test-invoice.png",
  "ground_truth": {
    "invoice_type": "增值税电子普通发票",
    "invoice_code": "033001900211",
    "invoice_number": "68087646",
    "issue_date": "2019-12-24",
    "seller_name": "杭州热联电子商务有限公司",
    "buyer_name": "杭州热联集团中邦实业有限公司",
    "total_amount": 2740.00,
    "amount_without_tax": 2358.92,
    "tax_amount": 381.08,
    "items_count": 7
  },
  "enabled": true
}
```

字段说明：

- `test_image_path`：测试发票图片的绝对路径，支持 PNG/JPEG。仓库 `sample/` 目录下提供了一张测试发票 `fake-invoice-1.png`。
- `ground_truth`：预期识别结果。所有字段均可选，填写的字段才会参与对比评分。
  - 字符串字段（`invoice_type`、`seller_name` 等）：包含匹配。
  - 金额字段（`total_amount` 等）：允许 ±5% 误差。
  - `items_count`：精确匹配。
- `enabled`：是否启用诊断。

修改 ground truth 时，先用「端到端诊断」运行一次，查看识别原始结果，再将正确值填入 `diagnostic_config.json`。评分 = 匹配字段数 / 填写的总字段数 × 100。
