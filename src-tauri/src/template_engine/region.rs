use super::ast::TemplateAst;

/// The kind of region in a template.
#[derive(Debug, Clone, PartialEq)]
pub enum RegionKind {
    /// A header row containing column labels.
    Header,
    /// A data append region — new rows are inserted here.
    DataAppend,
}

/// A detected region within a sheet.
#[derive(Debug, Clone)]
pub struct Region {
    pub sheet_index: usize,
    pub kind: RegionKind,
    /// 1-based row number of the start of this region.
    pub start_row: u32,
    /// 1-based row number of the end of this region (inclusive).
    pub end_row: u32,
    /// The column map: (0-based column index, matched key string).
    pub column_map: Vec<(usize, String)>,
    /// Confidence score (0.0 - 1.0) for this region detection.
    pub confidence: f64,
}

/// Recognize data regions in the parsed template.
///
/// `label_matcher` takes a list of cell text values from a row and returns
/// matched (column_index, key) pairs. This makes the engine generic —
/// the caller provides the domain-specific matching logic.
pub fn recognize_regions(
    ast: &TemplateAst,
    label_matcher: &dyn Fn(&[String]) -> Vec<(usize, String)>,
) -> Vec<Region> {
    let mut regions = Vec::new();

    for (sheet_idx, sheet) in ast.sheets.iter().enumerate() {
        if let Some((header_row, column_map, confidence)) =
            find_best_header(&sheet.rows, &ast.shared_strings, label_matcher)
        {
            let data_start = header_row + 1;
            let data_end = find_data_end(&sheet.rows, header_row);

            // Header region
            regions.push(Region {
                sheet_index: sheet_idx,
                kind: RegionKind::Header,
                start_row: header_row,
                end_row: header_row,
                column_map: column_map.clone(),
                confidence,
            });

            // DataAppend region (may be empty if no rows below header)
            if data_start <= data_end {
                regions.push(Region {
                    sheet_index: sheet_idx,
                    kind: RegionKind::DataAppend,
                    start_row: data_start,
                    end_row: data_end,
                    column_map,
                    confidence,
                });
            }
        }
    }

    regions
}

/// Find the best header row by scoring each row with the label matcher.
fn find_best_header(
    rows: &[super::ast::RowAst],
    shared_strings: &[String],
    label_matcher: &dyn Fn(&[String]) -> Vec<(usize, String)>,
) -> Option<(u32, Vec<(usize, String)>, f64)> {
    let mut best_row = 0u32;
    let mut best_map = Vec::new();
    let mut best_count = 0usize;

    for row in rows {
        let labels = row_text_values(row, shared_strings);
        let matches = label_matcher(&labels);
        if matches.len() > best_count {
            best_count = matches.len();
            best_row = row.row_num;
            best_map = matches;
        }
    }

    if best_count >= 2 {
        let confidence = (best_count as f64) / 10.0_f64.min(1.0);
        Some((best_row, best_map, confidence))
    } else {
        None
    }
}

/// Find the end of the data region: the last row with any content after the header.
fn find_data_end(rows: &[super::ast::RowAst], header_row: u32) -> u32 {
    let mut last_content_row = header_row;
    for row in rows {
        if row.row_num > header_row && !row.cells.is_empty() {
            last_content_row = row.row_num;
        }
    }
    last_content_row
}

/// Extract text values from a row's cells (resolving shared strings).
fn row_text_values(
    row: &super::ast::RowAst,
    shared_strings: &[String],
) -> Vec<String> {
    row.cells
        .iter()
        .map(|cell| resolve_cell_text(cell, shared_strings))
        .collect()
}

/// Resolve the display text of a cell.
fn resolve_cell_text(
    cell: &super::ast::CellAst,
    shared_strings: &[String],
) -> String {
    match cell.cell_type.as_deref() {
        Some("s") => cell
            .raw_value
            .as_ref()
            .and_then(|v| v.parse::<usize>().ok())
            .and_then(|idx| shared_strings.get(idx))
            .cloned()
            .unwrap_or_default(),
        Some("inlineStr") => cell.raw_value.clone().unwrap_or_default(),
        _ => cell.raw_value.clone().unwrap_or_default(),
    }
}
