use super::ast::*;
use super::TemplateError;
use std::io::Read;

/// Parse an XLSX file into a TemplateAst. Reads ALL sheets.
pub fn parse_xlsx(path: &str) -> Result<TemplateAst, TemplateError> {
    let file = std::fs::File::open(path).map_err(|e| TemplateError::Zip(e.to_string()))?;
    let mut archive =
        zip::ZipArchive::new(file).map_err(|e| TemplateError::Zip(e.to_string()))?;

    // 1. Read shared strings
    let shared_strings = {
        let mut xml = String::new();
        if let Ok(mut f) = archive.by_name("xl/sharedStrings.xml") {
            f.read_to_string(&mut xml)
                .map_err(|e| TemplateError::Zip(e.to_string()))?;
        }
        parse_shared_strings(&xml)
    };

    // 2. Discover sheet paths from workbook.xml
    let workbook_xml = read_entry(&mut archive, "xl/workbook.xml").unwrap_or_default();
    let sheet_paths = discover_sheet_paths(&mut archive, &workbook_xml);

    // 3. Parse each sheet
    let mut sheets = Vec::new();
    for (name, xml_path) in &sheet_paths {
        if let Some(xml) = read_entry(&mut archive, xml_path) {
            sheets.push(parse_sheet(name, xml_path, &xml, &shared_strings));
        }
    }

    // 4. Collect other entries (passthrough)
    let sheet_path_set: std::collections::HashSet<&str> =
        sheet_paths.iter().map(|(_, p)| p.as_str()).collect();
    let skip_paths: std::collections::HashSet<&str> = [
        "xl/sharedStrings.xml",
        "xl/workbook.xml",
        "xl/workbook.xml.rels",
    ]
    .into_iter()
    .collect();

    let mut other_entries = Vec::new();
    for i in 0..archive.len() {
        if let Ok(mut entry) = archive.by_index(i) {
            let name = entry.name().to_owned();
            if sheet_path_set.contains(name.as_str()) || skip_paths.contains(name.as_str()) {
                continue;
            }
            let mut data = Vec::new();
            if entry.read_to_end(&mut data).is_ok() {
                other_entries.push((name, data));
            }
        }
    }

    Ok(TemplateAst {
        sheets,
        shared_strings,
        other_entries,
    })
}

/// Discover sheet paths and names from workbook.xml + workbook.xml.rels.
fn discover_sheet_paths(
    archive: &mut zip::ZipArchive<std::fs::File>,
    workbook_xml: &str,
) -> Vec<(String, String)> {
    // Read workbook.xml.rels to map rId -> file path
    let rels_xml = read_entry(archive, "xl/_rels/workbook.xml.rels").unwrap_or_default();
    let rid_map = parse_rels(&rels_xml);

    // Parse <sheet> elements from workbook.xml
    let mut sheets = Vec::new();
    for segment in workbook_xml.split("<sheet").skip(1) {
        let segment = segment.split('>').next().unwrap_or("");
        let name = extract_attr(segment, "name").unwrap_or_default();
        let rid = extract_attr(segment, "r:id")
            .or_else(|| extract_attr(segment, "id"))
            .unwrap_or_default();
        if let Some(file_path) = rid_map.get(rid.as_str()) {
            sheets.push((name, format!("xl/{file_path}")));
        }
    }

    // Fallback: if no sheets discovered, try sheet1.xml
    if sheets.is_empty() {
        sheets.push(("Sheet1".into(), "xl/worksheets/sheet1.xml".into()));
    }

    sheets
}

/// Parse .rels file to get rId -> target path mapping.
fn parse_rels(xml: &str) -> std::collections::HashMap<String, String> {
    let mut map = std::collections::HashMap::new();
    for segment in xml.split("<Relationship").skip(1) {
        let segment = segment.split('>').next().unwrap_or("");
        let id = extract_attr(segment, "Id").unwrap_or_default();
        let target = extract_attr(segment, "Target").unwrap_or_default();
        if !id.is_empty() && !target.is_empty() {
            map.insert(id, target);
        }
    }
    map
}

