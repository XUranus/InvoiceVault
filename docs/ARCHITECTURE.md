# InvoiceVault 系统架构

## 整体架构

InvoiceVault 是基于 Tauri 2.x 的桌面应用，前端 React + TypeScript，后端 Rust。

### 分层架构

```
┌─────────────────────────────────────────────┐
│                  前端 (React)                │
│  stores/ components/ hooks/ pages/          │
├─────────────────────────────────────────────┤
│            Tauri Command 层                 │
│  commands/ (13 个领域文件, 107 个命令)       │
├─────────────────────────────────────────────┤
│              核心业务层                      │
│  AppState → extractor, agent, llm,          │
│  importer, dedupe, watcher, email_manager   │
├─────────────────────────────────────────────┤
│              基础设施层                      │
│  storage(SQLite), chroma(向量DB),           │
│  embedding(ONNX), raw_store, document       │
└─────────────────────────────────────────────┘
```

## 核心模块

### AppState (app_core/)
应用全局状态，持有所有子系统的引用。所有 Tauri 命令最终委托给 AppState 方法。

### 数据流
1. **导入**: 文件 → importer(去重) → raw_store(存储) → document(渲染) → llm(识别) → extractor(保存)
2. **搜索**: 前端 → commands → extractor::search_invoices → SQLite
3. **语义搜索**: 前端 → commands → embedding(向量化) → chroma(相似度查询)
4. **导出**: 前端 → commands → exporter(CSV/Excel/PDF) 或 template_engine(模板导出)
5. **Agent**: 前端 → commands → agent(工具循环) → llm(推理) + 各子系统(工具执行)

### 并发模型
- 数据库: `Arc<Mutex<Connection>>`，使用 `unwrap_or_else(|e| e.into_inner())` 恢复中毒的 mutex
- 配置: 各配置独立 `Mutex` 保护
- 文件监听: 每个监听目录独立线程
- 邮件同步: 独立后台线程
- Agent: tokio async runtime

### 关键设计决策
1. **Mutex 中毒恢复**: 使用 `unwrap_or_else` 而非 `expect`，避免级联崩溃
2. **锁作用域最小化**: ONNX 推理前释放 db 锁，防止长时间持锁
3. **ONNX 懒加载**: embedding 引擎不在启动时加载，避免与 WebKitGTK 冲突
4. **单实例机制**: 基于 TCP 端口绑定 + 文件锁
