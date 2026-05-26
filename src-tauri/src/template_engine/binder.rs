use super::ast::{MergeCell, RowAst, TemplateAst};
use super::cloner;
use super::ir::*;
use super::region::{Region, RegionKind, SUMMARY_MARKERS};
use super::strings::SharedStringPool;
use super::TemplateError;

/// Trait for providing data to the template engine.
/// Implementors supply rows of keyed values.
pub trait DataSource {
    /// Return the number of data rows available.
    fn row_count(&self) -> usize;

    /// Get a cell value for a given row index and column key.
    /// Returns None if the key is not available for this row.
    fn cell_value(&self, row_index: usize, col_key: &str) -> Option<DataValue>;

    /// Whether this column should be included in generated summary formulas.
    fn is_numeric_column(&self, col_key: &str) -> bool {
        (0..self.row_count()).any(|row_index| {
            matches!(
                self.cell_value(row_index, col_key),
                Some(DataValue::Number(_))
            )
        })
    }
}

/// A typed cell value from the data source.
#[derive(Debug, Clone)]
pub enum DataValue {
    String(String),
    Number(f64),
}

/// Bind data from a DataSource to the template AST, producing a SpreadsheetIR.
pub fn bind(
    ast: &TemplateAst,
    regions: &[Region],
    source: &dyn DataSource,
) -> Result<SpreadsheetIR, TemplateError> {
    if ast.sheets.is_empty() {
        return Err(TemplateError::Bind("template has no sheets".into()));
    }

    let mut ir_sheets = Vec::new();

    for (sheet_idx, sheet) in ast.sheets.iter().enumerate() {
        let sheet_regions: Vec<&Region> = regions
            .iter()
            .filter(|r| r.sheet_index == sheet_idx)
            .collect();

        let header_region = sheet_regions.iter().find(|r| r.kind == RegionKind::Header);
        let data_region = sheet_regions
            .iter()
            .find(|r| r.kind == RegionKind::DataAppend);

        let (column_map, header_row_num, data_start, data_end, summary_start) =
            if let (Some(hr), Some(dr)) = (header_region, data_region) {
                (
                    hr.column_map.clone(),
                    hr.start_row,
                    dr.start_row,
                    dr.end_row,
                    dr.summary_start_row,
                )
            } else {
                // No data region on this sheet — preserve all rows as static
                let rows = build_static_rows(&sheet.rows);
                ir_sheets.push(SheetIR {
                    sheet_index: sheet_idx,
                    rows,
                    merge_cells: sheet.merge_cells.clone(),
                    xml_before_sheet_data: sheet.xml_before_sheet_data.clone(),
                    xml_after_sheet_data: sheet.xml_after_sheet_data.clone(),
                });
                continue;
            };

        // Find the template data row (first row in data region with actual values)
        let template_row = sheet
            .rows
            .iter()
            .find(|r| r.row_num >= data_start && row_has_values(r))
            .cloned();

        // Detect sequence columns (columns where the template data row has value "1")
        let seq_cols = detect_sequence_columns(template_row.as_ref());

        // The clone range is the template's data capacity, including
        // preformatted blank placeholder rows before summary/footer rows.
        let clone_end = summary_start.map(|s| s - 1).unwrap_or(data_end);
        let data_count = source.row_count();
        let template_capacity = clone_end.saturating_sub(data_start).saturating_add(1) as usize;
        let output_data_slots = data_count.max(template_capacity);
        let row_offset = output_data_slots as i32 - template_capacity as i32;

        let mut ir_rows = Vec::new();

        // 1. Static rows before header
        for row in &sheet.rows {
            if row.row_num < header_row_num {
                ir_rows.push(build_static_row(row));
            }
        }

        // 2. Header row (preserved as-is)
        if let Some(header_row) = sheet.rows.iter().find(|r| r.row_num == header_row_num) {
            ir_rows.push(build_static_row(header_row));
        }

        let template_rows_in_clone_range: Vec<&RowAst> = sheet
            .rows
            .iter()
            .filter(|r| r.row_num >= data_start && r.row_num <= clone_end)
            .collect();

        // 3. Data rows plus any preformatted blank placeholder rows.
        if let Some(ref tmpl) = template_row {
            for slot_idx in 0..output_data_slots {
                let target_row_num = data_start + slot_idx as u32;
                let style_row = template_rows_in_clone_range
                    .get(slot_idx)
                    .copied()
                    .unwrap_or(tmpl);

                if slot_idx < data_count {
                    let values = column_map.iter().filter_map(|(col_idx, key)| {
                        source.cell_value(slot_idx, key).map(|v| {
                            let cv = match v {
                                DataValue::String(s) => CellValue::String(s),
                                DataValue::Number(n) => CellValue::Number(n),
                            };
                            (*col_idx, cv)
                        })
                    });
                    let mut cloned =
                        cloner::clone_row_with_values(style_row, target_row_num, values);

                    // Auto-fill sequence columns.
                    for &seq_col in &seq_cols {
                        if let Some(cell) = cloned.cells.iter_mut().find(|c| c.col == seq_col) {
                            cell.value = CellValue::Number((slot_idx + 1) as f64);
                            cell.formula = None;
                        }
                    }

                    ir_rows.push(cloned);
                } else {
                    let mut blank = cloner::clone_blank_row(style_row, target_row_num);
                    for &seq_col in &seq_cols {
                        if let Some(cell) = blank.cells.iter_mut().find(|c| c.col == seq_col) {
                            cell.value = CellValue::Blank;
                            cell.formula = None;
                        }
                    }
                    ir_rows.push(blank);
                }
            }
        }

        let numeric_summary_cols: Vec<(usize, f64)> = column_map
            .iter()
            .filter_map(|(col_idx, key)| {
                if !source.is_numeric_column(key) {
                    return None;
                }
                let total = (0..data_count)
                    .filter_map(|row_index| match source.cell_value(row_index, key) {
                        Some(DataValue::Number(value)) => Some(value),
                        _ => None,
                    })
                    .sum();
                Some((*col_idx, total))
            })
            .collect();
        let summary_range_end = data_start + output_data_slots.saturating_sub(1) as u32;

        // 4. Rows after data clone range: summary rows + footer + other static rows
        for row in &sheet.rows {
            if row.row_num <= clone_end {
                continue; // already handled (data rows or preserved placeholder rows)
            }
            if row.row_num > data_end {
                break; // beyond the region
            }

            let shifted_num = (row.row_num as i32 + row_offset).max(1) as u32;
            let mut shifted = build_static_row(row);
            shifted.row_num = shifted_num;
            for cell in &mut shifted.cells {
                cell.row = shifted_num;
            }
            if summary_start.is_some_and(|start| row.row_num >= start)
                && row_contains_summary_marker(row, &ast.shared_strings)
            {
                apply_summary_formulas(
                    &mut shifted,
                    &numeric_summary_cols,
                    data_start,
                    summary_range_end,
                );
            }
            ir_rows.push(shifted);
        }

        // 5. Rows after the data region (footer etc.)
        for row in &sheet.rows {
            if row.row_num > data_end {
                let shifted_num = (row.row_num as i32 + row_offset).max(1) as u32;
                let mut shifted = build_static_row(row);
                shifted.row_num = shifted_num;
                for cell in &mut shifted.cells {
                    cell.row = shifted_num;
                }
                ir_rows.push(shifted);
            }
        }

        // 6. Shift merge cells
        let merge_cells = shift_merges(&sheet.merge_cells, data_start, clone_end, row_offset);

        ir_sheets.push(SheetIR {
            sheet_index: sheet_idx,
            rows: ir_rows,
            merge_cells,
            xml_before_sheet_data: sheet.xml_before_sheet_data.clone(),
            xml_after_sheet_data: sheet.xml_after_sheet_data.clone(),
        });
    }

    let shared_strings = SharedStringPool::from_existing(ast.shared_strings.clone());

    Ok(SpreadsheetIR {
        sheets: ir_sheets,
        shared_strings,
        passthrough_entries: ast.other_entries.clone(),
    })
}

