# Receiptier 开发说明

## 当前状态

M0 脚手架已建立：

- 前端使用 Vite + React + TypeScript。
- 桌面端使用 Tauri 2 + Rust。
- 后端启动时创建应用数据目录、RAW 目录、缩略图目录和 SQLite 数据库。
- SQLite 使用内置迁移，当前迁移版本为 `1`。
- 前端通过 Tauri command `app_health` 读取基础设施状态。
- 后端支持 PDF/PNG/JPG/JPEG 路径导入，导入时会计算 SHA256/MD5、写入 RAW 内容寻址存储，并记录导入任务。
- 前端导入队列已接入 `import_files` 和 `list_import_jobs`，当前先使用路径输入，后续补文件选择器和拖拽。

## 常用命令

```bash
npm install
npm run build
cd src-tauri && cargo fmt --check
cd src-tauri && cargo test
cd src-tauri && cargo check
npm run tauri build -- --no-bundle
```

开发运行：

```bash
npm run tauri dev
```

## 本地配置

不要把 LLM API Key 写入仓库。后续接入 OpenAI-compatible Provider 时，开发测试配置应通过本地环境变量或应用设置写入本机配置目录。
