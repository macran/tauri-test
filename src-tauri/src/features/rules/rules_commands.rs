use std::path::Path;

use tauri::State;

use crate::DbState;

use super::{
    rules_models::{
      
        RuleImportError,
        RuleImportSummary,
        RuleInput,
        StampTaxCategoryOption
       
    },
    rules_parser::parse_rules_excel,
    rules_repository,
};

#[tauri::command]
#[specta::specta]
pub fn import_tax_rules(
    path: String,
    state: State<'_, DbState>,
) -> Result<RuleImportSummary, String> {
    let parsed = parse_rules_excel(Path::new(&path))?;

    let total_rows = u32::try_from(
        parsed
            .rows
            .len()
            .checked_add(parsed.errors.len())
            .ok_or_else(|| "导入行数计算溢出".to_string())?,
    )
    .map_err(|_| "导入行数超过 u32 可表示范围".to_string())?;

    let invalid_rows = u32::try_from(parsed.errors.len())
        .map_err(|_| "错误行数超过 u32 可表示范围".to_string())?;

    // Excel 解析阶段发现错误时，不开启数据库事务，也不写入任何规则。
    if !parsed.errors.is_empty() {
        return Ok(RuleImportSummary {
            total_rows,
            inserted: 0,
            duplicates: 0,
            invalid_rows,
            errors: parsed.errors,
        });
    }

    let mut conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;

    let tx = conn
        .transaction()
        .map_err(|error| format!("开始规则导入事务失败：{error}"))?;

   


    let mut rows = parsed.rows;

    
    let mut category_errors: Vec<RuleImportError> = Vec::new();

    // 校验印花税规则对应的 stamp_tax_categories。
   for parsed_row in &mut rows {
    match rules_repository::resolve_stamp_tax_category(
        &tx,
        &mut parsed_row.rule,
    ) {
        Ok(()) => {
           
        }
        Err(message) => {
           

            category_errors.push(RuleImportError {
                excel_row: parsed_row.excel_row,
                message,
            });
        }
    }
}

    if !category_errors.is_empty() {
        eprintln!(
        "[rules] 因类别校验错误提前返回，不执行 INSERT；错误数={}",
        category_errors.len()
    );
        let category_invalid_rows =
            u32::try_from(category_errors.len())
                .map_err(|_| {
                    "印花税类别错误行数超过 u32 可表示范围"
                        .to_string()
                })?;

        // tx 在这里离开作用域时会自动回滚。
        // 本批次不会产生部分写入。
        return Ok(RuleImportSummary {
            total_rows,
            inserted: 0,
            duplicates: 0,
            invalid_rows: category_invalid_rows,
            errors: category_errors,
        });
    }

    let mut inserted: u32 = 0;
    let mut duplicates: u32 = 0;

    for parsed_row in &mut rows {
            
        match rules_repository::insert_rule(
            &tx,
            &mut parsed_row.rule,
            "excel",
        )? {
            Some(_) => {
                inserted += 1;
            }
            None => {
                duplicates += 1;
            }
        }
    }

    tx.commit()
        .map_err(|error| format!("提交规则导入事务失败：{error}"))?;

    Ok(RuleImportSummary {
        total_rows,
        inserted,
        duplicates,
        invalid_rows: 0,
        errors: Vec::new(),
    })
}

#[tauri::command]
#[specta::specta]
pub fn add_tax_match_rule(
    input: RuleInput,
    state: State<'_, DbState>,
) -> Result<super::rules_models::AddTaxMatchRuleResult, String> {
    let mut rule = input.normalize()?;

    let mut conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;

    let tx = conn
        .transaction()
        .map_err(|error| format!("开始新增规则事务失败：{error}"))?;

    let Some(id) = rules_repository::insert_rule(
        &tx,
        &mut rule,
        "manual",
    )? else {
        return Ok(
            super::rules_models::AddTaxMatchRuleResult::Duplicate {
                message: "已经存在一条内容一致且正在启用的规则".to_string(),
            },
        );
    };

    tx.commit()
        .map_err(|error| format!("提交新增规则事务失败：{error}"))?;

    let saved_rule = rules_repository::get_rule(&conn, id)?
        .ok_or_else(|| "新增成功但读取规则失败".to_string())?;

    Ok(
        super::rules_models::AddTaxMatchRuleResult::Inserted {
            rule: saved_rule,
        },
    )
}

#[tauri::command]
#[specta::specta]
pub fn disable_tax_match_rule(
    id: u32,
    state: State<'_, crate::DbState>,
) -> Result<(), String> {
    let conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;

    rules_repository::disable_rule(&conn, i64::from(id))
}

#[tauri::command]
#[specta::specta]
pub fn list_tax_match_rules(
    include_disabled: bool,
    state: State<'_, DbState>,
) -> Result<Vec<super::rules_models::TaxMatchRuleRecord>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;
    rules_repository::list_rules(&conn, include_disabled)
}

