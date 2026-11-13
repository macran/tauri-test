import type { ColumnDef } from "@tanstack/react-table";

import { commands, type ReviewRow } from "@/bindings";
import type { dataTableFeatures } from "@/features/query/DataTable";
import { ExportButton } from "@/features/query/ExportButton";
import { HistoryQuery, type HistoryRow } from "@/features/tax-period/HistoryQuery";

import { TRANSFER_HISTORY_EXPORT } from "./exportColumns";
import { currentPeriod, fenToYuan, getErrorMessage } from "./utils";

type Column = ColumnDef<typeof dataTableFeatures, HistoryRow<ReviewRow>>;
const money = (get: (r: ReviewRow) => string) => ({
  accessorFn: (r: ReviewRow) => get(r),
  cell: ({ row }: { row: { original: ReviewRow } }) => (
    <span className="block">{fenToYuan(get(row.original))}</span>
  ),
});

// 只读：不含操作列、勾选框和修改控件。批次、纳税所属期、转出凭证信息由 HistoryQuery 统一加。
const COLUMNS: Column[] = [
  { id: "certPeriod", header: "认证期间", size: 90, accessorFn: (r) => r.certPeriod },
  { id: "voucherNo", header: "原记账凭证号", size: 120, accessorFn: (r) => r.voucherNo ?? "" },
  { id: "invoiceNo", header: "发票号码", size: 190, accessorFn: (r) => r.invoiceNo },
  { id: "issueDate", header: "开票日期", size: 100, accessorFn: (r) => r.issueDate },
  { id: "sellerName", header: "销售方名称", size: 200, accessorFn: (r) => r.sellerName },
  { id: "goodsName", header: "商品名称", size: 200, accessorFn: (r) => r.goodsName },
  { id: "goodsCategory", header: "税收分类编码简称", size: 150, accessorFn: (r) => r.goodsCategory ?? "" },
  { id: "amount", header: "金额", size: 110, ...money((r) => r.amount) },
  { id: "tax", header: "税额", size: 100, ...money((r) => r.tax) },
  { id: "decisionSource", header: "结论来源", size: 90, accessorFn: (r) => (r.decisionSource === "manual" ? "人工修改" : "机器匹配") },
  { id: "finalIncluded", header: "转出", size: 70, accessorFn: (r) => (r.finalIncluded ? "是" : "否") },
  { id: "finalTransferTax", header: "转出税额", size: 120, ...money((r) => r.finalTransferTax) },
  { id: "remark", header: "备注", size: 180, accessorFn: (r) => r.remark ?? "" },
];

const NUMERIC = ["amount", "tax", "finalTransferTax"] as const;
// 金额是分的整数字符串；合计用 BigInt，不经过浮点
const summarize = (rows: ReviewRow[]) =>
  `转出税额合计 ${fenToYuan(String(rows.reduce((s, r) => s + BigInt(r.finalTransferTax), 0n)))} 元`;

export default function TransferQueryPage() {
  return (
    <HistoryQuery<ReviewRow>
      columns={COLUMNS}
      voucherLabel="转出"
      onlyLabel="只查转出"
      numericColumns={NUMERIC}
      summarize={summarize}
      fetchPeriods={commands.queryInputTaxTransferHistory}
      initialPeriod={currentPeriod()}
      getError={getErrorMessage}
      exportButton={({ from, to, view, disabled }) => (
        <ExportButton
          disabled={disabled}
          defaultName={view === "confirmed" ? "已确认进项转出" : "进项转出"}
          run={(path) => commands.exportInputTaxTransferHistory(from, to, view, TRANSFER_HISTORY_EXPORT, path)}
        />
      )}
    />
  );
}
