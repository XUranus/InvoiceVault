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
    #[error("template export failed at {stage}: {message}")]
    Trace {
        stage: &'static str,
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct ExportResult {
    pub file_path: String,
    pub row_count: usize,
    pub byte_size: u64,
}

/// The main entry point. Orchestrates: parse → recognize → bind → write.
pub struct TemplateEngine;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::template_engine::binder::{DataSource, DataValue};
    use std::io::Read;

    struct MockDataSource {
        rows: Vec<Vec<(String, DataValue)>>,
    }

    impl MockDataSource {
        fn new(rows: Vec<Vec<(String, DataValue)>>) -> Self {
            Self { rows }
        }
    }

    impl DataSource for MockDataSource {
        fn row_count(&self) -> usize {
            self.rows.len()
        }

        fn cell_value(&self, row_index: usize, col_key: &str) -> Option<DataValue> {
            self.rows.get(row_index).and_then(|row| {
                row.iter()
                    .find(|(k, _)| k == col_key)
                    .map(|(_, v)| v.clone())
            })
        }
    }

    /// Test with the actual template file if available.
    #[test]
    fn test_template_export_with_real_file() {
        let template_path = "/home/xuranus/Documents/xwechat_files/wxid_ae1nz60lyw3r11_9dac/msg/file/2026-05/普票移交登记表.xlsx";
        if !std::path::Path::new(template_path).exists() {
            eprintln!("Skipping test: template file not found");
            return;
        }

        let output_path = "/tmp/invoicevault_template_export_test.xlsx";

        // Label matcher that matches the template headers
        let label_matcher = |labels: &[String]| -> Vec<(usize, String)> {
            let mapping = [
                ("开具时间", "issue_date"),
                ("发票代码", "invoice_code"),
                ("发票号码", "invoice_number"),
                ("开票内容", "remarks"),
                ("销售方", "seller_name"),
                ("价税合计", "total_amount"),
                ("金额", "amount_without_tax"),
                ("税额", "tax_amount"),
            ];
            let mut result = Vec::new();
            for (i, label) in labels.iter().enumerate() {
                for (cn, key) in &mapping {
                    if label.trim() == *cn {
                        result.push((i, key.to_string()));
                    }
                }
            }
            result
        };

        // Mock data: 3 invoices
        let source = MockDataSource::new(vec![
            vec![
                ("issue_date".into(), DataValue::String("2024-01-15".into())),
                (
                    "invoice_code".into(),
                    DataValue::String("044001900111".into()),
                ),
                (
                    "invoice_number".into(),
                    DataValue::String("12345678".into()),
                ),
                ("remarks".into(), DataValue::String("建筑材料".into())),
                (
                    "seller_name".into(),
                    DataValue::String("测试供应商A".into()),
                ),
                ("total_amount".into(), DataValue::Number(1130.0)),
                ("amount_without_tax".into(), DataValue::Number(1000.0)),
                ("tax_amount".into(), DataValue::Number(130.0)),
            ],
            vec![
                ("issue_date".into(), DataValue::String("2024-02-20".into())),
                (
                    "invoice_code".into(),
                    DataValue::String("044001900222".into()),
                ),
                (
                    "invoice_number".into(),
                    DataValue::String("87654321".into()),
                ),
                ("remarks".into(), DataValue::String("五金材料".into())),
                (
                    "seller_name".into(),
                    DataValue::String("测试供应商B".into()),
                ),
                ("total_amount".into(), DataValue::Number(565.0)),
                ("amount_without_tax".into(), DataValue::Number(500.0)),
                ("tax_amount".into(), DataValue::Number(65.0)),
            ],
            vec![
                ("issue_date".into(), DataValue::String("2024-03-10".into())),
                (
                    "invoice_code".into(),
                    DataValue::String("044001900333".into()),
                ),
                (
                    "invoice_number".into(),
                    DataValue::String("11223344".into()),
                ),
                ("remarks".into(), DataValue::String("水泥".into())),
                (
                    "seller_name".into(),
                    DataValue::String("测试供应商C".into()),
                ),
                ("total_amount".into(), DataValue::Number(2260.0)),
                ("amount_without_tax".into(), DataValue::Number(2000.0)),
                ("tax_amount".into(), DataValue::Number(260.0)),
            ],
        ]);

        let result = TemplateEngine::export(template_path, output_path, &source, &label_matcher);
        assert!(result.is_ok(), "Export failed: {:?}", result.err());
        let result = result.unwrap();
        assert_eq!(result.row_count, 3);
        assert!(result.byte_size > 0);

        // Verify the output file exists and is a valid XLSX
        assert!(std::path::Path::new(output_path).exists());

        // Parse the output and verify structure
        let ast = parser::parse_xlsx(output_path).expect("Failed to parse output");
        let sheet = &ast.sheets[0];

        let sheet_xml = read_xlsx_entry(output_path, "xl/worksheets/sheet1.xml");

        // Should keep the original template footprint: 3 title rows + 1 header
        // + 21 formatted data slots + 2 summary rows + spacer/footer rows.
        assert!(
            sheet_xml.contains(r#"<dimension ref="A1:L30""#),
            "Sheet dimension should still cover A1:L30"
        );
        assert_eq!(
            sheet_xml.matches("<mergeCells").count(),
            1,
            "Output should contain one mergeCells block"
        );
        assert!(
            sheet_xml.contains(r#"<mergeCells count="9">"#),
            "Output should preserve all 9 merge ranges"
        );
        assert!(
            sheet_xml.contains(r#"<row r="26" customFormat="false" ht="21.75""#),
            "Summary row should preserve row formatting attributes"
        );

        // Verify row 5 has sequence number 1
        let row5 = sheet.rows.iter().find(|r| r.row_num == 5);
        assert!(row5.is_some(), "Row 5 (first data row) should exist");
        let row5 = row5.unwrap();
        let a5 = row5.cells.iter().find(|c| c.col == 0);
        assert!(a5.is_some(), "Cell A5 should exist");
        assert_eq!(
            a5.unwrap().raw_value.as_deref(),
            Some("1"),
            "A5 should be sequence number 1"
        );

        // Verify row 6 has sequence number 2
        let row6 = sheet.rows.iter().find(|r| r.row_num == 6);
        assert!(row6.is_some(), "Row 6 (second data row) should exist");
        let row6 = row6.unwrap();
        let a6 = row6.cells.iter().find(|c| c.col == 0);
        assert!(a6.is_some(), "Cell A6 should exist");
        assert_eq!(
            a6.unwrap().raw_value.as_deref(),
            Some("2"),
            "A6 should be sequence number 2"
        );

        // Verify row 7 has sequence number 3
        let row7 = sheet.rows.iter().find(|r| r.row_num == 7);
        assert!(row7.is_some(), "Row 7 (third data row) should exist");
        let row7 = row7.unwrap();
        let a7 = row7.cells.iter().find(|c| c.col == 0);
        assert!(a7.is_some(), "Cell A7 should exist");
        assert_eq!(
            a7.unwrap().raw_value.as_deref(),
            Some("3"),
            "A7 should be sequence number 3"
        );

        // Verify row 8 is a blank placeholder row, not leaked sample data.
        let row8 = sheet.rows.iter().find(|r| r.row_num == 8);
        assert!(row8.is_some(), "Row 8 placeholder should still exist");
        let row8 = row8.unwrap();
        assert!(
            row8.cells
                .iter()
                .all(|c| c.raw_value.is_none() && !c.raw_xml.contains("<f")),
            "Row 8 should keep styles but clear sample values and formulas"
        );

        // Verify summary rows stay at the original template rows.
        let summary_row = sheet.rows.iter().find(|r| {
            r.cells.iter().any(|c| {
                if c.cell_type.as_deref() == Some("s") {
                    if let Some(ref v) = c.raw_value {
                        if let Ok(idx) = v.parse::<usize>() {
                            return ast
                                .shared_strings
                                .get(idx)
                                .map(|s| s.contains("小 计"))
                                .unwrap_or(false);
                        }
                    }
                }
                false
            })
        });
        assert!(
            summary_row.is_some(),
            "Summary row '小 计' should exist in output"
        );
        let summary_row_num = summary_row.unwrap().row_num;
        assert_eq!(summary_row_num, 26, "Summary row should remain at row 26");

        // Verify "合 计" row exists
        let total_row = sheet.rows.iter().find(|r| {
            r.cells.iter().any(|c| {
                if c.cell_type.as_deref() == Some("s") {
                    if let Some(ref v) = c.raw_value {
                        if let Ok(idx) = v.parse::<usize>() {
                            return ast
                                .shared_strings
                                .get(idx)
                                .map(|s| s.contains("合 计"))
                                .unwrap_or(false);
                        }
                    }
                }
                false
            })
        });
        assert!(
            total_row.is_some(),
            "Total row '合 计' should exist in output"
        );
        assert_eq!(
            total_row.unwrap().row_num,
            27,
            "Total row should remain at row 27"
        );

        for (cell_ref, formula, cached_value) in [
            ("H26", "SUM(H5:H25)", "3955"),
            ("I26", "SUM(I5:I25)", "3500"),
            ("K26", "SUM(K5:K25)", "455"),
            ("H27", "SUM(H5:H25)", "3955"),
            ("I27", "SUM(I5:I25)", "3500"),
            ("K27", "SUM(K5:K25)", "455"),
        ] {
            let cell_xml = cell_xml(&sheet_xml, cell_ref);
            assert!(
                cell_xml.contains(&format!("<f>{formula}</f>")),
                "{cell_ref} should contain formula {formula}"
            );
            assert!(
                cell_xml.contains(&format!("<v>{cached_value}</v>")),
                "{cell_ref} should contain cached value {cached_value}"
            );
        }

        let footer_row = sheet.rows.iter().find(|r| r.row_num == 29);
        assert!(footer_row.is_some(), "Footer/signature row 29 should exist");
        let footer_row = footer_row.unwrap();
        for expected in ["项目负责人", "移交人", "接收人", "日期"] {
            assert!(
                row_contains_shared_text(&ast, footer_row, expected),
                "Footer row should preserve {expected}"
            );
        }

        // Verify K5 has the tax_amount value (130 = 1130 - 1000)
        let k5 = row5.cells.iter().find(|c| c.col == 10); // K = col 10
        assert!(k5.is_some(), "Cell K5 should exist");
        let k5 = k5.unwrap();
        // K is mapped to tax_amount, so the value comes from the data source
        assert!(
            k5.cell_type.is_none() || k5.cell_type.as_deref() == Some("n"),
            "K5 should be numeric"
        );
        let k5_val: f64 = k5.raw_value.as_ref().unwrap().parse().unwrap();
        assert!(
            (k5_val - 130.0).abs() < 0.01,
            "K5 should be ~130, got {}",
            k5_val
        );

        // Cleanup (skip for manual inspection)
        // let _ = std::fs::remove_file(output_path);

        println!(
            "✓ Template export test passed: 3 rows, summary at row {}",
            summary_row_num
        );
    }

    fn read_xlsx_entry(path: &str, entry_name: &str) -> String {
        let file = std::fs::File::open(path).expect("open output xlsx");
        let mut archive = zip::ZipArchive::new(file).expect("open output zip");
        let mut entry = archive.by_name(entry_name).expect("read xlsx entry");
        let mut xml = String::new();
        entry.read_to_string(&mut xml).expect("read entry XML");
        xml
    }

    fn cell_xml<'a>(sheet_xml: &'a str, cell_ref: &str) -> &'a str {
        let marker = format!(r#"<c r="{cell_ref}""#);
        let start = sheet_xml
            .find(&marker)
            .unwrap_or_else(|| panic!("{cell_ref} should exist in sheet XML"));
        let after = &sheet_xml[start..];
        let end = after
            .find("</c>")
            .map(|idx| idx + "</c>".len())
            .or_else(|| after.find("/>").map(|idx| idx + 2))
            .expect("cell XML should be closed");
        &after[..end]
    }

    fn row_contains_shared_text(
        ast: &crate::template_engine::ast::TemplateAst,
        row: &crate::template_engine::ast::RowAst,
        text: &str,
    ) -> bool {
        row.cells.iter().any(|cell| {
            if cell.cell_type.as_deref() != Some("s") {
                return false;
            }
            let Some(raw_value) = cell.raw_value.as_deref() else {
                return false;
            };
            let Ok(index) = raw_value.parse::<usize>() else {
                return false;
            };
            ast.shared_strings
                .get(index)
                .is_some_and(|value| value.contains(text))
        })
    }
}

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
        std::fs::copy(template_path, output_path)
            .map_err(|e| trace_export_error("copy_template", template_path, output_path, e))?;

        // 2. Parse
        let ast = parser::parse_xlsx(output_path)
            .map_err(|e| trace_export_error("parse_xlsx", template_path, output_path, e))?;

        // 3. Recognize regions
        let regions = region::recognize_regions(&ast, label_matcher);
        if regions
            .iter()
            .all(|r| r.kind != region::RegionKind::DataAppend)
        {
            return Err(trace_export_error(
                "recognize_regions",
                template_path,
                output_path,
                TemplateError::NoDataRegion,
            ));
        }

        // 4. Bind
        let mut ir = binder::bind(&ast, &regions, source)
            .map_err(|e| trace_export_error("bind_data", template_path, output_path, e))?;

        // 5. Write
        writer::write_xlsx(output_path, &mut ir)
            .map_err(|e| trace_export_error("write_xlsx", template_path, output_path, e))?;

        let byte_size = std::fs::metadata(output_path)
            .map_err(|e| trace_export_error("stat_output", template_path, output_path, e))?
            .len();
        Ok(ExportResult {
            file_path: output_path.to_owned(),
            row_count: source.row_count(),
            byte_size,
        })
    }
}

fn trace_export_error(
    stage: &'static str,
    template_path: &str,
    output_path: &str,
    error: impl std::fmt::Display,
) -> TemplateError {
    TemplateError::Trace {
        stage,
        message: format!("template_path={template_path}, output_path={output_path}, error={error}"),
    }
}
