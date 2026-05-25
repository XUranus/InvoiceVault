use super::ir::{CellIR, CellValue, RowIR, SheetIR, SpreadsheetIR};
use super::strings::SharedStringPool;
use super::TemplateError;
use std::io::{Read, Write};

/// Write the SpreadsheetIR back to an XLSX file by patching the ZIP.
pub fn write_xlsx(path: &str, ir: &mut SpreadsheetIR) -> Result<(), TemplateError> {
    // Build sheet path -> index mapping
    let sheet_count = ir.sheets.len();

    // Read all entries from the existing ZIP
    let file = std::fs::File::open(path).map_err(|e| TemplateError::Zip(e.to_string()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| TemplateError::Zip(e.to_string()))?;

    let mut entries: Vec<(String, Vec<u8>)> = Vec::new();
    for i in 0..archive.len() {
        let mut entry = archive
            .by_index(i)
            .map_err(|e| TemplateError::Zip(e.to_string()))?;
        let name = entry.name().to_owned();
        let mut data = Vec::new();
        entry
            .read_to_end(&mut data)
            .map_err(|e| TemplateError::Zip(e.to_string()))?;
        entries.push((name, data));
    }
    drop(archive);

    // Prepare replacement content for sheets and shared strings
    let ss_xml = if ir.shared_strings.is_extended() {
        Some(ir.shared_strings.to_xml())
    } else {
        None
    };

    // Render sheet XMLs
    let mut sheet_xmls: Vec<String> = Vec::with_capacity(sheet_count);
    for sheet in &ir.sheets {
        sheet_xmls.push(render_sheet_xml(sheet, &ir.shared_strings));
    }

    // Write new ZIP
    let out_file = std::fs::File::create(path).map_err(|e| TemplateError::Write(e.to_string()))?;
    let mut writer = zip::ZipWriter::new(out_file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);

    for (name, data) in &entries {
        writer
            .start_file(name, options)
            .map_err(|e| TemplateError::Write(e.to_string()))?;

        // Check if this is a sheet we need to replace
        if let Some(sheet_idx) = find_sheet_index(name, sheet_count) {
            if let Some(sheet_xml) = sheet_xmls.get(sheet_idx) {
                writer
                    .write_all(sheet_xml.as_bytes())
                    .map_err(|e| TemplateError::Write(e.to_string()))?;
                continue;
            }
        }

        // Check if this is sharedStrings.xml we need to replace
        if name == "xl/sharedStrings.xml" {
            if let Some(ref ss) = ss_xml {
                writer
                    .write_all(ss.as_bytes())
                    .map_err(|e| TemplateError::Write(e.to_string()))?;
                continue;
            }
        }

        // Pass through unchanged
        writer
            .write_all(data)
            .map_err(|e| TemplateError::Write(e.to_string()))?;
    }

    writer
        .finish()
        .map_err(|e| TemplateError::Write(e.to_string()))?;
    Ok(())
}

/// Try to match a ZIP entry name to a sheet index.
fn find_sheet_index(name: &str, _sheet_count: usize) -> Option<usize> {
    // Match patterns like "xl/worksheets/sheet1.xml", "xl/worksheets/sheet2.xml"
    if !name.starts_with("xl/worksheets/sheet") || !name.ends_with(".xml") {
        return None;
    }
    let num_str = &name["xl/worksheets/sheet".len()..name.len() - ".xml".len()];
    let num: usize = num_str.parse().ok()?;
    // Sheets are 1-based in XLSX, 0-based in our vec
    Some(num.checked_sub(1)?)
}

/// Render a SheetIR back to sheet XML.
fn render_sheet_xml(sheet: &SheetIR, strings: &SharedStringPool) -> String {
    let mut xml = sheet.xml_before_sheet_data.clone();
    xml.push_str("<sheetData>");

    for row in &sheet.rows {
        xml.push_str(&render_row_xml(row, strings));
    }

    xml.push_str("</sheetData>");

    // Merge cells
    if !sheet.merge_cells.is_empty() {
        xml.push_str(&format!(
            r#"<mergeCells count="{}">"#,
            sheet.merge_cells.len()
        ));
        for mc in &sheet.merge_cells {
            // Rebuild ref_str from start/end
            let start_ref = format!(
                "{}{}",
                col_index_to_letter(mc.start_col),
                mc.start_row
            );
            let end_ref = format!("{}{}", col_index_to_letter(mc.end_col), mc.end_row);
            xml.push_str(&format!(r#"<mergeCell ref="{start_ref}:{end_ref}"/>"#));
        }
        xml.push_str("</mergeCells>");
    }

    xml.push_str(&sheet.xml_after_sheet_data);
    xml
}

/// Render a single RowIR to <row>...</row> XML.
fn render_row_xml(row: &RowIR, strings: &SharedStringPool) -> String {
    // For static rows, use the preserved raw XML
    if !row.is_data {
        if let Some(ref raw) = row.raw_row_xml {
            return raw.clone();
        }
    }

    let mut xml = if let Some(ref header) = row.template_row_header {
        // Use the template row's header (preserves spans, height, etc.)
        // Replace the row number
        replace_row_number(header, row.row_num)
    } else {
        format!(r#"<row r="{}">"#, row.row_num)
    };

    for cell in &row.cells {
        xml.push_str(&render_cell_xml(cell, row.row_num, strings));
    }

    xml.push_str("</row>");
    xml
}

/// Render a CellIR to <c>...</c> XML.
fn render_cell_xml(cell: &CellIR, row_num: u32, strings: &SharedStringPool) -> String {
    match &cell.value {
        CellValue::Preserve(ast_cell) => {
            // For static rows, emit raw XML but update row number if needed
            if ast_cell.row == row_num {
                ast_cell.raw_xml.clone()
            } else {
                // Update cell reference with new row number
                let new_ref = format!("{}{}", col_index_to_letter(ast_cell.col), row_num);
                replace_cell_ref(&ast_cell.raw_xml, &new_ref)
            }
        }
        CellValue::String(s) => {
            let ref_str = format!("{}{}", col_index_to_letter(cell.col), row_num);
            let ss_idx = strings.index_of(s).unwrap_or(0);
            let style_attr = cell
                .style_index
                .map(|s| format!(r#" s="{s}""#))
                .unwrap_or_default();
            format!(r#"<c r="{ref_str}" t="s"{style_attr}><v>{ss_idx}</v></c>"#)
        }
        CellValue::Number(n) => {
            let ref_str = format!("{}{}", col_index_to_letter(cell.col), row_num);
            let style_attr = cell
                .style_index
                .map(|s| format!(r#" s="{s}""#))
                .unwrap_or_default();
            format!(r#"<c r="{ref_str}"{style_attr}><v>{n}</v></c>"#)
        }
    }
}

/// Replace the row number in a <row r="N" ...> tag.
fn replace_row_number(header: &str, new_row: u32) -> String {
    if let Some(r_pos) = header.find(" r=\"") {
        let value_start = r_pos + " r=\"".len();
        if let Some(value_end) = header[value_start..].find('"') {
            return format!(
                "{}{}{}",
                &header[..value_start],
                new_row,
                &header[value_start + value_end..]
            );
        }
    }
    format!(r#"<row r="{new_row}">"#)
}

/// Replace the cell reference in a <c r="..." ...> tag.
fn replace_cell_ref(raw_xml: &str, new_ref: &str) -> String {
    if let Some(r_pos) = raw_xml.find(" r=\"") {
        let value_start = r_pos + " r=\"".len();
        if let Some(value_end) = raw_xml[value_start..].find('"') {
            return format!(
                "{}{}{}",
                &raw_xml[..value_start],
                new_ref,
                &raw_xml[value_start + value_end..]
            );
        }
    }
    raw_xml.to_owned()
}

/// Convert 0-based column index to letter(s): 0 -> "A", 25 -> "Z", 26 -> "AA".
pub fn col_index_to_letter(mut idx: usize) -> String {
    let mut result = String::new();
    idx += 1; // 0-based to 1-based
    while idx > 0 {
        let rem = (idx - 1) % 26;
        result.insert(0, (b'A' + rem as u8) as char);
        idx = (idx - 1) / 26;
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_col_index_to_letter() {
        assert_eq!(col_index_to_letter(0), "A");
        assert_eq!(col_index_to_letter(25), "Z");
        assert_eq!(col_index_to_letter(26), "AA");
        assert_eq!(col_index_to_letter(51), "AZ");
        assert_eq!(col_index_to_letter(52), "BA");
    }

    #[test]
    fn test_replace_row_number() {
        assert_eq!(
            replace_row_number(r#"<row r="1" spans="1:5">"#, 10),
            r#"<row r="10" spans="1:5">"#
        );
    }

    #[test]
    fn test_find_sheet_index() {
        assert_eq!(find_sheet_index("xl/worksheets/sheet1.xml", 3), Some(0));
        assert_eq!(find_sheet_index("xl/worksheets/sheet2.xml", 3), Some(1));
        assert_eq!(find_sheet_index("xl/styles.xml", 3), None);
    }
}
