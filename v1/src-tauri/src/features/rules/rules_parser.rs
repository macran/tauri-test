use std::path::Path;

use calamine::{open_workbook, Data, DataType, Reader, Xlsx};

use super::rules_models::{
    normalize_review, RuleImportError, NormalizedRule, RuleInput,
};

pub(crate) struct ParsedRule {
    pub(crate) excel_row: u32,
    pub(crate) rule: NormalizedRule,
}

pub(crate) struct ParseResult {
    pub(crate) rows: Vec<ParsedRule>,
    pub(crate) errors: Vec<RuleImportError>,
}

fn cell_text(row: &[Data], column: usize) -> String {
    row.get(column)
        .and_then(DataType::as_string)
        .unwrap_or_default()
        .trim()
        .to_string()
}

pub(crate) fn parse_rules_excel(
    path: &Path,
) -> Result<ParseResult, String> {
    let mut workbook: Xlsx<_> = open_workbook(path)
        .map_err(|error| format!("无法打开 Excel 文件：{error}"))?;

    let range = workbook
        .worksheet_range("Sheet1")
        .map_err(|error| format!("无法读取 Sheet1：{error}"))?;

    let mut rows = Vec::new();
    let mut errors = Vec::new();

    // 第 1、2 行是说明和标题。
    // 第 3 行对应 zero-based index 2。
    for (zero_based_index, row) in range.rows().enumerate().skip(2) {
        let excel_row = u32::try_from(zero_based_index + 1)
            .map_err(|_| "Excel 行号超过 u32 可表示范围".to_string())?;

        let is_empty = (0..11)
            .all(|column| cell_text(row, column).is_empty());

        if is_empty {
            continue;
        }

        let needs_manual_review =
            match normalize_review(&cell_text(row, 9)) {
                Ok(value) => value,
                Err(message) => {
                    errors.push(RuleImportError {
                        excel_row,
                        message,
                    });

                    continue;
                }
            };

        let input = RuleInput {
            scope: cell_text(row, 0),
            rule_category: cell_text(row, 1),

            seller_tax_id_match_type: Some(cell_text(row, 2)),
            seller_tax_id_values: Some(cell_text(row, 3)),

            // E 列销售方名称只用于 Excel 中人工核对。
            // 不进入数据库，也不进入 RuleInput。

            tax_code_match_type: Some(cell_text(row, 5)),
            tax_code_values: Some(cell_text(row, 6)),

            tax_name_match_type: Some(cell_text(row, 7)),
            tax_name_values: Some(cell_text(row, 8)),

            needs_manual_review,
            note: cell_text(row, 10),
        };

        match input.normalize() {
            Ok(rule) => {
                rows.push(ParsedRule {
                    excel_row,
                    rule,
                });
            }
            Err(message) => {
                errors.push(RuleImportError {
                    excel_row,
                    message,
                });
            }
        }
    }



    Ok(ParseResult { rows, errors })
    

}