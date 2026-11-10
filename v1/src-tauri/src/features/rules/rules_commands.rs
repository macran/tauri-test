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