use super::ast::TemplateAst;
use super::region::{resolve_cell_text, Region, RegionKind};
use super::writer::col_index_to_letter;
use serde::{Deserialize, Serialize};

/// A structured plan describing how to populate a template with data.
/// Can be produced by heuristics or by an LLM.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TemplatePlan {
    /// 0-based index of the sheet to populate.
    pub target_sheet: usize,
    /// 1-based row numbers that serve as the header row(s).
    pub header_rows: Vec<u32>,
    /// The data region definition.
    pub data_region: PlanDataRegion,
    /// Column mappings: which template columns map to which data fields.
    pub columns: Vec<PlanColumn>,
    /// 0-based column indices that should auto-increment as row numbers.
    pub sequence_columns: Vec<usize>,
    /// Summary/total rows with formula specifications.
    pub summary_rows: Vec<PlanSummaryRow>,
    /// 1-based row numbers for footer/signature rows (preserved as static).
    pub footer_rows: Vec<u32>,
    /// Warnings or notes from the planner (displayed to user).
    pub warnings: Vec<String>,
    /// Overall confidence of this plan (0.0 - 1.0).
    pub confidence: f64,
    /// Which planner produced this plan.
    pub source: PlanSource,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanDataRegion {
    /// 1-based row number where data rows begin.
    pub start_row: u32,
    /// 1-based row number of the last data/placeholder row (before summary).
    pub end_row: u32,
    /// 1-based row numbers of the "template" data rows whose formatting
    /// gets cloned for each real data row. Usually `[start_row]`.
    pub template_rows: Vec<u32>,
    /// If true, keep empty placeholder rows to fill the template's original
    /// data capacity even when fewer real data rows exist.
    pub preserve_empty_slots: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanColumn {
    /// 0-based column index in the template.
    pub col: usize,
    /// The header label text as it appears in the template (for display).
    pub label: String,
    /// The data field key to bind (e.g. "issue_date", "total_amount").
    pub field_key: String,
    /// Confidence of this specific mapping (0.0 - 1.0).
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlanSummaryRow {
    /// 1-based row number.
    pub row: u32,
    /// "subtotal" or "total".
    pub kind: String,
    /// 0-based column indices that should have SUM formulas.
    pub formula_columns: Vec<usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub enum PlanSource {
    Heuristic,
    Llm,
    UserEdited,
    Cached,
}

// ---------------------------------------------------------------------------
// Conversion: TemplatePlan -> Vec<Region>
// ---------------------------------------------------------------------------

impl TemplatePlan {
    /// Convert this plan into the `Vec<Region>` format that the existing
    /// `binder::bind()` expects. This bridges the new plan system with the
    /// current bind logic, enabling zero-regression Phase 1 adoption.
    pub fn to_regions(&self) -> Vec<Region> {
        let header_row = self.header_rows.first().copied().unwrap_or(1);
        let column_map: Vec<(usize, String)> = self
            .columns
            .iter()
            .map(|c| (c.col, c.field_key.clone()))
            .collect();
        let avg_confidence = if self.columns.is_empty() {
            0.0
        } else {
            self.columns.iter().map(|c| c.confidence).sum::<f64>() / self.columns.len() as f64
        };

        let mut regions = Vec::new();

        // Header region
        regions.push(Region {
            sheet_index: self.target_sheet,
            kind: RegionKind::Header,
            start_row: header_row,
            end_row: header_row,
            column_map: column_map.clone(),
            confidence: avg_confidence,
            summary_start_row: None,
        });

        // DataAppend region
        let summary_start = self.summary_rows.iter().map(|s| s.row).min();
        regions.push(Region {
            sheet_index: self.target_sheet,
            kind: RegionKind::DataAppend,
            start_row: self.data_region.start_row,
            end_row: self.data_region.end_row,
            column_map,
            confidence: self.confidence,
            summary_start_row: summary_start,
        });

        regions
    }
}

// ---------------------------------------------------------------------------
// Validation
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct PlanValidationResult {
    pub valid: bool,
    pub errors: Vec<String>,
    pub warnings: Vec<String>,
}

/// Validate a `TemplatePlan` against the parsed template structure.
///
/// `field_catalog` is a list of `(key, is_numeric)` pairs from the exporter's
/// `ALL_COLUMNS`.
pub fn validate_plan(
    plan: &TemplatePlan,
    sheet_count: usize,
    max_row: u32,
    field_catalog: &[(&str, bool)],
) -> PlanValidationResult {
    let mut errors = Vec::new();
    let mut warnings = Vec::new();

    // 1. Target sheet in range
    if plan.target_sheet >= sheet_count {
        errors.push(format!(
            "target_sheet {} out of range (sheet_count={})",
            plan.target_sheet, sheet_count
        ));
    }

    // 2. Header rows in range
    for &r in &plan.header_rows {
        if r == 0 || r > max_row {
            errors.push(format!("header row {r} out of range (max_row={max_row})"));
        }
    }

    // 3. Data region in range and after header
    let dr = &plan.data_region;
    if dr.start_row == 0 || dr.start_row > max_row {
        errors.push(format!(
            "data_region.start_row {} out of range",
            dr.start_row
        ));
    }
    if dr.end_row < dr.start_row {
        errors.push(format!(
            "data_region.end_row {} < start_row {}",
            dr.end_row, dr.start_row
        ));
    }
    if let Some(&last_header) = plan.header_rows.iter().max() {
        if dr.start_row <= last_header {
            errors.push(format!(
                "data_region.start_row {} overlaps with header row {}",
                dr.start_row, last_header
            ));
        }
    }

    // 4. Template rows within data region
    for &tr in &dr.template_rows {
        if tr < dr.start_row || tr > dr.end_row {
            warnings.push(format!(
                "template row {} is outside data region {}..{}",
                tr, dr.start_row, dr.end_row
            ));
        }
    }

    // 5. field_key must exist in catalog
    let catalog_keys: Vec<&str> = field_catalog.iter().map(|(k, _)| *k).collect();
    for col in &plan.columns {
        if !catalog_keys.contains(&col.field_key.as_str()) {
            errors.push(format!(
                "column {} maps to unknown field_key '{}'",
                col.col, col.field_key
            ));
        }
    }

    // 6. Duplicate field mappings warning
    let mut seen_keys: Vec<&str> = Vec::new();
    for col in &plan.columns {
        if seen_keys.contains(&col.field_key.as_str()) {
            warnings.push(format!(
                "field_key '{}' is mapped to multiple columns",
                col.field_key
            ));
        }
        seen_keys.push(&col.field_key);
    }

    // 7. Summary rows must be after data region
    for sr in &plan.summary_rows {
        if sr.row <= dr.end_row {
            errors.push(format!(
                "summary row {} overlaps with data region (end_row={})",
                sr.row, dr.end_row
            ));
        }
        if sr.row > max_row {
            errors.push(format!("summary row {} out of range", sr.row));
        }
        // 8. Summary formula columns must be numeric
        for &fc in &sr.formula_columns {
            if let Some(col) = plan.columns.iter().find(|c| c.col == fc) {
                if let Some((_, is_num)) = field_catalog
                    .iter()
                    .find(|(k, _)| *k == col.field_key.as_str())
                {
                    if !is_num {
                        errors.push(format!(
                            "summary row {} col {} (field '{}') is not numeric",
                            sr.row, fc, col.field_key
                        ));
                    }
                }
            }
        }
    }

    // 9. Footer rows should not overlap with summary rows
    for &fr in &plan.footer_rows {
        if plan.summary_rows.iter().any(|s| s.row == fr) {
            warnings.push(format!("footer row {fr} overlaps with summary row"));
        }
    }

    // 10. Low-confidence columns
    for col in &plan.columns {
        if col.confidence < 0.7 {
            warnings.push(format!(
                "low confidence ({:.0}%) for column {} -> '{}'",
                col.confidence * 100.0,
                col.col,
                col.field_key
            ));
        }
    }

    PlanValidationResult {
        valid: errors.is_empty(),
        errors,
        warnings,
    }
}

// ---------------------------------------------------------------------------
// Template snapshot generation (for LLM input in Phase 2)
// ---------------------------------------------------------------------------

/// Generate a compressed JSON snapshot of the template suitable for LLM input.
/// Contains only structural/textual information — no raw XML.
pub fn generate_template_snapshot(ast: &TemplateAst) -> serde_json::Value {
    let sheets: Vec<serde_json::Value> = ast
        .sheets
        .iter()
        .enumerate()
        .map(|(idx, sheet)| {
            let max_row = sheet.rows.iter().map(|r| r.row_num).max().unwrap_or(0);
            let max_col = sheet
                .rows
                .iter()
                .flat_map(|r| r.cells.iter().map(|c| c.col))
                .max()
                .unwrap_or(0);

            let rows: Vec<serde_json::Value> = sheet
                .rows
                .iter()
                .map(|row| {
                    let texts: Vec<serde_json::Value> = row
                        .cells
                        .iter()
                        .filter_map(|cell| {
                            let text = resolve_cell_text(cell, &ast.shared_strings);
                            if text.trim().is_empty() {
                                return None;
                            }
                            let col_letter = col_index_to_letter(cell.col);
                            Some(serde_json::json!({
                                "c": col_letter,
                                "text": text
                            }))
                        })
                        .collect();
                    serde_json::json!({
                        "r": row.row_num,
                        "texts": texts
                    })
                })
                .collect();

            let merges: Vec<String> = sheet.merge_cells.iter().map(|m| m.ref_str.clone()).collect();

            serde_json::json!({
                "index": idx,
                "name": sheet.name,
                "dimension": format!("A1:{}{}", col_index_to_letter(max_col), max_row),
                "rows": rows,
                "merges": merges
            })
        })
        .collect();

    let field_catalog: Vec<serde_json::Value> = crate::exporter::ALL_COLUMNS
        .iter()
        .map(|col| {
            serde_json::json!({
                "key": col.key,
                "label": col.label,
                "type": if col.numeric { "number" } else { "string" },
                "aliases": col.aliases
            })
        })
        .collect();

    serde_json::json!({
        "sheets": sheets,
        "field_catalog": field_catalog
    })
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    fn make_test_plan() -> TemplatePlan {
        TemplatePlan {
            target_sheet: 0,
            header_rows: vec![4],
            data_region: PlanDataRegion {
                start_row: 5,
                end_row: 25,
                template_rows: vec![5],
                preserve_empty_slots: true,
            },
            columns: vec![
                PlanColumn {
                    col: 1,
                    label: "开具时间".into(),
                    field_key: "issue_date".into(),
                    confidence: 0.9,
                },
                PlanColumn {
                    col: 7,
                    label: "价税合计".into(),
                    field_key: "total_amount".into(),
                    confidence: 0.95,
                },
            ],
            sequence_columns: vec![0],
            summary_rows: vec![PlanSummaryRow {
                row: 26,
                kind: "subtotal".into(),
                formula_columns: vec![7],
            }],
            footer_rows: vec![29],
            warnings: vec![],
            confidence: 0.9,
            source: PlanSource::Heuristic,
        }
    }

    #[test]
    fn test_to_regions() {
        let plan = make_test_plan();
        let regions = plan.to_regions();

        assert_eq!(regions.len(), 2);

        // Header region
        let header = &regions[0];
        assert_eq!(header.kind, RegionKind::Header);
        assert_eq!(header.start_row, 4);
        assert_eq!(header.sheet_index, 0);
        assert_eq!(header.column_map.len(), 2);
        assert_eq!(header.column_map[0], (1, "issue_date".to_string()));
        assert_eq!(header.column_map[1], (7, "total_amount".to_string()));

        // DataAppend region
        let data = &regions[1];
        assert_eq!(data.kind, RegionKind::DataAppend);
        assert_eq!(data.start_row, 5);
        assert_eq!(data.end_row, 25);
        assert_eq!(data.summary_start_row, Some(26));
    }

    #[test]
    fn test_validate_plan_valid() {
        let plan = make_test_plan();
        let catalog = vec![
            ("issue_date", false),
            ("total_amount", true),
        ];
        let result = validate_plan(&plan, 1, 30, &catalog);
        assert!(result.valid, "errors: {:?}", result.errors);
    }

    #[test]
    fn test_validate_plan_bad_sheet() {
        let mut plan = make_test_plan();
        plan.target_sheet = 5;
        let catalog = vec![("issue_date", false), ("total_amount", true)];
        let result = validate_plan(&plan, 1, 30, &catalog);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("target_sheet")));
    }

    #[test]
    fn test_validate_plan_unknown_field() {
        let mut plan = make_test_plan();
        plan.columns.push(PlanColumn {
            col: 3,
            label: "未知".into(),
            field_key: "nonexistent_field".into(),
            confidence: 0.5,
        });
        let catalog = vec![("issue_date", false), ("total_amount", true)];
        let result = validate_plan(&plan, 1, 30, &catalog);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("nonexistent_field")));
    }

    #[test]
    fn test_validate_plan_summary_overlaps_data() {
        let mut plan = make_test_plan();
        plan.summary_rows.push(PlanSummaryRow {
            row: 20, // within data region 5..25
            kind: "total".into(),
            formula_columns: vec![7],
        });
        let catalog = vec![("issue_date", false), ("total_amount", true)];
        let result = validate_plan(&plan, 1, 30, &catalog);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("overlaps with data region")));
    }

    #[test]
    fn test_validate_plan_non_numeric_summary_col() {
        let mut plan = make_test_plan();
        plan.summary_rows = vec![PlanSummaryRow {
            row: 26,
            kind: "subtotal".into(),
            formula_columns: vec![1], // issue_date is NOT numeric
        }];
        let catalog = vec![("issue_date", false), ("total_amount", true)];
        let result = validate_plan(&plan, 1, 30, &catalog);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("not numeric")));
    }

    #[test]
    fn test_validate_plan_low_confidence_warning() {
        let mut plan = make_test_plan();
        plan.columns.push(PlanColumn {
            col: 3,
            label: "地区".into(),
            field_key: "issue_date".into(),
            confidence: 0.3,
        });
        let catalog = vec![("issue_date", false), ("total_amount", true)];
        let result = validate_plan(&plan, 1, 30, &catalog);
        assert!(result.valid);
        assert!(result.warnings.iter().any(|w| w.contains("low confidence")));
    }

    #[test]
    fn test_validate_plan_data_before_header() {
        let mut plan = make_test_plan();
        plan.data_region.start_row = 3; // before header row 4
        let catalog = vec![("issue_date", false), ("total_amount", true)];
        let result = validate_plan(&plan, 1, 30, &catalog);
        assert!(!result.valid);
        assert!(result.errors.iter().any(|e| e.contains("overlaps with header")));
    }
}
