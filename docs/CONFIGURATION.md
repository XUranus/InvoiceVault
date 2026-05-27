# InvoiceVault 配置参考

## 运行时配置文件

所有配置存储在应用数据目录下（各平台路径不同）。

| 文件 | 结构体 | 说明 |
|------|--------|------|
| `llm_config.json` | `LlmProviderConfig` | LLM API 配置 |
| `chroma_config.json` | `ChromaConfig` | ChromaDB 向量数据库配置 |
| `price_config.json` | `PriceConfig` | LLM 调用价格配置 |
| `badge_config.json` | `BadgeConfig` | 发票标签/分类配置 |
| `diagnostic_config.json` | `DiagnosticConfig` | 诊断工具配置 |
| `embedding_enabled.json` | - | embedding 功能开关 |
| `theme.json` | - | 主题设置 (light/dark) |
| `window_state.json` | - | 窗口尺寸记忆 |

## 常量配置 (app_core/constants.rs)

### 目录名
| 常量 | 值 | 说明 |
|------|----|----|
| `DIR_LOGS` | `"logs"` | 日志目录 |
| `DIR_MODELS` | `"models"` | 模型目录 |
| `DIAGNOSTIC_CONFIG_FILE` | `"diagnostic_config.json"` | 诊断配置文件名 |

### Embedding 模型
| 常量 | 值 | 说明 |
|------|----|----|
| `EMBEDDING_MODEL_REPO` | `"Xenova/bge-small-zh-v1.5"` | HuggingFace 模型仓库 |
| `EMBEDDING_MODEL_DIR` | `"bge-small-zh-v1.5"` | 本地模型目录名 |
| `EMBEDDING_DIMENSIONS` | `384` | 向量维度 |
| `EMBEDDING_MAX_TOKENS` | `512` | 最大 token 长度 |

### 超时 (秒)
| 常量 | 值 | 说明 |
|------|----|----|
| `LLM_DEFAULT_TIMEOUT_SECS` | `30` | LLM 默认请求超时 |
| `LLM_RECOGNITION_TIMEOUT_SECS` | `90` | 发票识别超时 |
| `LLM_CONNECT_TEST_TIMEOUT_SECS` | `30` | LLM 连接测试超时 |
| `AGENT_DEFAULT_TIMEOUT_SECS` | `60` | Agent 请求超时 |
| `SCNET_OCR_TIMEOUT_SECS` | `30` | SCNet OCR 超时 |
| `EMBEDDING_DOWNLOAD_TIMEOUT_SECS` | `120` | 模型下载超时 |
| `EMBEDDING_TEST_TIMEOUT_SECS` | `120` | Embedding 测试超时 |

### LLM 推理参数
| 常量 | 值 | 说明 |
|------|----|----|
| `LLM_MAX_RETRIES` | `3` | 最大重试次数 |
| `LLM_VLM_MAX_ATTEMPTS` | `3` | VLM 识别尝试次数 |
| `LLM_VLM_CONFIDENCE_THRESHOLD` | `0.5` | 置信度阈值 |
| `LLM_VLM_TEMPERATURES` | `[0.0, 0.3, 0.5]` | VLM 温度调度 |
| `LLM_RECOGNITION_MAX_TOKENS` | `4096` | 识别响应最大 token |
| `AGENT_MAX_TOKENS` | `2000` | Agent 响应最大 token |

### Agent
| 常量 | 值 | 说明 |
|------|----|----|
| `AGENT_MAX_ITERATIONS` | `20` | 工具调用循环上限 |
| `AGENT_HISTORY_LIMIT` | `20` | 上下文消息数 |
| `AGENT_DEFAULT_TITLE` | `"新对话"` | 默认会话标题 |

### 文件监听
| 常量 | 值 | 说明 |
|------|----|----|
| `WATCHER_DEFAULT_STABLE_WAIT_MS` | `2000` | 文件稳定等待时间(ms) |
| `WATCHER_STABILITY_CHECK_INTERVAL_MS` | `100` | 稳定检查间隔(ms) |
| `ALLOWED_EXTENSIONS` | `["pdf","png","jpg","jpeg"]` | 允许的文件扩展名 |

## 环境变量

| 变量 | 说明 |
|------|------|
| `ORT_DYLIB_PATH` | ONNX Runtime 动态库路径（自动设置） |
| `INVOICEVAULT_WIN_DEPS_DIR` | Windows 原生依赖目录（自动设置） |
| `WEBKIT_DISABLE_COMPOSITING_MODE` | Linux WebKitGTK 兼容（自动设置） |
| `RUST_LOG` | 日志级别（默认 `invoicevault=info`） |
