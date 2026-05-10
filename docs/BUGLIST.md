# Bug List & Known Issues

## 163.com IMAP SELECT 被拦截 (Unsafe Login)

**现象**: IMAP LOGIN 成功，但执行 `SELECT INBOX` 时报错：
```
No Response: SELECT Unsafe Login. Please contact kefu@188.com for help
```

**根因**: 163.com 使用 Coremail 服务器，要求 IMAP 客户端在 `SELECT`/`EXAMINE` 之前先发送 `ID` 命令声明客户端身份。未发送 `ID` 时，Coremail 会拒绝所有邮箱操作并返回 "Unsafe Login"。

**验证过程**:
1. `openssl s_client -connect imap.163.com:993` 直接测试 → LOGIN 成功，SELECT 被拦截
2. 用 `STATUS INBOX "(MESSAGES)"` 代替 `SELECT` → 成功，确认账号和网络无问题
3. 在 `SELECT` 前发送 `ID ("name" "InvoiceVault")` → SELECT 成功

**修复**: 在 `email_manager.rs` 中添加 `imap_send_id()` 辅助函数，在 LOGIN 成功后自动发送 `ID` 命令，然后才执行 `SELECT`。`test_connection` 和 `do_sync` 均已适配。

**影响范围**: 所有使用 Coremail 系统的邮箱（163.com、yeah.net 等）。

**相关文件**:
- `src-tauri/src/email_manager.rs` — `imap_send_id()`, `test_connection()`, `do_sync()`

---

## 国内邮箱 IMAP/POP3 需要授权码

**现象**: 使用登录密码连接 163/QQ 等邮箱时，认证失败：
```
NO LOGIN Login error or password error
```

**根因**: 国内邮箱（163、QQ、Yeah 等）的安全策略要求第三方客户端使用专门的「授权码」而非登录密码进行 IMAP/POP3 认证。

**修复**: 在 `email_manager.rs` 中，当 IMAP/POP3 登录失败时检测错误信息，自动附加提示："如果使用国内邮箱（163/QQ/Yeah等），请使用「授权码」而非登录密码"。

**相关文件**:
- `src-tauri/src/email_manager.rs` — `wrap_imap_login_error()`, `pop3_send_cmd()`