/// Parse a single sheet XML into a SheetAst.
fn parse_sheet(name: &str, path: &str, xml: &str, shared_strings: &[String]) -> SheetAst {
    // Split on <sheetData> to get before/after portions
    let (before, after) = if let Some(pos) = xml.find("<sheetData>") {
        (&xml[..pos], &xml[pos..])
    } else {
        (xml.as_ref(), "")
    };
    let xml_after_sheet_data = if let Some(end) = after.find("</sheetData>") {
        after[end + "</sheetData>".len()..].to_owned()
    } else {
        String::new()
    };

    // Parse rows
    let rows = parse_rows(xml, shared_strings);

    // Parse merge cells
    let merge_cells = parse_merge_cells(xml);

    SheetAst {
        name: name.to_owned(),
        sheet_path: path.to_owned(),
        rows,
        merge_cells,
        xml_before_sheet_data: before.to_owned(),
        xml_after_sheet_data,
        full_xml: xml.to_owned(),
    }
}

/// Parse all <row> elements from sheet XML.
fn parse_rows(xml: &str, shared_strings: &[String]) -> Vec<RowAst> {
    let mut rows = Vec::new();
    // Find <sheetData>...</sheetData> section
    let sheet_data = if let Some(start) = xml.find("<sheetData>") {
        let start = start + "<sheetData>".len();
        if let Some(end) = xml[start..].find("</sheetData>") {
            &xml[start..start + end]
        } else {
            return rows;
        }
    } else {
        return rows;
    };

    for segment in sheet_data.split("<row").skip(1) {
        let Some(end_pos) = segment.find('>') else {
            continue;
        };
        let header_part = &segment[..end_pos];
        let row_content = &segment[end_pos + 1..];

        let row_num = extract_attr(header_part, "r")
            .and_then(|v| v.parse::<u32>().ok())
            .unwrap_or(0);

        let raw_xml_header = format!("<row{}", header_part);
        let cells = parse_cells(row_content, row_num, shared_strings);

        rows.push(RowAst {
            row_num,
            cells,
            raw_xml_header,
        });
    }

    rows
}

/// Parse all <c> elements from a row's content.
fn parse_cells(row_xml: &str, row_num: u32, shared_strings: &[String]) -> Vec<CellAst> {
    let mut cells = Vec::new();

    for segment in row_xml.split("<c").skip(1) {
        let Some(end_tag) = segment.find('>') else {
            continue;
        };
        let attrs = &segment[..end_tag];
        let inner = &segment[end_tag + 1..];

        let ref_str = extract_attr(attrs, "r").unwrap_or_default();
        let (col, _) = parse_cell_ref(&ref_str);
        let cell_type = extract_attr(attrs, "t");
        let style_index = extract_attr(attrs, "s").and_then(|v| v.parse::<u32>().ok());

        // Extract <v> value
        let raw_value = extract_tag_value(inner, "v");

        // Resolve the display value for shared strings
        let _display_value = match cell_type.as_deref() {
            Some("s") => raw_value
                .as_ref()
                .and_then(|v| v.parse::<usize>().ok())
                .and_then(|idx| shared_strings.get(idx))
                .map(|s| s.to_owned()),
            Some("inlineStr") => extract_tag_value(inner, "t"),
            _ => raw_value.clone(),
        };

        let raw_xml = format!("<c{}</c>", segment);

        cells.push(CellAst {
            col,
            row: row_num,
            ref_str,
            cell_type,
            raw_value,
            raw_xml,
            style_index,
        });
    }

    cells.sort_by_key(|c| c.col);
    cells
}

/// Parse <mergeCells> from sheet XML.
fn parse_merge_cells(xml: &str) -> Vec<MergeCell> {
    let mut merges = Vec::new();

    let Some(mc_start) = xml.find("<mergeCells") else {
        return merges;
    };
    let mc_section = &xml[mc_start..];
    let Some(mc_end) = mc_section.find("</mergeCells>") else {
        return merges;
    };
    let mc_section = &mc_section[..mc_end];

    for segment in mc_section.split("<mergeCell").skip(1) {
        let segment = segment.split('>').next().unwrap_or("");
        let ref_str = extract_attr(segment, "ref").unwrap_or_default();
        if let Some(mc) = parse_merge_ref(&ref_str) {
            merges.push(mc);
        }
    }

    merges
}

