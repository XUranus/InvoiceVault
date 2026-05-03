# Agent workflow and tool design

This document describes the current Agent execution model and the next steps for richer task orchestration.

## Current scope

The Agent can call OpenAI-compatible chat-completion function tools. The current tool set includes:

- `search_invoices`: search invoices with keyword, invoice summary fields, line-item content, date range, seller, buyer, invoice number, invoice type, category, status, duplicate status, amount range, paging, and sorting parameters.
- `get_invoice_detail`: read one invoice with detail rows and custom badges.
- `get_dashboard_stats`: read dashboard statistics for an optional date range.
- `get_current_date_context`: return stable date context for relative phrases such as this month.
- `get_invoice_field_catalog`: return exportable invoice fields, Chinese labels, aliases, and data types.
- `list_message_attachments`: list files uploaded in the current Agent session.
- `inspect_spreadsheet`: inspect uploaded `xlsx` or `csv` files and return sheet name, header row, columns, and sample rows.
- `create_export_preview`: preview an export with row count, selected columns, and sample rows before writing a file.
- `export_invoices`: export invoices as `csv` or `xlsx`, with optional invoice IDs, date range, and selected columns. This tool requires user confirmation and a save path.
- `export_invoices_with_template`: export invoices using the uploaded spreadsheet's header order. The current implementation maps template columns to invoice fields; it does not clone styles, formulas, merged cells, or multiple sheets.
- `update_invoice`: update invoice fields. This tool requires user confirmation.

## Attachment flow

1. The frontend uploads `xlsx` or `csv` files with `attach_agent_file`.
2. The backend copies the file into `app_data/agent_uploads/<session_id>/` and records it in `agent_attachments`.
3. When the user sends a message with attachments, the message is linked to those attachment IDs.
4. The model receives attachment context and can call `list_message_attachments` or `inspect_spreadsheet`.

The current version extracts template columns from spreadsheets and can export using the matched column order. It does not yet clone workbook styles, formulas, merged cells, or multiple-sheet layouts.

## Tasks and artifacts

Write tools that generate durable outputs should create an `agent_tasks` row while running and complete it with `completed` or `failed`.

Generated files are recorded in `agent_artifacts` with the session, task, file path, MIME type, byte size, and metadata such as row count and columns. Export tool results include both the export payload and the persisted artifact/task summary so the chat UI can show the saved file clearly.

The Agent page shows session artifacts in a side panel. Users can open generated files, open the containing folder, copy the saved path, or remove the artifact record from the session. Removing an artifact record does not delete the exported file from disk.

## Streaming UI

The frontend uses `send_agent_message_stream` and `confirm_agent_action_stream` for interactive turns. The backend sends OpenAI-compatible chat requests with `stream: true`, parses SSE chunks, and forwards assistant deltas and tool progress through the Tauri `agent://stream` event.

The UI renders the user message immediately, then shows a temporary assistant bubble with animated thinking/tool states. When the command completes, temporary messages are replaced by the persisted `agent_messages` returned by the backend.

## Example task flow

User request:

> 按照表格里的格式，导出这个月的发票信息。发票内容需要是办公用品，得是增值税的。

Expected tool sequence:

1. `get_current_date_context` to resolve this month into `date_from` and `date_to`.
2. `list_message_attachments` to locate the uploaded spreadsheet.
3. `inspect_spreadsheet` to read template columns.
4. `get_invoice_field_catalog` to map template labels to invoice field keys.
5. `search_invoices` to find invoices matching date range, office-supply content/category, and VAT invoice type.
6. `create_export_preview` with the mapped columns and matched `invoice_ids` or date range.
7. `export_invoices_with_template` after the user confirms the preview and chooses a save path.

## Near-term backlog

- Enhance `export_invoices_with_template` to copy column widths, styles, sheet names, and possibly multiple sheets from the uploaded workbook.
- Add badge filters to Agent search and export flows.
- Add task filtering and retry controls in the Agent UI.
- Add artifact cleanup policies for deleted sessions and missing files.

## Implementation notes

- Keep read-only tools separate from write tools. Write tools should continue to return `ConfirmationRequired`.
- Use `get_invoice_field_catalog` before custom-column exports. Avoid hard-coding field aliases in prompts only.
- For relative dates, call `get_current_date_context`; do not let the model infer dates without tool context.
- For spreadsheet templates, inspect the uploaded file before choosing export columns.
