use super::models::{MatchedRule, RuleCondition, RuleForMatch};

pub(crate) struct MatchInput<'a> {
    pub seller_tax_no: Option<&'a str>,
    pub tax_code: Option<&'a str>,
    pub goods_category: Option<&'a str>,
}

fn operator_label(match_type: &str) -> &'static str {
    match match_type {
        "eq" => "等于",
        "neq" => "不等于",
        _ => "未知匹配方式",
    }
}

/// 维度未配置（match_type/values 任一为空）视为不约束，恒为 true。
fn dimension_matches(
    match_type: Option<&str>,
    values: Option<&str>,
    actual: Option<&str>,
) -> bool {
    let (Some(match_type), Some(values)) = (match_type, values) else {
        return true;
    };

    let actual = actual.unwrap_or("").trim();
    let is_in_values = values
        .split(',')
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .any(|value| value == actual);

    match match_type {
        "eq" => is_in_values,
        "neq" => !is_in_values,
        _ => false,
    }
}

fn rule_matches(rule: &RuleForMatch, item: &MatchInput<'_>) -> bool {
    dimension_matches(
        rule.seller_tax_id_match_type.as_deref(),
        rule.seller_tax_id_values.as_deref(),
        item.seller_tax_no,
    ) && dimension_matches(
        rule.tax_code_match_type.as_deref(),
        rule.tax_code_values.as_deref(),
        item.tax_code,
    ) && dimension_matches(
        rule.tax_name_match_type.as_deref(),
        rule.tax_name_values.as_deref(),
        item.goods_category,
    )
}

fn push_condition(
    conditions: &mut Vec<RuleCondition>,
    field: &str,
    match_type: Option<&str>,
    values: Option<&str>,
) {
    if let (Some(match_type), Some(values)) = (match_type, values) {
        conditions.push(RuleCondition {
            field: field.to_string(),
            operator: operator_label(match_type).to_string(),
            values: values.to_string(),
        });
    }
}

/// 把规则转换为前端可渲染的结构：条件逐条列出，附带备注与人工核对标记。
pub(crate) fn to_matched_rule(rule: &RuleForMatch) -> Result<MatchedRule, String> {
    let mut conditions = Vec::with_capacity(3);

    push_condition(
        &mut conditions,
        "销售方税号",
        rule.seller_tax_id_match_type.as_deref(),
        rule.seller_tax_id_values.as_deref(),
    );
    push_condition(
        &mut conditions,
        "税收分类编码",
        rule.tax_code_match_type.as_deref(),
        rule.tax_code_values.as_deref(),
    );
    push_condition(
        &mut conditions,
        "税收分类编码简称",
        rule.tax_name_match_type.as_deref(),
        rule.tax_name_values.as_deref(),
    );

    Ok(MatchedRule {
        rule_id: u32::try_from(rule.id).map_err(|_| "规则 ID 超出可导出范围".to_string())?,
        rule_category: rule.rule_category.clone(),
        note: rule.note.clone(),
        needs_manual_review: rule.needs_manual_review,
        enabled: rule.enabled,
        conditions,
    })
}

/// 返回命中的规则 ID，顺序与规则表一致。
pub(crate) fn find_matching_rule_ids(
    rules: &[RuleForMatch],
    item: &MatchInput<'_>,
) -> Vec<i64> {
    rules
        .iter()
        .filter(|rule| rule_matches(rule, item))
        .map(|rule| rule.id)
        .collect()
}