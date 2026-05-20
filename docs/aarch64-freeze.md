# aarch64 Linux 卡死问题

## 现象

在 aarch64 Linux（Ubuntu 22.04, WebKitGTK 2.50.4, Rockchip Mali GPU）上，应用启动后 3-60 秒内窗口完全冻结：
- 右键菜单无响应
- JS `setInterval` 停止触发
- GTK 事件循环阻塞（gdb 显示主线程在 `__GI___poll`）
- 频繁切换 tab 更容易触发

## 根因

WebKitGTK 使用独立的 **compositor 线程**（`WebPageCompositor`）做 GPU 加速渲染合成。在 aarch64 + Mesa/Rockchip Mali 驱动上，compositor 线程与 GPU 驱动存在 **AB-BA 死锁**：

1. Compositor 线程向 GPU 提交渲染命令
2. Mali GPU 驱动（panfrost）阻塞等待主线程
3. 主线程等待 compositor 线程完成
4. → 死锁，整个 webview 冻结

CSS transitions、transforms、tab 切换等操作会触发大量 GPU 合成，增加死锁概率。

## 修复

在 Rust 启动入口无条件设置环境变量：

```rust
#[cfg(target_os = "linux")]
{
    std::env::set_var("WEBKIT_DISABLE_COMPOSITING_MODE", "1");
}
```

此变量在 WebKitGTK 初始化前设置，强制所有渲染在主线程同步执行，跳过 compositor 线程，避免死锁。

对于 InvoiceVault 这类表单/表格型业务应用，禁用 GPU 合成对性能无可见影响。

## 相关优化

- **WatcherManager 延迟初始化**：`resume_enabled()` 从 `new()` 中移到后台线程，避免阻塞 setup 流程
- **预览缩略图生成移至后台**：`regenerate_missing_previews()` 不再在 `AppState::initialize` 中同步执行
- **Embedding 引擎懒加载**：ONNX Runtime 不在启动时加载，避免 `dlopen` 与 WebKitGTK 冲突
- **前端 heartbeat**：每秒调用 `frontend_heartbeat` IPC，可在日志中确认前端 JS 事件循环是否正常运行

## 参考

- WebKit Bug: https://bugs.webkit.org/show_bug.cgi?id=263930
- 环境变量文档：`WEBKIT_DISABLE_COMPOSITING_MODE` (WebKitGTK 内部)
