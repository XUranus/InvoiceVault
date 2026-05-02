# Windows 适配 TODO

本文记录 Receiptier 后续在 Windows 上运行、适配和打包时需要注意的问题，以及建议推进计划。

## 目标

- 支持 Windows 10/11 桌面环境运行。
- 支持本地开发、调试和 release 构建。
- 支持生成 Windows 安装包，优先 NSIS，必要时补充 MSI。
- 保持和 Linux 版本一致的核心能力：导入、PDF 渲染、图片标准化、发票识别、归档、导出、托盘后台运行。

## 环境要求

Windows 开发机需要安装：

- Node.js 22+
- npm
- Rust stable MSVC toolchain
- Microsoft Visual Studio Build Tools
- Microsoft Edge WebView2 Runtime
- Poppler for Windows
- ImageMagick for Windows

Rust 建议使用 MSVC 工具链：

```bash
rustup default stable-x86_64-pc-windows-msvc
rustup target add x86_64-pc-windows-msvc
```

Visual Studio Build Tools 至少需要：

- Desktop development with C++
- MSVC v143 或更新版本
- Windows 10/11 SDK
- CMake tools 可选

## 需要重点验证的问题

### 1. 外部命令依赖

当前 PDF 和图片处理依赖外部命令：

- `pdftoppm`：来自 Poppler
- `magick`：来自 ImageMagick

Windows 适配时需要确认：

- 两个命令是否已加入 `PATH`。
- 路径包含空格时是否仍能正常调用。
- 中文路径、用户目录路径、长路径是否能正常处理。
- 命令缺失时，前端是否有清晰错误提示。

后端调用外部命令时应保持使用 `std::process::Command` 和逐个 `.arg(...)`，避免拼接 shell 字符串。

### 2. 文件路径兼容性

Windows 路径特点：

- 使用反斜杠：`C:\Users\...`
- 带盘符：`C:`
- 常见路径包含空格：`C:\Program Files\...`
- 用户目录可能包含中文或其他非 ASCII 字符。

需要检查：

- 后端是否全程使用 `Path` / `PathBuf` / `join()`。
- 数据库中保存的路径能否被前端正确展示。
- 点击打开文件、打开目录、导出文件时路径是否正常。
- RAW 归档目录结构在 Windows 下是否正常生成。
- 导入拖拽文件时 Tauri 返回的路径是否被正确处理。

### 3. 应用数据目录和权限

Windows 下应用数据目录通常位于：

- `%APPDATA%`
- `%LOCALAPPDATA%`

需要确认：

- SQLite 数据库不会写入安装目录。
- RAW 归档、缩略图、日志都写入 Tauri 分配的 app data 目录。
- 普通用户权限下可以创建目录、写入文件、导出文件。
- 杀毒软件或系统权限导致写入失败时，错误信息可诊断。

### 4. 图标和安装包资源

当前图标主要是 PNG。Windows 打包建议补充 `.ico`：

- `icons/icon.ico`
- 包含多尺寸：16、24、32、48、64、128、256。

需要确认：

- 任务栏图标清晰。
- Alt-Tab 图标清晰。
- 安装器图标清晰。
- 桌面快捷方式图标清晰。

### 5. 托盘行为

当前交互：

- 关闭窗口隐藏到托盘。
- 双击托盘恢复窗口。
- 托盘菜单提供“工作台”和“退出”。

Windows 下需要实际验证：

- 托盘图标是否显示清晰。
- 右键菜单是否正常显示。
- 点击“退出”是否真正结束进程。
- 关闭窗口后是否仍在托盘后台运行。
- 多显示器、高 DPI 缩放下菜单位置和图标是否正常。

### 6. WebView2

Tauri Windows 依赖 WebView2。

需要确认：

- 目标系统是否已安装 WebView2 Runtime。
- 安装包是否需要引导或提示安装 WebView2。
- 离线环境下安装体验是否可接受。

### 7. 打包格式

优先支持 NSIS：