#[tauri::command]
#[specta::specta]
pub fn list_stamp_tax_categories(
    state: State<'_, DbState>,
) -> Result<Vec<StampTaxCategoryOption>, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;

    rules_repository::list_stamp_tax_categories(&conn)
}
/// 导出规则为 Excel，格式与导入模板一致（Sheet1，第 1 行说明、第 2 行标题、第 3 行起数据），
/// 导出的文件可直接重新导入。默认只导出启用的规则。返回导出的规则条数。
#[tauri::command]
#[specta::specta]
pub fn export_tax_rules(
    include_disabled: bool,
    path: String,
    state: State<'_, DbState>,
) -> Result<u32, String> {
    let conn = state
        .conn
        .lock()
        .map_err(|_| "数据库连接锁定失败".to_string())?;
    let rules = rules_repository::list_rules(&conn, include_disabled)?;

    // E 列销售方名称仅供人工核对：用销售方税号查 sellers（只有“等于”单个税号时才有意义）
    let seller_name = |rule: &super::rules_models::TaxMatchRuleRecord| -> String {
        if rule.seller_tax_id_match_type.as_deref() != Some("eq") {
            return String::new();
        }
        let Some(tax_no) = rule.seller_tax_id_values.as_deref() else {
            return String::new();
        };
        conn.query_row("SELECT name FROM sellers WHERE tax_no = ?1", [tax_no], |r| r.get(0))
            .unwrap_or_default()
    };
    let match_text = |t: &Option<String>| match t.as_deref() {
        Some("eq") => "等于",
        Some("neq") => "不等于",
        _ => "",
    };

    let mut rows: Vec<[String; 12]> = Vec::with_capacity(rules.len());
    for r in &rules {
        rows.push([
            if r.scope == "stamp_tax" { "印花税计算" } else { "进项税转出" }.to_string(),
            r.rule_category.clone(),
            match_text(&r.seller_tax_id_match_type).to_string(),
            r.seller_tax_id_values.clone().unwrap_or_default(),
            seller_name(r),
            match_text(&r.tax_code_match_type).to_string(),
            r.tax_code_values.clone().unwrap_or_default(),
            match_text(&r.tax_name_match_type).to_string(),
            r.tax_name_values.clone().unwrap_or_default(),
            if r.needs_manual_review { "是" } else { "否" }.to_string(),
            r.note.clone(),
            if r.enabled { "启用" } else { "已禁用" }.to_string(),
        ]);
    }

    write_rules_xlsx(&path, &rows)?;
    u32::try_from(rows.len()).map_err(|_| "规则条数超过 u32 可表示范围".to_string())
}

// 第 12 列“状态”只读：导入只读前 11 列，会忽略它
const RULE_EXPORT_HEADERS: [&str; 12] = [
    "规则所属", "规则类别", "销售方税号匹配方式", "销售方税号", "销售方名称（仅核对，不导入）",
    "税收分类编码匹配方式", "税收分类编码", "税收分类编码简称匹配方式", "税收分类编码简称",
    "是否核对", "备注", "状态（仅查看，不导入）",
];

fn write_rules_xlsx(path: &str, rows: &[[String; 12]]) -> Result<(), String> {
    use rust_xlsxwriter::{Format, Workbook};

    if !path.to_ascii_lowercase().ends_with(".xlsx") {
        return Err("导出文件必须是 .xlsx".into());
    }
    let err = |e: rust_xlsxwriter::XlsxError| format!("写入 Excel 失败：{e}");
    let head = Format::new().set_bold();

    let mut wb = Workbook::new();
    let ws = wb.add_worksheet();
    ws.set_name("Sheet1").map_err(err)?;
    ws.write_string(0, 0, "由规则管理导出；可直接修改后重新导入。第 3 行起为规则数据，勿改动前两行。")
        .map_err(err)?;
    for (c, title) in RULE_EXPORT_HEADERS.iter().enumerate() {
        ws.write_string_with_format(1, c as u16, *title, &head).map_err(err)?;
        ws.set_column_width(c as u16, (title.chars().count() as f64 * 2.0).max(14.0))
            .map_err(err)?;
    }
    for (i, row) in rows.iter().enumerate() {
        for (c, text) in row.iter().enumerate() {
            if !text.is_empty() {
                // 一律写成文本：税号、编码是数字串，不能被 Excel 当数字吞掉前导 0
                ws.write_string(i as u32 + 2, c as u16, text).map_err(err)?;
            }
        }
    }
    ws.set_freeze_panes(2, 0).map_err(err)?;
    wb.save(path).map_err(err)
}

#[cfg(test)]
mod export_tests {
    use super::*;
    use crate::features::rules::rules_parser::parse_rules_excel;

    // 导出文件必须能被导入解析器原样读回
    #[test]
    fn exported_file_can_be_parsed_by_importer() {
        let rows = vec![
            [
                "印花税计算", "买卖合同", "等于", "91310000ABC", "某供应商",
                "", "", "等于", "办公用品", "是", "备注一", "启用",
            ]
            .map(String::from),
            [
                "进项税转出", "福利", "", "", "", "不等于", "3010101,3010102", "", "", "否", "", "已禁用",
            ]
            .map(String::from),
        ];
        let p = std::env::temp_dir().join("rules_export_roundtrip.xlsx");
        write_rules_xlsx(p.to_str().unwrap(), &rows).unwrap();

        let parsed = parse_rules_excel(&p).unwrap();
        assert!(parsed.errors.is_empty(), "{:?}", parsed.errors.iter().map(|e| &e.message).collect::<Vec<_>>());
        assert_eq!(parsed.rows.len(), 2);
        assert_eq!(parsed.rows[0].excel_row, 3);
        assert_eq!(parsed.rows[0].rule.scope, "stamp_tax");
        assert_eq!(parsed.rows[0].rule.rule_category, "买卖合同");
        assert_eq!(parsed.rows[0].rule.seller_tax_id_values.as_deref(), Some("91310000ABC"));
        assert!(parsed.rows[0].rule.needs_manual_review);
        assert_eq!(parsed.rows[1].rule.tax_code_match_type.as_deref(), Some("neq"));
        assert_eq!(parsed.rows[1].rule.tax_code_values.as_deref(), Some("3010101,3010102"));
        assert!(write_rules_xlsx("x.csv", &[]).is_err());
    }
}
