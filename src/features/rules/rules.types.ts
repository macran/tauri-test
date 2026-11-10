export type RuleScope = "" | "input_tax_transfer" | "stamp_tax";

export type MatchType = "" | "eq" | "neq";

export type RuleFormValues = {
  scope: RuleScope;
  ruleCategory: string;

  sellerTaxIdMatchType: MatchType;
  sellerTaxIdValues: string;

  taxCodeMatchType: MatchType;
  taxCodeValues: string;

  taxNameMatchType: MatchType;
  taxNameValues: string;

  needsManualReview: boolean;
  note: string;
};

export const defaultRuleFormValues: RuleFormValues = {
  scope: "input_tax_transfer",
  ruleCategory: "",

  sellerTaxIdMatchType: "",
  sellerTaxIdValues: "",

  taxCodeMatchType: "",
  taxCodeValues: "",

  taxNameMatchType: "",
  taxNameValues: "",

  needsManualReview: false,
  note: "",
};

export type RuleFieldKey =
  | "ruleCategory"
  | "sellerTaxIdMatchType"
  | "sellerTaxIdValues"
  | "taxCodeMatchType"
  | "taxCodeValues"
  | "taxNameMatchType"
  | "taxNameValues"
  | "matchConditions";

const FIELD_ORDER: RuleFieldKey[] = [
  "ruleCategory",
  "sellerTaxIdMatchType",
  "sellerTaxIdValues",
  "taxCodeMatchType",
  "taxCodeValues",
  "taxNameMatchType",
  "taxNameValues",
  "matchConditions",
];

function splitRuleValues(value: string): string[] {
  return value
    .split("，")
    .join(",")
    .split(",")
    .map((item: string) => item.trim())
    .filter(Boolean);
}

/**
 * Validates the rule form and returns per-field error messages, keyed in the
 * order fields should be focused in when the form is invalid.
 */
export function validateRuleFields(
  value: RuleFormValues
): Partial<Record<RuleFieldKey, string>> {
  const errors: Partial<Record<RuleFieldKey, string>> = {};

  if (!value.ruleCategory.trim()) {
    errors.ruleCategory =
      value.scope === "stamp_tax" ? "请选择印花税税目" : "规则类别不能为空";
  }

  const dimensions: Array<{
    label: string;
    typeKey: RuleFieldKey;
    valueKey: RuleFieldKey;
    type: MatchType;
    values: string;
  }> = [
    {
      label: "销售方税号",
      typeKey: "sellerTaxIdMatchType",
      valueKey: "sellerTaxIdValues",
      type: value.sellerTaxIdMatchType,
      values: value.sellerTaxIdValues,
    },
    {
      label: "税收分类编码",
      typeKey: "taxCodeMatchType",
      valueKey: "taxCodeValues",
      type: value.taxCodeMatchType,
      values: value.taxCodeValues,
    },
    {
      label: "税收分类编码简称",
      typeKey: "taxNameMatchType",
      valueKey: "taxNameValues",
      type: value.taxNameMatchType,
      values: value.taxNameValues,
    },
  ];

  let hasDimension = false;

  for (const dimension of dimensions) {
    const hasType = dimension.type !== "";
    const hasValue = dimension.values.trim() !== "";

    if (hasType && !hasValue) {
      errors[dimension.valueKey] = "已选择匹配方式，但匹配内容为空";
      continue;
    }

    if (!hasType && hasValue) {
      errors[dimension.typeKey] = "未选择匹配方式，不能填写匹配内容";
      continue;
    }

    if (hasType && hasValue) {
      hasDimension = true;

      if (
        dimension.type === "eq" &&
        splitRuleValues(dimension.values).length > 1
      ) {
        errors[dimension.valueKey] = `使用"等于"时只能填写一个值`;
      }
    }
  }

  if (!hasDimension && Object.keys(errors).length === 0) {
    errors.matchConditions = "至少需要选择一个匹配维度并填写对应内容";
  }

  const ordered: Partial<Record<RuleFieldKey, string>> = {};
  for (const key of FIELD_ORDER) {
    if (errors[key]) {
      ordered[key] = errors[key];
    }
  }

  return ordered;
}

export function matchTypeLabel(value: string | null | undefined): string {
  switch (value) {
    case "eq":
      return "等于";
    case "neq":
      return "不等于";
    default:
      return "未设置";
  }
}

export function scopeLabel(scope: string): string {
  switch (scope) {
    case "input_tax_transfer":
      return "进项税转出";
    case "stamp_tax":
      return "印花税计算";
    default:
      return scope;
  }
}
