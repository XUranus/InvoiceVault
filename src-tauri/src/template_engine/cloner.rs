use super::ast::RowAst;
use super::ir::{CellIR, CellValue, RowIR};
use std::collections::HashMap;

/// Clone a template data row's formatting for a new data row.
///
/// - `template_row`: the source row from the AST (the "template data row")
/// - `target_row_num`: the 1-based row number for the new row
/// - `values`: iterator of (col_index, CellValue) pairs to insert
pub fn clone_row_with_values(
    template_row: &RowAst,
    target_row_num: u32,
    values: impl Iterator<Item = (usize, CellValue)>,
) -> RowIR {
    let value_map: HashMap<usize, CellValue> = values.collect();

    // Start by cloning all template cells as blank cells, preserving style but
    // clearing sample values from the template data area.
    let mut cells: Vec<CellIR> = template_row
        .cells
        .iter()
        .map(|c| {
            let formula = extract_formula_from_raw_xml(&c.raw_xml)
                .map(|formula| shift_formula_row(&formula, target_row_num as i32 - c.row as i32));
            CellIR {
                col: c.col,
                row: target_row_num,
                value: CellValue::Blank,
                style_index: c.style_index,
                formula,
            }
        })
        .collect();

    // Create a col->index lookup for the cells vec
    let col_to_idx: HashMap<usize, usize> =
        cells.iter().enumerate().map(|(i, c)| (c.col, i)).collect();

    // Overlay data values
    for (col_idx, new_value) in &value_map {
        if let Some(&cell_idx) = col_to_idx.get(col_idx) {
            // Template has this cell — override value, keep style
            cells[cell_idx].value = new_value.clone();
            cells[cell_idx].formula = None;
        } else {
            // Template does not have this cell — create new
            cells.push(CellIR {
                col: *col_idx,
                row: target_row_num,
                value: new_value.clone(),
                style_index: None,
                formula: None,
            });
        }
    }

    // Sort by column
    cells.sort_by_key(|c| c.col);

    RowIR {
        row_num: target_row_num,
        cells,
        is_data: true,
        template_row_header: Some(template_row.raw_xml_header.clone()),
        raw_row_xml: None,
    }
}

/// Clone a template row as an empty placeholder row, keeping cell styles but
/// removing values and formulas so old sample data cannot leak into exports.
pub fn clone_blank_row(template_row: &RowAst, target_row_num: u32) -> RowIR {
    let cells = template_row
        .cells
        .iter()
        .map(|c| CellIR {
            col: c.col,
            row: target_row_num,
            value: CellValue::Blank,
            style_index: c.style_index,
            formula: None,
        })
        .collect();

    RowIR {
        row_num: target_row_num,
        cells,
        is_data: true,
        template_row_header: Some(template_row.raw_xml_header.clone()),
        raw_row_xml: None,
    }
}

/// Shift a formula's row references by `offset`.
/// Handles patterns like A1, $A$1, A$1, $A1.
pub fn shift_formula_row(formula: &str, offset: i32) -> String {
    if offset == 0 {
        return formula.to_owned();
    }

    let mut result = String::with_capacity(formula.len());
    let chars: Vec<char> = formula.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        // Look for a cell reference pattern: optional $, letters, optional $, digits
        if chars[i].is_ascii_uppercase()
            || (chars[i] == '$' && i + 1 < len && chars[i + 1].is_ascii_uppercase())
        {
            let start = i;
            let mut col_part = String::new();

            // Parse column part
            if chars[i] == '$' {
                col_part.push('$');
                i += 1;
            }
            while i < len && chars[i].is_ascii_uppercase() {
                col_part.push(chars[i]);
                i += 1;
            }

            // Must have at least one letter
            if col_part.trim_start_matches('$').is_empty() {
                for j in start..i {
                    result.push(chars[j]);
                }
                continue;
            }

            // Parse row part
            let mut has_dollar_before_row = false;
            let mut row_digits = String::new();
            if i < len && chars[i] == '$' {
                has_dollar_before_row = true;
                i += 1;
            }
            while i < len && chars[i].is_ascii_digit() {
                row_digits.push(chars[i]);
                i += 1;
            }

            if row_digits.is_empty() {
                for j in start..i {
                    result.push(chars[j]);
                }
                continue;
            }

            // Check this isn't a function name (cell refs need at least 1 letter + 1 digit)
            if col_part.trim_start_matches('$').is_empty() {
                for j in start..i {
                    result.push(chars[j]);
                }
                continue;
            }

            // Check if next char makes this look like a function name (e.g., SUM)
            if i < len && chars[i].is_ascii_alphabetic() {
                for j in start..i {
                    result.push(chars[j]);
                }
                continue;
            }

            if let Ok(row_num) = row_digits.parse::<i32>() {
                result.push_str(&col_part);
                if has_dollar_before_row {
                    result.push('$');
                    result.push_str(&row_digits);
                } else {
                    let new_row = (row_num + offset).max(1);
                    result.push_str(&new_row.to_string());
                }
            } else {
                for j in start..i {
                    result.push(chars[j]);
                }
            }
        } else {
            result.push(chars[i]);
            i += 1;
        }
    }

    result
}

/// Extract a formula from raw cell XML (look for <f>...</f>).
fn extract_formula_from_raw_xml(raw_xml: &str) -> Option<String> {
    let start_tag = "<f>";
    let end_tag = "</f>";
    let start = raw_xml.find(start_tag)? + start_tag.len();
    let end = raw_xml[start..].find(end_tag)?;
    Some(raw_xml[start..start + end].to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shift_formula_simple() {
        assert_eq!(shift_formula_row("SUM(A1:A10)", 5), "SUM(A6:A15)");
        assert_eq!(shift_formula_row("B2*C2", 3), "B5*C5");
        assert_eq!(shift_formula_row("A1", 0), "A1");
    }

    #[test]
    fn test_shift_formula_absolute() {
        assert_eq!(shift_formula_row("$A$1", 5), "$A$1");
        assert_eq!(shift_formula_row("A$1", 5), "A$1");
        // $A1 — absolute column, relative row
        assert_eq!(shift_formula_row("$A1", 5), "$A6");
    }

    #[test]
    fn test_shift_formula_no_offset() {
        assert_eq!(shift_formula_row("SUM(A1:A10)", 0), "SUM(A1:A10)");
    }
}