fn apply_summary_formulas(
    row: &mut RowIR,
    numeric_cols: &[(usize, f64)],
    data_start: u32,
    data_end: u32,
) {
    if data_end < data_start {
        return;
    }

    let mut changed = false;
    for &(col, total) in numeric_cols {
        let Some(cell) = row.cells.iter_mut().find(|cell| cell.col == col) else {
            continue;
        };
        let col_letter = super::writer::col_index_to_letter(col);
        cell.formula = Some(format!(
            "SUM({col_letter}{data_start}:{col_letter}{data_end})"
        ));
        cell.value = CellValue::Number(total);
        changed = true;
    }

    if changed {
        row.raw_row_xml = None;
    }
}

/// Build a static RowIR from a RowAst (preserving all cells as-is).
fn build_static_row(row: &RowAst) -> RowIR {
    let cells = row
        .cells
        .iter()
        .map(|c| CellIR {
            col: c.col,
            row: row.row_num,
            value: CellValue::Preserve(c.clone()),
            style_index: c.style_index,
            formula: None,
        })
        .collect();

    // Build raw_row_xml from header + cells
    let mut raw = row.raw_xml_header.clone();
    for cell in &row.cells {
        raw.push_str(&cell.raw_xml);
    }
    raw.push_str("</row>");

    RowIR {
        row_num: row.row_num,
        cells,
        is_data: false,
        template_row_header: Some(row.raw_xml_header.clone()),
        raw_row_xml: Some(raw),
    }
}

