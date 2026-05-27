use super::ast::{CellAst, MergeCell};
use super::strings::SharedStringPool;

/// A cell value to be written into the output.
#[derive(Debug, Clone)]
pub enum CellValue {
    /// String value — will be added to the shared string pool during write.
    String(String),
    /// Numeric value — written directly as `<v>N</v>`.
    Number(f64),
    /// Empty cell that keeps row/cell style but clears any template sample value.
    Blank,
    /// Preserve the original cell as-is (for header/static rows).
    Preserve(CellAst),
}

/// A single cell in the IR.
#[derive(Debug, Clone)]
pub struct CellIR {
    pub col: usize,
    pub row: u32,
    pub value: CellValue,
    /// Style index inherited from the template row.
    pub style_index: Option<u32>,
    /// Formula text (if this cell has a formula).
    pub formula: Option<String>,
}

/// A row in the IR.
#[derive(Debug, Clone)]
pub struct RowIR {
    pub row_num: u32,
    pub cells: Vec<CellIR>,
    /// True if this is a cloned data row (not a template-preserved row).
    pub is_data: bool,
    /// The raw <row ...> opening tag from the source template row (for style inheritance).
    pub template_row_header: Option<String>,
    /// The complete raw row XML for static rows (preserved byte-identical).
    pub raw_row_xml: Option<String>,
}

/// A sheet in the IR.
#[derive(Debug, Clone)]
pub struct SheetIR {
    pub sheet_index: usize,
    pub rows: Vec<RowIR>,
    pub merge_cells: Vec<MergeCell>,
    pub xml_before_sheet_data: String,
    pub xml_after_sheet_data: String,
}

/// The intermediate representation of the entire workbook after data binding.
#[derive(Debug, Clone)]
pub struct SpreadsheetIR {
    pub sheets: Vec<SheetIR>,
    pub shared_strings: SharedStringPool,
    /// All other ZIP entries to pass through unchanged.
    pub passthrough_entries: Vec<(String, Vec<u8>)>,
}
