use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RuleInput {
    pub scope: String,
    pub rule_category: String,

    pub seller_tax_id_match_type: Option<String>,
    pub seller_tax_id_values: Option<String>,

    pub tax_code_match_type: Option<String>,
    pub tax_code_values: Option<String>,

    pub tax_name_match_type: Option<String>,
    pub tax_name_values: Option<String>,

    pub needs_manual_review: bool,
    pub note: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(tag = "status", rename_all = "camelCase")]
pub enum AddTaxMatchRuleResult {
    Inserted {
        rule: TaxMatchRuleRecord,
    },
    Duplicate {
        message: String,
    },
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct StampTaxCategoryOption {
    pub id: u32,
    pub category_name: String,
    /// 税率基点，实际税率 = rate_bp / 100000
    pub rate_bp: u32,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct TaxMatchRuleRecord {
    // 对外 DTO 不使用 i64，避免 Specta 导出 BigInt。
    pub id: u32,

    pub scope: String,
    pub rule_category: String,

    pub stamp_tax_category_id: Option<u32>,

    pub seller_tax_id_match_type: Option<String>,
    pub seller_tax_id_values: Option<String>,

    pub tax_code_match_type: Option<String>,
    pub tax_code_values: Option<String>,

    pub tax_name_match_type: Option<String>,
    pub tax_name_values: Option<String>,

    pub needs_manual_review: bool,
    pub note: String,
    pub enabled: bool,
    pub disabled_at: Option<String>,
    pub source: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RuleImportError {
    // Excel 行号用 u32 即可。
    pub excel_row: u32,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, specta::Type)]
#[serde(rename_all = "camelCase")]
pub struct RuleImportSummary {
    pub total_rows: u32,
    pub inserted: u32,
    pub duplicates: u32,
    pub invalid_rows: u32,
    pub errors: Vec<RuleImportError>,
}
#[derive(Debug, Clone, Serialize)]
pub(crate) struct NormalizedRule {
    pub(crate) scope: String,
    pub(crate) rule_category: String,
    pub(crate) stamp_tax_category_id: Option<i64>,

    pub(crate) seller_tax_id_match_type: Option<String>,
    pub(crate) seller_tax_id_values: Option<String>,
    pub(crate) tax_code_match_type: Option<String>,
    pub(crate) tax_code_values: Option<String>,
    pub(crate) tax_name_match_type: Option<String>,
    pub(crate) tax_name_values: Option<String>,

    pub(crate) needs_manual_review: bool,
    pub(crate) note: String,
}

impl RuleInput {
    pub(crate) fn normalize(self) -> Result<NormalizedRule, String> {
        let scope = match self.scope.trim() {
            "进项税转出" | "input_tax_transfer" => "input_tax_transfer",
            "印花税计算" | "stamp_tax" => "stamp_tax",
            other => return Err(format!("规则所属无效：{other}")),
        }
        .to_string();

        let rule_category = self.rule_category.trim().to_string();
        if rule_category.is_empty() {
            return Err("规则类别不能为空".to_string());
        }

        let seller_type = normalize_match_type(self.seller_tax_id_match_type.as_deref())?;
        let tax_code_type = normalize_match_type(self.tax_code_match_type.as_deref())?;
        let tax_name_type = normalize_match_type(self.tax_name_match_type.as_deref())?;

        let seller_values = normalize_values(
            &seller_type,
            self.seller_tax_id_values.as_deref().unwrap_or(""),
            "销售方税号",
        )?;
        let tax_code_values = normalize_values(
            &tax_code_type,
            self.tax_code_values.as_deref().unwrap_or(""),
            "税收分类编码",
        )?;
        let tax_name_values = normalize_values(
            &tax_name_type,
            self.tax_name_values.as_deref().unwrap_or(""),
            "税收分类编码简称",
        )?;

        if seller_type.is_none() && tax_code_type.is_none() && tax_name_type.is_none() {
            return Err("至少要填写一个匹配维度".to_string());
        }

        Ok(NormalizedRule {
            scope,
            rule_category,
            stamp_tax_category_id: None,
            seller_tax_id_match_type: seller_type,
            seller_tax_id_values: seller_values,
            tax_code_match_type: tax_code_type,
            tax_code_values,
            tax_name_match_type: tax_name_type,
            tax_name_values,
            needs_manual_review: self.needs_manual_review,
            note: self.note.trim().to_string(),
        })
    }
}

fn normalize_match_type(value: Option<&str>) -> Result<Option<String>, String> {
    match value.unwrap_or("").trim() {
        "" => Ok(None),
        "等于" | "eq" | "equal" => Ok(Some("eq".to_string())),
        "不等于" | "neq" | "not_equal" => Ok(Some("neq".to_string())),
        other => Err(format!("匹配方式无效：{other}")),
    }
}

pub(crate) fn normalize_review(value: &str) -> Result<bool, String> {
    match value.trim() {
        "是" | "true" | "1" => Ok(true),
        "否" | "false" | "0" | "" => Ok(false),
        other => Err(format!("是否核对只能是“是”或“否”，实际为：{other}")),
    }
}

fn normalize_values(
    match_type: &Option<String>,
    raw_value: &str,
    dimension_name: &str,
) -> Result<Option<String>, String> {
    let raw_value = raw_value.trim();
    if match_type.is_none() {
        return if raw_value.is_empty() {
            Ok(None)
        } else {
            Err(format!("{dimension_name}匹配方式为空时，匹配内容也必须为空"))
        };
    }

    let mut values: Vec<String> = raw_value
        .replace('，', ",")
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .collect();

    if values.is_empty() {
        return Err(format!("{dimension_name}匹配内容不能为空"));
    }

    if match_type.as_deref() == Some("eq") {
        if values.len() != 1 {
            return Err(format!(
                "{dimension_name}使用等于时只能填写一个值；多个值应拆成多行规则"
            ));
        }
        return Ok(Some(values.remove(0)));
    }

    values.sort();
    values.dedup();
    Ok(Some(values.join(",")))
}