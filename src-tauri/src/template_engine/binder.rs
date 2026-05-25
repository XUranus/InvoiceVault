use super::ast::{MergeCell, RowAst, TemplateAst};
use super::cloner;
use super::ir::*;
use super::region::{Region, RegionKind};
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

        let (column_map, header_row_num, data_start, data_end) =
            if let (Some(hr), Some(dr)) = (header_region, data_region) {
                (
                    hr.column_map.clone(),
                    hr.start_row,
                    dr.start_row,
                    dr.end_row,
                )
            } else {
                // No data region on this sheet — preserve all rows as static
                let rows = build_static_rows(&sheet.rows, &sheet.full_xml);
                ir_sheets.push(SheetIR {
                    sheet_index: sheet_idx,
                    rows,
                    merge_cells: sheet.merge_cells.clone(),
                    xml_before_sheet_data: sheet.xml_before_sheet_data.clone(),
                    xml_after_sheet_data: sheet.xml_after_sheet_data.clone(),
                });
                continue;
            };

        // Find the template data row (first row in data region)
        let template_row = sheet.rows.iter().find(|r| r.row_num == data_start).cloned();

        let data_count = source.row_count();
        let row_offset = if data_count > 0 {
            data_count as i32 - 1 // template row is replaced by N rows
        } else {
            -1 // no data, template row is removed
        };

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

        // 3. Data rows
        if let Some(ref tmpl) = template_row {
            for data_idx in 0..data_count {
                let target_row_num = data_start + data_idx as u32;
                let values = column_map.iter().filter_map(|(col_idx, key)| {
                    source.cell_value(data_idx, key).map(|v| {
                        let cv = match v {
                            DataValue::String(s) => CellValue::String(s),
                            DataValue::Number(n) => CellValue::Number(n),
                        };
                        (*col_idx, cv)
                    })
                });
                let cloned = cloner::clone_row_with_values(tmpl, target_row_num, values);
                ir_rows.push(cloned);
            }
        }

        // 4. Static rows after data region (shifted down)
        for row in &sheet.rows {
            if row.row_num > data_end {
                let shifted_num = (row.row_num as i32 + row_offset).max(1) as u32;
                let mut shifted = build_static_row(row);
                shifted.row_num = shifted_num;
                // Update cell row numbers
                for cell in &mut shifted.cells {
                    cell.row = shifted_num;
                }
                ir_rows.push(shifted);
            }
        }

        // 5. Shift merge cells
        let merge_cells = shift_merges(&sheet.merge_cells, data_start, data_end, row_offset);

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
        template_row_header: None,
        raw_row_xml: Some(raw),
    }
}

/// Build static rows for a sheet with no data region.
fn build_static_rows(rows: &[RowAst], _full_xml: &str) -> Vec<RowIR> {
    rows.iter().map(build_static_row).collect()
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
