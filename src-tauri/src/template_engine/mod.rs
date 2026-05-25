pub mod ast;
pub mod binder;
pub mod cloner;
pub mod ir;
pub mod parser;
pub mod region;
pub mod strings;
pub mod writer;

#[derive(Debug, thiserror::Error)]
pub enum TemplateError {
    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),
    #[error("ZIP error: {0}")]
    Zip(String),
    #[error("parse error: {0}")]
    Parse(String),
    #[error("binding error: {0}")]
    Bind(String),
    #[error("write error: {0}")]
    Write(String),
    #[error("no data region found in template")]
    NoDataRegion,
}

#[derive(Debug, Clone)]
pub struct ExportResult {
    pub file_path: String,
    pub row_count: usize,
    pub byte_size: u64,
}

/// The main entry point. Orchestrates: parse → recognize → bind → write.
pub struct TemplateEngine;

impl TemplateEngine {
    /// Full pipeline: read template XLSX, bind data, write output XLSX.
    ///
    /// 1. Copy template to output path
    /// 2. Parse the copied XLSX into AST
    /// 3. Recognize data regions using the provided label matcher
    /// 4. Bind data from the DataSource into IR
    /// 5. Write IR back to the XLSX file
    pub fn export(
        template_path: &str,
        output_path: &str,
        source: &dyn binder::DataSource,
        label_matcher: &dyn Fn(&[String]) -> Vec<(usize, String)>,
    ) -> Result<ExportResult, TemplateError> {
        // 1. Copy template to output
        std::fs::copy(template_path, output_path)?;

        // 2. Parse
        let ast = parser::parse_xlsx(output_path)?;

        // 3. Recognize regions
        let regions = region::recognize_regions(&ast, label_matcher);
        if regions
            .iter()
            .all(|r| r.kind != region::RegionKind::DataAppend)
        {
            return Err(TemplateError::NoDataRegion);
        }

        // 4. Bind
        let mut ir = binder::bind(&ast, &regions, source)?;

        // 5. Write
        writer::write_xlsx(output_path, &mut ir)?;

        let byte_size = std::fs::metadata(output_path)?.len();
        Ok(ExportResult {
            file_path: output_path.to_owned(),
            row_count: source.row_count(),
            byte_size,
        })
    }
}
