# Agent Excel 模板导出实现说明

本文档描述 Agent 模块中“基于 Excel 模板导出发票”的当前实现。该能力用于让 Agent 接收用户上传的 `.xlsx` 模板，识别模板表头，将系统内的发票数据填入模板，同时最大限度保留原模板的表头、表尾、合并单元格、行高、列宽、样式和空白占位行。

参考模板：

`/home/xuranus/Documents/xwechat_files/wxid_ae1nz60lyw3r11_9dac/msg/file/2026-05/普票移交登记表.xlsx`

## 目标

模板导出需要满足以下约束：

- 表头、标题区、页脚签名区不丢失。
- 原模板格式不被破坏，包括行高、列宽、单元格样式、合并单元格和空白占位行。
- 模板数据区中的样例值不会泄漏到导出结果。
- 汇总行保留公式，并写入缓存值，便于 Excel 和部分预览器直接显示结果。
- 失败信息可追溯到导出阶段、模板路径和输出路径。

## 主要模块

### Agent 工具入口

入口位于 `src-tauri/src/app_core/mod.rs` 的 `export_invoices_with_template` 分支。

职责：

- 校验 `attachment_id` 并确认附件属于当前 Agent 会话。
- 解析模板表头，确认至少能匹配一个可导出的发票字段。
- 在未确认时返回 `ConfirmationRequired`，提示导出行数、模板名和匹配字段。
- 在确认后加载发票数据，创建 Agent task。
- 调用 `TemplateEngine::export` 执行模板导出。
- 成功时记录导出 artifact，并将 task 标记为 `completed`。
- 失败时将错误字符串写入 task，标记为 `failed`。

### Agent 适配层

文件：`src-tauri/src/app_core/template_adapter.rs`

职责：

- `label_matcher(labels)`：把模板行中的中文表头交给 `exporter::resolve_template_column_map` 解析，返回 `(列索引, 字段 key)`。
- `matched_keys_from_attachment(attachment)`：解析附件模板并运行区域识别，返回最佳表头行匹配到的字段。
- `resolve_column_defs(keys)`：将字段 key 转换为 `exporter::ColumnDef`，用于后续取值和判断数值列。
- `InvoiceDataSource`：将 `InvoiceRow` 适配为模板引擎需要的 `DataSource`。

`InvoiceDataSource` 会根据 `ColumnDef.numeric` 区分数值列和文本列。数值列通过 `row.number_by_key()` 输出为 `DataValue::Number`，文本列通过 `row.field_by_key()` 输出为 `DataValue::String`。空字符串会返回 `None`，避免生成无意义内容。

### 模板引擎

目录：`src-tauri/src/template_engine/`

核心文件：

- `mod.rs`：导出总入口、错误类型、真实模板回归测试。
- `parser.rs`：解析 `.xlsx` ZIP 包中的 worksheet、shared strings、rows、cells、merge cells。
- `region.rs`：识别表头行、数据区、汇总起始行。
- `binder.rs`：把数据源绑定到模板 AST，生成中间表示 IR。
- `cloner.rs`：按模板行克隆数据行或空白占位行。
- `writer.rs`：将 IR 写回 `.xlsx`。
- `ir.rs`：定义写出阶段使用的行、列、单元格中间表示。
- `strings.rs`：管理 shared strings。

## 导出流程

完整流程由 `TemplateEngine::export(template_path, output_path, source, label_matcher)` 串联。

1. 复制模板

   先将模板文件复制到输出路径。后续操作都在输出副本上执行，避免改动原模板。

2. 解析 XLSX

   `parser::parse_xlsx(output_path)` 读取 `.xlsx` ZIP 结构，解析：

   - `xl/sharedStrings.xml`
   - workbook 中的 worksheet 路径
   - 每个 sheet 的 `sheetData`
   - 行、单元格、原始 XML、样式索引
   - 合并单元格区域
   - `sheetData` 前后的原始 XML 片段

   解析器会移除 `xml_after_sheet_data` 中原有的 `<mergeCells>` 块，因为写出阶段会重新生成合并单元格，避免重复写入。

3. 识别区域

   `region::recognize_regions()` 遍历每个 sheet，通过 `label_matcher` 计算每一行的字段匹配数，选择匹配数量最高的行作为表头。

   表头下一行是数据区开始行。识别器继续查找“小计”“合计”“总计”“汇总”等汇总标记，确定：

   - `Header` 区域
   - `DataAppend` 区域
   - `summary_start_row`

4. 数据绑定

   `binder::bind()` 是格式保护的关键。

   对有数据区的 sheet：

   - 保留表头之前的所有静态行。
   - 原样保留表头行。
   - 将 `data_start..summary_start-1` 视为模板数据容量区。
   - `output_data_slots = max(发票行数, 模板数据容量)`。
   - 前 `发票行数` 行填入真实发票数据。
   - 剩余行调用 `clone_blank_row()` 输出空白占位行，保留样式但清除样例值和公式。
   - 如果发票行数超过模板容量，则增加数据行，并按偏移移动汇总区和页脚区。
   - 只对包含汇总标记的行生成汇总公式，不会把页脚签名行当作汇总行。

   对没有数据区的 sheet：

   - 所有行按静态内容保留。

