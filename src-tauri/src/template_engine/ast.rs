/// A parsed cell in the template, preserving all XML attributes.
#[derive(Debug, Clone)]
pub struct CellAst {
    /// Column index (0-based, computed from cell reference like "B3" -> 1)
    pub col: usize,
    /// Row number (1-based, as in Excel)
    pub row: u32,
    /// Cell reference string, e.g. "B3"
    pub ref_str: String,
    /// Cell type attribute: None=number, Some("s")=shared string,
    /// Some("inlineStr"), Some("b")=boolean, Some("str")=formula string
    pub cell_type: Option<String>,
    /// The raw <v> value text (before dereferencing shared strings)
    pub raw_value: Option<String>,
    /// The raw cell XML <c ...>...</c> for style preservation
    pub raw_xml: String,
    /// Style index (the "s" attribute on <c>)
    pub style_index: Option<u32>,
}

/// A parsed row in the template.
#[derive(Debug, Clone)]
pub struct RowAst {
    /// Row number (1-based)
    pub row_num: u32,
    /// Cells sorted by column index
    pub cells: Vec<CellAst>,
    /// The raw row opening tag: <row r="..." spans="..." ht="...">
    pub raw_xml_header: String,
}

/// A parsed merge cell entry.
#[derive(Debug, Clone)]
pub struct MergeCell {
    pub ref_str: String,   // e.g. "A1:C1"
    pub start_row: u32,
    pub end_row: u32,
    pub start_col: usize,
    pub end_col: usize,
}

/// A parsed sheet from the XLSX.
#[derive(Debug, Clone)]
pub struct SheetAst {
    pub name: String,
    pub sheet_path: String,     // e.g. "xl/worksheets/sheet1.xml"
    pub rows: Vec<RowAst>,
    pub merge_cells: Vec<MergeCell>,
    /// Raw XML before <sheetData>
    pub xml_before_sheet_data: String,
    /// Raw XML after </sheetData>
    pub xml_after_sheet_data: String,
    /// Full raw sheet XML
    pub full_xml: String,
}

/// The entire parsed template workbook.
#[derive(Debug, Clone)]
pub struct TemplateAst {
    pub sheets: Vec<SheetAst>,
    pub shared_strings: Vec<String>,
    /// Other ZIP entries to pass through (styles.xml, theme, content_types, etc.)
    pub other_entries: Vec<(String, Vec<u8>)>,
}
