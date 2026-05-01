# Receiptier 开发说明

## 当前状态

M0 脚手架已建立：

- 前端使用 Vite + React + TypeScript。
- 桌面端使用 Tauri 2 + Rust。
- 后端启动时创建应用数据目录、RAW 目录、缩略图目录和 SQLite 数据库。
- SQLite 使用内置迁移，当前迁移版本为 `1`。
- 前端通过 Tauri command `app_health` 读取基础设施状态。
- 后端支持 PDF/PNG/JPG/JPEG 路径导入，导入时会计算 SHA256/MD5、按 `raw/YYYY/MM/文件名` 存储原始 PDF/图片，并记录导入任务。
- 前端导入队列已接入 `import_files` 和 `list_import_jobs`，支持原生文件选择器、拖拽和路径输入。
- 前端支持 OpenAI-compatible LLM Provider 连接测试；API Key 只在本次界面输入中使用，不写入仓库。

## 常用命令

```bash
npm install
npm run build
cd src-tauri && cargo fmt --check
cd src-tauri && cargo test
cd src-tauri && cargo check
npm run tauri build -- --no-bundle
```

LLM 真实连接测试默认被忽略，需要本机临时环境变量：

```bash
cd src-tauri
RECEIPTIER_LLM_BASE_URL=... RECEIPTIER_LLM_MODEL=... RECEIPTIER_LLM_API_KEY=... \
  cargo test live_llm_connection_from_env -- --ignored
```

开发运行：

```bash
npm run tauri dev
```

## 本地配置

不要把 LLM API Key 写入仓库。后续接入 OpenAI-compatible Provider 时，开发测试配置应通过本地环境变量或应用设置写入本机配置目录。