/// Build static rows for a sheet with no data region.
fn build_static_rows(rows: &[RowAst]) -> Vec<RowIR> {
    rows.iter().map(build_static_row).collect()
}

/// Check if a row has at least one cell with an actual value.
fn row_has_values(row: &RowAst) -> bool {
    row.cells.iter().any(|c| c.raw_value.is_some())
}

fn row_contains_summary_marker(row: &RowAst, shared_strings: &[String]) -> bool {
    row.cells.iter().any(|cell| {
        let Some(raw_value) = cell.raw_value.as_deref() else {
            return false;
        };
        let text = if cell.cell_type.as_deref() == Some("s") {
            raw_value
                .parse::<usize>()
                .ok()
                .and_then(|index| shared_strings.get(index))
                .map(String::as_str)
                .unwrap_or("")
        } else {
            raw_value
        };
        SUMMARY_MARKERS.iter().any(|marker| text.contains(marker))
    })
}

/// Detect sequence columns: columns where the template data row has a numeric value of 1.
/// Returns the list of 0-based column indices that should auto-increment.
fn detect_sequence_columns(template_row: Option<&RowAst>) -> Vec<usize> {
    let Some(row) = template_row else {
        return Vec::new();
    };
    let mut seq_cols = Vec::new();
    for cell in &row.cells {
        // Numeric cells: no type attribute (default) or explicit t="n"
        let is_numeric = cell.cell_type.is_none() || cell.cell_type.as_deref() == Some("n");
        if is_numeric {
            if let Some(ref val) = cell.raw_value {
                if val.trim() == "1" {
                    seq_cols.push(cell.col);
                }
            }
        }
    }
    seq_cols
}

/// Shift merge cells that are below the data region.
fn shift_merges(
    merges: &[MergeCell],
    data_start: u32,
    data_end: u32,
    row_offset: i32,
) -> Vec<MergeCell> {
    if row_offset == 0 {
        return merges.to_vec();
    }

    merges
        .iter()
        .map(|m| {
            if m.start_row > data_end {
                // Below data region — shift down
                let new_start = (m.start_row as i32 + row_offset).max(1) as u32;
                let new_end = (m.end_row as i32 + row_offset).max(1) as u32;
                MergeCell {
                    ref_str: format!("{}:{}", m.ref_str.split(':').next().unwrap_or(""), ""), // will be recomputed
                    start_row: new_start,
                    end_row: new_end,
                    start_col: m.start_col,
                    end_col: m.end_col,
                }
            } else if m.start_row >= data_start && m.end_row <= data_end {
                // Inside data region — skip (will be replaced by data rows)
                // Actually, we should keep merge cells from the template row
                // For now, skip them
                m.clone()
            } else {
                m.clone()
            }
        })
        .collect()
}