/// Parse a merge reference like "A1:C3" into a MergeCell.
fn parse_merge_ref(ref_str: &str) -> Option<MergeCell> {
    let parts: Vec<&str> = ref_str.split(':').collect();
    if parts.len() != 2 {
        return None;
    }
    let (start_col, start_row) = parse_cell_ref(parts[0]);
    let (end_col, end_row) = parse_cell_ref(parts[1]);
    Some(MergeCell {
        ref_str: ref_str.to_owned(),
        start_row,
        end_row,
        start_col,
        end_col,
    })
}

/// Parse shared strings from xl/sharedStrings.xml.
pub fn parse_shared_strings(xml: &str) -> Vec<String> {
    let mut values = Vec::new();
    for item in xml.split("<si").skip(1) {
        let segment = item.split("</si>").next().unwrap_or("");
        let mut text = String::new();
        for part in segment.split("<t").skip(1) {
            if let Some(after) = part.split('>').nth(1) {
                if let Some(value) = after.split("</t>").next() {
                    text.push_str(&xml_unescape(value));
                }
            }
        }
        values.push(text);
    }
    values
}

/// Extract a cell reference (e.g. "B3") and return (col_0based, row_1based).
pub fn parse_cell_ref(ref_str: &str) -> (usize, u32) {
    let mut col_letters = String::new();
    let mut row_digits = String::new();
    for ch in ref_str.chars() {
        if ch.is_ascii_alphabetic() {
            col_letters.push(ch);
        } else if ch.is_ascii_digit() {
            row_digits.push(ch);
        }
    }
    let col = col_letter_to_index(&col_letters);
    let row = row_digits.parse::<u32>().unwrap_or(0);
    (col, row)
}

/// Convert column letters to 0-based index: "A" -> 0, "B" -> 1, "AA" -> 26.
pub fn col_letter_to_index(letters: &str) -> usize {
    let mut result = 0usize;
    for ch in letters.to_ascii_uppercase().chars() {
        result = result * 26 + (ch as usize - b'A' as usize + 1);
    }
    result.saturating_sub(1)
}

/// Extract an XML attribute value from a tag segment.
fn extract_attr(segment: &str, attr_name: &str) -> Option<String> {
    // Try with =" pattern
    let patterns = [format!("{attr_name}=\""), format!("{attr_name} = \"")];
    for pattern in &patterns {
        if let Some(start) = segment.find(pattern.as_str()) {
            let value_start = start + pattern.len();
            if let Some(end) = segment[value_start..].find('"') {
                return Some(segment[value_start..value_start + end].to_owned());
            }
        }
    }
    None
}

/// Extract the text content of an XML tag like <v>...</v>.
fn extract_tag_value(xml: &str, tag: &str) -> Option<String> {
    let open = format!("<{tag}>");
    let close = format!("</{tag}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)?;
    Some(xml_unescape(&xml[start..start + end]))
}

fn xml_unescape(s: &str) -> String {
    s.replace("&amp;", "&")
        .replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
}

fn read_entry(
    archive: &mut zip::ZipArchive<std::fs::File>,
    name: &str,
) -> Option<String> {
    let mut f = archive.by_name(name).ok()?;
    let mut s = String::new();
    f.read_to_string(&mut s).ok()?;
    Some(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_col_letter_to_index() {
        assert_eq!(col_letter_to_index("A"), 0);
        assert_eq!(col_letter_to_index("B"), 1);
        assert_eq!(col_letter_to_index("Z"), 25);
        assert_eq!(col_letter_to_index("AA"), 26);
        assert_eq!(col_letter_to_index("AZ"), 51);
        assert_eq!(col_letter_to_index("BA"), 52);
    }

    #[test]
    fn test_parse_cell_ref() {
        assert_eq!(parse_cell_ref("A1"), (0, 1));
        assert_eq!(parse_cell_ref("B3"), (1, 3));
        assert_eq!(parse_cell_ref("AA10"), (26, 10));
    }

    #[test]
    fn test_parse_shared_strings() {
        let xml = r#"<sst><si><t>hello</t></si><si><t>world</t></si></sst>"#;
        let strings = parse_shared_strings(xml);
        assert_eq!(strings, vec!["hello", "world"]);
    }

    #[test]
    fn test_extract_attr() {
        let seg = r#"r="B3" t="s""#;
        assert_eq!(extract_attr(seg, "r"), Some("B3".into()));
        assert_eq!(extract_attr(seg, "t"), Some("s".into()));
        assert_eq!(extract_attr(seg, "x"), None);
    }
}