```bash
npm run tauri -- build --bundles nsis
```

可选支持 MSI：

```bash
npm run tauri -- build --bundles msi
```

需要确认：

- 产物路径是否符合预期。
- 安装、升级、卸载流程是否正常。
- 安装后快捷方式、开始菜单项是否正常。
- 卸载后用户数据是否保留或按预期处理。

### 8. GitHub Actions Windows 构建

建议新增 Windows workflow：

- 使用 `windows-latest`。
- 安装 Node.js 22。
- 安装 Rust stable。
- 使用 Rust cache。
- 执行 `npm ci`。
- 执行 `npm run tauri -- build --bundles nsis`。
- 上传 NSIS `.exe` artifact。
- tag 发布时上传到 GitHub Release。

产物路径通常为：

```text
src-tauri/target/release/bundle/nsis/*.exe
src-tauri/target/release/bundle/msi/*.msi
```

## 适配计划

### Phase 1：本地 Windows 可运行

- [ ] 准备 Windows 开发环境。
- [ ] 安装 Visual Studio Build Tools。
- [ ] 安装 Rust MSVC toolchain。
- [ ] 安装 Poppler for Windows，并确认 `pdftoppm` 在 `PATH` 中可用。
- [ ] 安装 ImageMagick for Windows，并确认 `magick` 在 `PATH` 中可用。
- [ ] 在 Windows 上执行 `npm ci`。
- [ ] 在 Windows 上执行 `npm run build`。
- [ ] 在 Windows 上执行 `cd src-tauri && cargo check`。
- [ ] 在 Windows 上执行 `npm run tauri dev`。

### Phase 2：核心功能验证

- [ ] 导入 PNG/JPG/JPEG 文件。
- [ ] 导入 PDF 文件。
- [ ] 验证 PDF 渲染调用 `pdftoppm`。
- [ ] 验证图片标准化和缩略图调用 `magick`。
- [ ] 验证中文路径、空格路径、长文件名。
- [ ] 验证 RAW 归档目录结构。
- [ ] 验证 SQLite 数据写入。
- [ ] 验证 CSV / Excel 导出。
- [ ] 验证打开文件、打开目录、路径展示。
- [ ] 验证 LLM Provider 配置保存和读取。

### Phase 3：Windows 图标和托盘

- [ ] 从现有 PNG 生成 `icons/icon.ico`。
- [ ] 更新 Tauri bundle icon 配置，确保 Windows 使用 `.ico`。
- [ ] 验证任务栏图标。
- [ ] 验证安装器图标。
- [ ] 验证桌面快捷方式图标。
- [ ] 验证托盘图标。
- [ ] 验证关闭窗口隐藏到托盘。
- [ ] 验证双击托盘恢复窗口。
- [ ] 验证托盘“退出”菜单。

### Phase 4：Windows 安装包

- [ ] 本地执行 `npm run tauri -- build --bundles nsis`。
- [ ] 安装 NSIS 产物。
- [ ] 验证首次启动。
- [ ] 验证升级安装。
- [ ] 验证卸载。
- [ ] 检查开始菜单和桌面快捷方式。
- [ ] 如有必要，补充 MSI 构建。

### Phase 5：CI/CD

- [ ] 新增 Windows GitHub Actions workflow。
- [ ] 配置 Rust cache。
- [ ] 上传 NSIS artifact。
- [ ] tag 发布时上传 Windows 安装包到 GitHub Release。
- [ ] 记录构建耗时和失败点。
- [ ] 根据 CI 结果补充依赖或配置。

## 推荐后续改动

- [ ] 增加启动时依赖检测：检查 `pdftoppm` 和 `magick` 是否可用。
- [ ] 在设置页显示外部依赖状态。
- [ ] 外部命令缺失时给出 Windows 安装提示。
- [ ] 为 Windows 增加 `.ico` 图标资源。
- [ ] 新增 Windows NSIS GitHub Actions workflow。
- [ ] 补充 Windows 打包和运行说明到 README。
