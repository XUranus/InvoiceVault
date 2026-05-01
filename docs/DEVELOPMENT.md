# Receiptier 开发说明

## 当前状态

M0 脚手架已建立：

- 前端使用 Vite + React + TypeScript。
- 桌面端使用 Tauri 2 + Rust。
- 后端启动时创建应用数据目录、RAW 目录、缩略图目录和 SQLite 数据库。
- SQLite 使用内置迁移，当前迁移版本为 `1`。
- 前端通过 Tauri command `app_health` 读取基础设施状态。

## 常用命令

```bash
npm install
npm run build
cd src-tauri && cargo fmt --check
cd src-tauri && cargo test
cd src-tauri && cargo check
```

开发运行：

```bash
npm run tauri dev
```

## 本地配置

不要把 LLM API Key 写入仓库。后续接入 OpenAI-compatible Provider 时，开发测试配置应通过本地环境变量或应用设置写入本机配置目录。

