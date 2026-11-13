// 印花税导出列（交会计做账的清单）。key 是 StampTaxReviewRow 的字段名；已确认的行带 confirm.*。
// 「原记账凭证」= AP/IC/银行/差旅的凭证；「印花税凭证」= 印花税自己的会计凭证（锁定时填写）。
import { type ExportCol, moneyCol, textCol } from "@/features/query/ExportButton";

const t = textCol;

export const STAMP_EXPORT: ExportCol[] = [
  t("bookedPeriod", "凭证入账期间"),
  t("voucherNo", "原记账凭证号"),
  t("invoiceNo", "发票号码"),
  t("issueDate", "开票日期"),
  t("sellerTaxNo", "销方识别号"),
  t("sellerName", "销方名称"),
  t("goodsName", "商品名称"),
  t("taxCode", "税收分类编码"),
  t("taxRate", "税率"),
  moneyCol("amount", "金额"),
  moneyCol("tax", "税额"),
  moneyCol("total", "价税合计"),
  moneyCol("stampBase", "计税基数"),
  t("stampTaxCategory", "印花税类别"),
  moneyCol("stampTaxAmount", "行级印花税额"),
  t("decisionSource", "结论来源", { manual: "人工修改", system: "机器匹配" }),
  t("remark", "备注"),
  t("confirm.filingPeriod", "确认所属期"),
  t("confirm.batchNo", "批次"),
  t("confirm.state", "批次状态", { confirmed: "已确认", locked: "已锁定" }),
  t("confirm.voucherNo", "印花税凭证号"),
  t("confirm.postedDate", "印花税入账日期"),
  t("confirm.preparer", "做账人"),
  t("confirm.confirmedBy", "确认人"),
  t("confirm.confirmedAt", "确认时间"),
  t("confirm.lockedAt", "锁定时间"),
];