5. 写出 XLSX

   `writer::write_xlsx()` 读取输出副本中的 ZIP entries，替换对应 worksheet XML 和必要时的 shared strings XML，其它资源保持 passthrough。

   写出阶段会：

   - 更新 worksheet `<dimension ref="...">`。
   - 重建 `<sheetData>`。
   - 重新写入 `<mergeCells>`。
   - 对公式单元格写入 `<f>...</f>`，数值公式同时写入缓存 `<v>...</v>`。
   - 对 `CellValue::Blank` 写出带样式的空单元格。

## 模板格式保护策略

### 静态行

标题、建设单位、项目名称、表头、页脚签名等静态行通过 `build_static_row()` 保留。

静态行保存：

- 原始 row header，例如行高、hidden、customHeight 等属性。
- 原始 cell XML。
- 单元格样式索引。

当静态行没有发生行号移动时，直接输出原始 XML。当需要移动行号时，写出阶段会重建行，并替换行号和单元格引用。

### 数据行

数据行通过 `clone_row_with_values()` 生成。

实现要点：

- 使用模板对应行的单元格样式。
- 默认将模板单元格清为空白，避免样例数据泄漏。
- 对匹配到的字段写入真实数据。
- 对序号列自动写入 `1, 2, 3...`。序号列通过模板数据行中值为 `1` 的数值单元格识别。
- 对未匹配字段不写入旧模板内容。

### 空白占位行

如果模板预留了多行数据空间，而本次导出数据不足，剩余行通过 `clone_blank_row()` 输出。

这类行会：

- 保留单元格样式、边框、背景、宽高等视觉格式。
- 清除单元格值。
- 清除公式。

以“普票移交登记表”为例，模板数据容量为第 5 到第 25 行。导出 3 条发票时，第 8 到第 25 行仍保留为空白格式行。

### 汇总行

汇总行识别依赖文本标记：

- `小 计`
- `小计`
- `合 计`
- `合计`
- `总计`
- `总 计`
- `汇总`
- `小  计`
- `合  计`

只有包含这些标记的行才会生成汇总公式。页脚签名行即使位于汇总区域之后，也不会被写入汇总公式。

数值列来自 `DataSource::is_numeric_column()`。发票导出中由 `ColumnDef.numeric` 决定。

汇总公式格式：

```text
SUM(H5:H25)
```

同时写入缓存值，例如：

```xml
<c r="H26" s="27"><f>SUM(H5:H25)</f><v>3955</v></c>
```

### 合并单元格

合并单元格由解析器读入 `MergeCell`，写出阶段统一重建。

处理规则：

- 数据区之前的合并区域保留。
- 数据区内部的合并区域保留当前模板定义。
- 数据区之后的合并区域在数据行超过模板容量时按 `row_offset` 下移。
- 写出前先移除原 worksheet 尾部的 `<mergeCells>`，避免重复。

### Sheet 尺寸

写出阶段根据输出行列重新计算 worksheet dimension。例如参考模板在导出 3 条数据后仍保持：

```xml
<dimension ref="A1:L30"/>
```

## 失败可追溯

模板引擎错误类型包含 `TemplateError::Trace`：

```rust
Trace {
    stage: &'static str,
    message: String,
}
```

`TemplateEngine::export()` 对关键阶段统一包装错误：

- `copy_template`
- `parse_xlsx`
- `recognize_regions`
- `bind_data`
- `write_xlsx`
- `stat_output`

错误信息包含：

- stage
- template_path
- output_path
- 原始错误内容

Agent 工具层捕获错误后，会将错误字符串写入对应 task 的失败结果，因此可以从 Agent task 追溯到具体失败阶段。

## 真实模板回归验证

测试位于 `src-tauri/src/template_engine/mod.rs`：

`template_engine::tests::test_template_export_with_real_file`

测试使用参考模板路径。如果本机不存在该模板，会跳过。

当前验证点包括：

- 能成功导出 `.xlsx`。
- 输出 row count 正确。
- worksheet dimension 保持 `A1:L30`。
- 只存在一个 `<mergeCells>` 块。
- 保留 9 个合并单元格区域。
- 第 5、6、7 行序号分别为 1、2、3。
- 第 8 行仍存在，并且为空白占位行，没有样例值和公式。
- “小 计”行保持在第 26 行。
- “合 计”行保持在第 27 行。
- H/I/K 列汇总公式覆盖第 5 到第 25 行。
- 汇总公式包含缓存数值。
- 第 29 行页脚签名信息保留：项目负责人、移交人、接收人、日期。
- K5 等数值字段按发票数据写入。

常用验证命令：

```bash
cd src-tauri
cargo test template_engine::tests::test_template_export_with_real_file -- --nocapture
cargo test
cargo check --all-targets
```

前端构建验证：

```bash
npm run build
```

## 已知边界

- 当前模板识别依赖表头文本匹配，字段别名能力由 `exporter::resolve_template_column_map` 决定。
- 汇总行只对已识别为数值列的字段生成公式。
- 公式缓存值由导出时的数据计算，Excel 打开后仍可按公式重新计算。
- 真实模板回归测试依赖本机示例模板路径，模板不存在时测试会跳过。
- 模板引擎当前主要面向 `.xlsx`，不支持 `.xls`。

## 扩展建议

- 将真实模板样例纳入受控测试资源，避免依赖本机绝对路径。
- 为常见模板字段别名增加单元测试，提升表头匹配稳定性。
- 增加端到端 Agent 工具测试，覆盖附件校验、确认流程、task 失败记录和 artifact 记录。
- 对超出模板容量的数据行增加专门回归测试，验证汇总区、页脚区和合并单元格整体下移。
