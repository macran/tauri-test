import type { ColumnDef } from "@tanstack/react-table";

import { commands, type StampTaxReviewRow } from "@/bindings";
import type { dataTableFeatures } from "@/features/query/DataTable";
import { HistoryQuery, type HistoryRow } from "@/features/tax-period/HistoryQuery";

import { currentPeriod, fenToYuan, getErrorMessage } from "./utils";

type Column = ColumnDef<typeof dataTableFeatures, HistoryRow<StampTaxReviewRow>>;
const money = (get: (r: StampTaxReviewRow) => string) => ({
  accessorFn: (r: StampTaxReviewRow) => get(r),
  cell: ({ row }: { row: { original: StampTaxReviewRow } }) => (
    <span className="block">{fenToYuan(get(row.original))}</span>
  ),
});

// 只读：不含操作列、勾选框和修改控件。批次、纳税所属期、印花税凭证信息由 HistoryQuery 统一加。
const COLUMNS: Column[] = [
  { id: "bookedPeriod", header: "凭证入账期间", size: 100, accessorFn: (r) => r.bookedPeriod },
  { id: "voucherNo", header: "原记账凭证号", size: 120, accessorFn: (r) => r.voucherNo },
  { id: "invoiceNo", header: "发票号码", size: 190, accessorFn: (r) => r.invoiceNo },
  { id: "issueDate", header: "开票日期", size: 100, accessorFn: (r) => r.issueDate },
  { id: "sellerName", header: "销方名称", size: 200, accessorFn: (r) => r.sellerName },
  { id: "goodsName", header: "商品名称", size: 200, accessorFn: (r) => r.goodsName },
  { id: "amount", header: "金额", size: 110, ...money((r) => r.amount) },
  { id: "stampBase", header: "计税基数", size: 110, ...money((r) => r.stampBase) },
  { id: "tax", header: "税额", size: 100, ...money((r) => r.tax) },
  { id: "decisionSource", header: "结论来源", size: 90, accessorFn: (r) => (r.decisionSource === "manual" ? "人工修改" : "机器匹配") },
  { id: "finalIncluded", header: "计入印花税", size: 100, accessorFn: (r) => (r.finalIncluded ? "是" : "否") },
  { id: "stampTaxCategory", header: "印花税类别", size: 160, accessorFn: (r) => (r.finalIncluded ? (r.stampTaxCategory ?? "待选类别") : "") },
  {
    id: "stampAmount",
    header: "印花税额",
    size: 110,
    accessorFn: (r) => (r.finalIncluded ? Number(r.stampTaxAmount) : 0),
    cell: ({ row }) => (
      <span className="block">{row.original.finalIncluded ? fenToYuan(row.original.stampTaxAmount) : ""}</span>
    ),
  },
  { id: "remark", header: "备注", size: 180, accessorFn: (r) => r.remark ?? "" },
];

const NUMERIC = ["amount", "tax", "stampBase", "stampAmount"] as const;
// 印花税额由后端算好（专用发票按金额，其余按价税合计），这里只汇总行级税额（仅参考；采集表按子目汇总后计算）。
const summarize = (rows: StampTaxReviewRow[]) =>
  `行级印花税额合计 ${fenToYuan(String(rows.reduce((s, r) => s + BigInt(r.stampTaxAmount), 0n)))} 元`;

export default function StampTaxQueryPage() {
  return (
    <HistoryQuery<StampTaxReviewRow>
      columns={COLUMNS}
      voucherLabel="印花税"
      onlyLabel="只查计入印花税"
      numericColumns={NUMERIC}
      summarize={summarize}
      fetchPeriods={commands.queryStampTaxHistory}
      initialPeriod={currentPeriod()}
      getError={getErrorMessage}
    />
  );
}
