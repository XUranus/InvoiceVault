use rusqlite::Connection;

use crate::agent::AgentAttachment;
use crate::exporter::{self, ColumnDef, InvoiceRow};
use crate::template_engine::binder::{DataSource, DataValue};

/// Label matcher for the template engine.
/// Wraps `resolve_export_column_keys_from_labels` to match the engine's
/// `label_matcher` signature: `&[String] -> Vec<(usize, String)>`.
pub fn label_matcher(labels: &[String]) -> Vec<(usize, String)> {
    exporter::resolve_template_column_map(labels)
        .into_iter()
        .map(|(idx, col_def)| (idx, col_def.key.to_owned()))
        .collect()
}

/// Resolve matched column keys into (col_index, &ColumnDef) pairs.
/// Called after the engine's region detection returns matched keys.
pub fn resolve_column_defs(keys: &[(usize, String)]) -> Vec<(usize, &'static ColumnDef)> {
    keys.iter()
        .filter_map(|(idx, key)| {
            exporter::ALL_COLUMNS
                .iter()
                .find(|c| c.key == key.as_str())
                .map(|col_def| (*idx, col_def))
        })
        .collect()
}


/// Generate a `TemplatePlan` from a template attachment using heuristic
/// region detection. Returns `Err` if parsing fails, `Ok(None)` if no
/// header region could be detected.
pub fn generate_plan_from_attachment(
    attachment: &AgentAttachment,
) -> Result<Option<crate::template_engine::plan::TemplatePlan>, String> {
    crate::template_engine::TemplateEngine::generate_heuristic_plan(
        &attachment.storage_path,
        &label_matcher,
    )
    .map_err(|e| format!("模板分析失败: {e}"))
}

/// Adapter that implements `template_engine::binder::DataSource` for invoice export.
pub struct InvoiceDataSource<'a> {
    pub rows: &'a [InvoiceRow],
    pub column_map: &'a [(usize, &'static ColumnDef)],
}

impl DataSource for InvoiceDataSource<'_> {
    fn row_count(&self) -> usize {
        self.rows.len()
    }

    fn cell_value(&self, row_index: usize, col_key: &str) -> Option<DataValue> {
        let row = self.rows.get(row_index)?;
        if let Some((_, col_def)) = self.column_map.iter().find(|(_, cd)| cd.key == col_key) {
            if col_def.numeric {
                return row.number_by_key(col_key).map(DataValue::Number);
            }
        }
        let val = row.field_by_key(col_key);
        if val.is_empty() {
            None
        } else {
            Some(DataValue::String(val))
        }
    }

    fn is_numeric_column(&self, col_key: &str) -> bool {
        self.column_map
            .iter()
            .any(|(_, col_def)| col_def.key == col_key && col_def.numeric)
    }
}

/// Load invoices from the database and return them as InvoiceRow for template export.
pub fn load_invoices(
    conn: &Connection,
    invoice_ids: Option<&[i64]>,
    date_from: Option<&str>,
    date_to: Option<&str>,
) -> Result<Vec<InvoiceRow>, String> {
    exporter::load_invoices_for_export(conn, invoice_ids, date_from, date_to)
        .map_err(|e| e.to_string())
}
