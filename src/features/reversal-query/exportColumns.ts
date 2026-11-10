// 红冲查询导出的列定义。key 是后端结果行的 camelCase 字段名（"voucher.voucherNo" 支持点路径）。
// 与页面表格列一一对应，但页面列用格式化后的 accessorFn，无法直接复用。
import { type ExportCol, enumCols, moneyCol, textCol } from "@/features/query/ExportButton";
import { INV_STATE_OPTIONS, INV_TYPE_OPTIONS, IS_POSITIVE_OPTIONS } from "@/lib/enum-options";

export const MATCH_RESULT_OPTIONS = [
  { value: 0, label: "未核对" },
  { value: 1, label: "成功" },
  { value: 2, label: "无蓝字发票" },
  { value: 3, label: "红字发票异常" },
];

const t = textCol;
const m = moneyCol;
const inv = enumCols(INV_TYPE_OPTIONS);
const state = enumCols(INV_STATE_OPTIONS);
const match = enumCols(MATCH_RESULT_OPTIONS);

const head = (): ExportCol[] => [
  t("code", "发票代码"),
  t("num", "发票号码"),
  t("eNum", "数电发票号码"),
  t("sellerTaxNo", "销方识别号"),
  t("sellerName", "销方名称"),
  t("issueDate", "开票日期"),
];
const kind = (): ExportCol[] => [t("invType", "发票票种", inv), t("state", "发票状态", state)];
const voucher = (): ExportCol[] => [
  t("voucher.voucherNo", "记账凭证号码"),
  t("voucher.postedDate", "入账时间"),
  t("voucherCount", "凭证数"),
];
const goods = (): ExportCol[] => [
  t("lineNo", "行号"),
  t("taxCode", "税收分类编码"),
  t("goodsName", "货物或应税劳务名称"),
  t("specModel", "规格型号"),
  t("unit", "单位"),
  t("quantity", "数量"),
  t("unitPrice", "单价"),
  t("taxRate", "税率"),
];
const isPos = (): ExportCol => t("isPos", "是否正数发票", enumCols(IS_POSITIVE_OPTIONS));

const blueMoney = (): ExportCol[] => [m("amount", "金额"), m("tax", "税额"), m("total", "价税合计")];
const blueReversed = (): ExportCol[] => [
  m("reversedAmount", "累计冲红金额"),
  m("reversedTax", "累计冲红税额"),
  m("reversedTotal", "累计冲红价税合计"),
];
const blueTail = (): ExportCol[] => [
  m("remainAmount", "剩余金额"),
  m("remainTax", "剩余税额"),
  m("remainTotal", "剩余价税合计"),
  t("redInvoiceNos", "红字发票号码"),
  t("redNoticeNos", "红字信息表编号"),
];
const redMoney = (): ExportCol[] => [
  m("amount", "金额"),
  m("tax", "税额"),
  m("total", "价税合计"),
  m("matchedAmount", "已匹配金额"),
  m("matchedTax", "已匹配税额"),
  m("matchedTotal", "已匹配价税合计"),
  m("unmatchedAmount", "未匹配金额"),
  m("unmatchedTax", "未匹配税额"),
  m("unmatchedTotal", "未匹配价税合计"),
];
const redTail = (): ExportCol[] => [
  t("redNoticeNo", "红字信息表编号"),
  t("blueInvKey", "对应蓝字发票号码"),
  t("blueIssueDate", "蓝字发票开票日期"),
  t("matchResult", "核对结果", match),
  t("matchNote", "核对备注"),
];

export const BLUE_SUMMARY_EXPORT: ExportCol[] = [
  ...head(), ...kind(), isPos(), t("remark", "备注"),
  ...blueMoney(), ...blueReversed(), ...blueTail(), ...voucher(),
];
export const BLUE_DETAIL_EXPORT: ExportCol[] = [
  ...head(),
  t("isDiscount", "行标志", { true: "折扣行", false: "" }),
  ...goods(), ...kind(), isPos(), t("remark", "备注"),
  ...blueMoney(),
  t("reversedQuantity", "累计冲红数量"),
  ...blueReversed(),
  t("remainQuantity", "剩余数量"),
  ...blueTail(), ...voucher(),
];
export const RED_SUMMARY_EXPORT: ExportCol[] = [
  ...head(), ...kind(), t("remark", "备注"), ...redMoney(), ...redTail(), ...voucher(),
];
export const RED_DETAIL_EXPORT: ExportCol[] = [
  ...head(), ...goods(), ...kind(), t("remark", "备注"),
  ...redMoney(),
  t("blueLineNos", "冲到的蓝字行号"),
  ...redTail(), ...voucher(),
];
