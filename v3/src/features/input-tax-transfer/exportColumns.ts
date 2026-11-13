// 进项转出导出列。key 是 ReviewRow 的字段名；已确认的行带 confirm.*（批次与转出凭证信息，后端嵌套在行里）。
// 「原记账凭证」= AP/IC/银行/差旅的凭证；「转出凭证」= 进项转出自己的会计凭证（锁定时填写）。
import { type ExportCol, moneyCol, textCol } from "@/features/query/ExportButton";

const t = textCol;
const BOOL = { true: "是", false: "否" };

export const TRANSFER_EXPORT: ExportCol[] = [
  t("certPeriod", "认证期间"),
  t("voucherNo", "原记账凭证号"),
  t("invoiceNo", "发票号码"),
  t("issueDate", "开票日期"),
  t("sellerTaxNo", "销方税号"),
  t("sellerName", "销售方名称"),
  t("goodsName", "商品名称"),
  t("taxCode", "税收分类编码"),
  t("goodsCategory", "税收分类编码简称"),
  t("taxRate", "税率"),
  moneyCol("amount", "金额"),
  moneyCol("tax", "税额"),
  t("matchState", "匹配状态", { matched: "已命中", notMatched: "未命中", missingVoucher: "缺凭证号" }),
  t("needsManualReview", "需人工核对", BOOL),
  t("decisionSource", "结论来源", { manual: "人工修改", system: "机器匹配" }),
  t("finalIncluded", "转出", BOOL),
  moneyCol("finalTransferTax", "转出税额"),
  t("remark", "备注"),
  t("confirm.filingPeriod", "确认所属期"),
  t("confirm.batchNo", "批次"),
  t("confirm.state", "批次状态", { confirmed: "已确认", locked: "已锁定" }),
  t("confirm.voucherNo", "转出凭证号"),
  t("confirm.postedDate", "转出入账日期"),
  t("confirm.preparer", "做账人"),
  t("confirm.confirmedBy", "确认人"),
  t("confirm.confirmedAt", "确认时间"),
  t("confirm.lockedAt", "锁定时间"),
];

/** 查询页：每行补上纳税所属期 queryPeriod */
export const TRANSFER_HISTORY_EXPORT: ExportCol[] = [t("queryPeriod", "纳税所属期"), ...TRANSFER_EXPORT];
