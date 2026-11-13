// 红冲查询：一次查询同时得到「汇总表」「明细表」和「对应关系」。
//   - 蓝字视角：汇总 = 蓝字发票一行（累计冲红 / 剩余），明细 = 蓝字明细一行
//   - 红字视角：汇总 = 红字发票一行（已匹配 / 未匹配 / 核对结果），明细 = 红字明细一行
//   - 对应关系：一行 = 一条红冲匹配记录，两个视角共用，用来看“谁冲了谁”
// 红冲核对产生的结果是只读的，这里只查询，不做任何修改。
// 凭证可能一张发票对应多条（AP / IC）：默认一行只带第一条，选「按凭证展开」才展开成多行，
// 一次只展开一个维度，避免行数相乘。

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo, useState } from "react";
import { z } from "zod";

import {
  commands,
  type DiscountStats,
  type RedDetailRow,
  type RedSummaryRow,
  type ReversalDetailRow,
  type ReversalLinkRow,
  type ReversalQueryFilter,
  type ReversalQueryResult,
  type ReversalSummaryRow,
} from "@/bindings";
import { DataTable, type dataTableFeatures } from "@/features/query/DataTable";
import { DateRangeField, FieldShell, RadioField, SelectField, TextField, defaultDateRange } from "@/features/query/fields";
import { formatMoney } from "@/features/query/format";
import { ExportButton } from "@/features/query/ExportButton";
import {
  MATCH_RESULT_OPTIONS,
  BLUE_DETAIL_EXPORT,
  BLUE_SUMMARY_EXPORT,
  RED_DETAIL_EXPORT,
  RED_SUMMARY_EXPORT,
} from "./exportColumns";
import { LayoutSelect } from "@/features/query/LayoutSelect";
import { QueryShell } from "@/features/query/QueryShell";
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from "@/components/ui/select";
import { useTableLayouts } from "@/features/query/table-layout";
import {
  enumLabel,
  type EnumOption,
  INV_STATE_OPTIONS,
  INV_TYPE_OPTIONS,
  IS_POSITIVE_OPTIONS,
} from "@/lib/enum-options";
import { cn } from "@/lib/utils";

type View = "blue" | "red";
type Expand = "" | "ap" | "ic";
type Tab = "summary" | "detail" | "link";

const VIEW_OPTIONS: { value: View; label: string }[] = [
  { value: "blue", label: "蓝字\n发票" },
  { value: "red", label: "红字\n发票" },
];
const EXPAND_OPTIONS: { value: Expand; label: string }[] = [
  { value: "", label: "不展开" },
  { value: "ap", label: "AP 凭证" },
  { value: "ic", label: "IC 凭证" },
];
/** 核对结果：与后端 match_result 一致 */

const schema = z
  .object({
    view: z.enum(["blue", "red"]),
    issueDateFrom: z.string(),
    issueDateTo: z.string(),
    invoiceNumber: z.string(),
    sellerName: z.string(),
    sellerTaxNo: z.string(),
    state: z.array(z.number()),
    matchResult: z.array(z.number()),
    expand: z.enum(["", "ap", "ic"]),
  })
  .refine((v) => !v.issueDateFrom || !v.issueDateTo || v.issueDateFrom <= v.issueDateTo, {
    message: "开始日期不能晚于结束日期",
    path: ["issueDateTo"],
  });

type FilterValues = z.infer<typeof schema>;

const dateRange = defaultDateRange();
const defaultValues: FilterValues = {
  view: "blue",
  issueDateFrom: dateRange.from,
  issueDateTo: dateRange.to,
  invoiceNumber: "",
  sellerName: "",
  sellerTaxNo: "",
  state: [],
  matchResult: [],
  expand: "",
};

type Features = typeof dataTableFeatures;
type Col<T extends object> = ColumnDef<Features, T>;

// ───────── 列工厂 ─────────
const text = <T extends object,>(id: string, header: string, size: number, get: (r: T) => unknown): Col<T> =>
  ({ id, header, size, accessorFn: get }) as Col<T>;
const money = <T extends object,>(id: string, header: string, get: (r: T) => string): Col<T> =>
  ({ id, header, size: 110, accessorFn: (r: T) => formatMoney(get(r)) }) as Col<T>;
const enumc = <T extends object,>(id: string, header: string, size: number, opts: EnumOption[], get: (r: T) => number): Col<T> =>
  ({ id, header, size, accessorFn: (r: T) => enumLabel(opts, get(r)) }) as Col<T>;

type Head = {
  code: string | null;
  num: string | null;
  eNum: string | null;
  sellerTaxNo: string;
  sellerName: string;
  issueDate: string;
  invType: number;
  state: number;
  remark: string | null;
  voucher: { voucherNo: string | null; postedDate: string | null } | null;
  voucherCount: number;
};

/** 发票号码 + 销方 + 开票日期 + 票种状态：四张主表都一样 */
const headCols = <T extends Head>(): Col<T>[] => [
  text<T>("code", "发票代码", 110, (r) => r.code ?? ""),
  text<T>("num", "发票号码", 100, (r) => r.num ?? ""),
  text<T>("eNum", "数电发票号码", 170, (r) => r.eNum ?? ""),
  text<T>("sellerTaxNo", "销方识别号", 160, (r) => r.sellerTaxNo),
  text<T>("sellerName", "销方名称", 200, (r) => r.sellerName),
  text<T>("issueDate", "开票日期", 100, (r) => r.issueDate),
  enumc<T>("invType", "发票票种", 200, INV_TYPE_OPTIONS, (r) => r.invType),
  enumc<T>("state", "发票状态", 100, INV_STATE_OPTIONS, (r) => r.state),
];
const voucherCols = <T extends Head>(): Col<T>[] => [
  text<T>("voucherNo", "记账凭证号码", 140, (r) => r.voucher?.voucherNo ?? ""),
  text<T>("postedDate", "入账时间", 100, (r) => r.voucher?.postedDate ?? ""),
  text<T>("voucherCount", "凭证数", 70, (r) => r.voucherCount),
];
const isPosCol = <T extends object,>(): Col<T> => text<T>("isPos", "是否正数发票", 110, () => enumLabel(IS_POSITIVE_OPTIONS, 1));

// ───────── 蓝字视角 ─────────
type Blue = ReversalSummaryRow | ReversalDetailRow;
const blueMoney = <T extends Blue>(): Col<T>[] => [
  money<T>("amount", "金额", (r) => r.amount),
  money<T>("tax", "税额", (r) => r.tax),
  money<T>("total", "价税合计", (r) => r.total),
];
const blueReversed = <T extends Blue>(): Col<T>[] => [
  money<T>("reversedAmount", "累计冲红金额", (r) => r.reversedAmount),
  money<T>("reversedTax", "累计冲红税额", (r) => r.reversedTax),
  money<T>("reversedTotal", "累计冲红价税合计", (r) => r.reversedTotal),
];
const blueTail = <T extends Blue>(): Col<T>[] => [
  money<T>("remainAmount", "剩余金额", (r) => r.remainAmount),
  money<T>("remainTax", "剩余税额", (r) => r.remainTax),
  money<T>("remainTotal", "剩余价税合计", (r) => r.remainTotal),
  text<T>("redInvoiceNos", "红字发票号码", 200, (r) => r.redInvoiceNos ?? ""),
  text<T>("redNoticeNos", "红字信息表编号", 200, (r) => r.redNoticeNos ?? ""),
];

const BLUE_SUMMARY: Col<ReversalSummaryRow>[] = [
  ...headCols<ReversalSummaryRow>(),
  isPosCol<ReversalSummaryRow>(),
  text<ReversalSummaryRow>("remark", "备注", 160, (r) => r.remark ?? ""),
  ...blueMoney<ReversalSummaryRow>(),
  ...blueReversed<ReversalSummaryRow>(),
  ...blueTail<ReversalSummaryRow>(),
  ...voucherCols<ReversalSummaryRow>(),
];

/** 明细表共用的商品列，放在开票日期之后 */
const goodsCols = <T extends ReversalDetailRow | RedDetailRow>(): Col<T>[] => {
  const d = (id: string, header: string, size: number, get: (r: T) => unknown) => text<T>(id, header, size, get);
  return [
    d("lineNo", "行号", 60, (r) => r.lineNo),
    d("taxCode", "税收分类编码", 140, (r) => r.taxCode ?? ""),
    d("goodsName", "货物或应税劳务名称", 200, (r) => r.goodsName),
    d("specModel", "规格型号", 120, (r) => r.specModel ?? ""),
    d("unit", "单位", 60, (r) => r.unit ?? ""),
    d("quantity", "数量", 90, (r) => r.quantity ?? ""),
    d("unitPrice", "单价", 100, (r) => r.unitPrice ?? ""),
    d("taxRate", "税率", 70, (r) => r.taxRate ?? ""),
  ];
};

const BLUE_DETAIL: Col<ReversalDetailRow>[] = (() => {
  const head = headCols<ReversalDetailRow>();
  const d = (id: string, header: string, size: number, get: (r: ReversalDetailRow) => unknown) =>
    text<ReversalDetailRow>(id, header, size, get);
  return [
    ...head.slice(0, 6), // 发票号码、销方、开票日期
    d("lineFlag", "行标志", 80, (r) => (r.isDiscount ? "折扣行" : "")),
    ...goodsCols<ReversalDetailRow>(),
    ...head.slice(6), // 票种、状态
    isPosCol<ReversalDetailRow>(),
    d("remark", "备注", 160, (r) => r.remark ?? ""),
    ...blueMoney<ReversalDetailRow>(),
    d("reversedQuantity", "累计冲红数量", 110, (r) => r.reversedQuantity ?? ""),
    ...blueReversed<ReversalDetailRow>(),
    d("remainQuantity", "剩余数量", 100, (r) => r.remainQuantity ?? ""),
    ...blueTail<ReversalDetailRow>(),
    ...voucherCols<ReversalDetailRow>(),
  ];
})();

// ───────── 红字视角 ─────────
type Red = RedSummaryRow | RedDetailRow;
const matchResultText = (r: number) => enumLabel(MATCH_RESULT_OPTIONS, r);

const redMoney = <T extends Red>(): Col<T>[] => [
  money<T>("amount", "金额", (r) => r.amount),
  money<T>("tax", "税额", (r) => r.tax),
  money<T>("total", "价税合计", (r) => r.total),
  money<T>("matchedAmount", "已匹配金额", (r) => r.matchedAmount),
  money<T>("matchedTax", "已匹配税额", (r) => r.matchedTax),
  money<T>("matchedTotal", "已匹配价税合计", (r) => r.matchedTotal),
  money<T>("unmatchedAmount", "未匹配金额", (r) => r.unmatchedAmount),
  money<T>("unmatchedTax", "未匹配税额", (r) => r.unmatchedTax),
  money<T>("unmatchedTotal", "未匹配价税合计", (r) => r.unmatchedTotal),
];
const redTail = <T extends Red>(): Col<T>[] => [
  text<T>("redNoticeNo", "红字信息表编号", 200, (r) => r.redNoticeNo ?? ""),
  text<T>("blueInvKey", "对应蓝字发票号码", 190, (r) => r.blueInvKey ?? ""),
  text<T>("blueIssueDate", "蓝字发票开票日期", 120, (r) => r.blueIssueDate ?? ""),
  text<T>("matchResult", "核对结果", 110, (r) => matchResultText(r.matchResult)),
  text<T>("matchNote", "核对备注", 260, (r) => r.matchNote ?? ""),
];

const RED_SUMMARY: Col<RedSummaryRow>[] = [
  ...headCols<RedSummaryRow>(),
  text<RedSummaryRow>("remark", "备注", 160, (r) => r.remark ?? ""),
  ...redMoney<RedSummaryRow>(),
  ...redTail<RedSummaryRow>(),
  ...voucherCols<RedSummaryRow>(),
];

const RED_DETAIL: Col<RedDetailRow>[] = (() => {
  const head = headCols<RedDetailRow>();
  return [
    ...head.slice(0, 6),
    ...goodsCols<RedDetailRow>(),
    ...head.slice(6),
    text<RedDetailRow>("remark", "备注", 160, (r) => r.remark ?? ""),
    ...redMoney<RedDetailRow>(),
    text<RedDetailRow>("blueLineNos", "冲到的蓝字行号", 120, (r) => r.blueLineNos ?? ""),
    ...redTail<RedDetailRow>(),
    ...voucherCols<RedDetailRow>(),
  ];
})();

// ───────── 对应关系 ─────────
/** 号码优先用代码-号码，其次数电号码；蓝票不在库时退回红票头里记的对应号码 */
const invNo = (code: string | null, num: string | null, eNum: string | null, fallback?: string | null) =>
  code && num ? `${code}-${num}` : (eNum ?? fallback ?? "");

const L = (id: string, header: string, size: number, get: (r: ReversalLinkRow) => unknown) =>
  text<ReversalLinkRow>(id, header, size, get);

const LINK_RED: Col<ReversalLinkRow>[] = [
  L("redNo", "红字发票号码", 190, (r) => invNo(r.redCode, r.redNum, r.redENum)),
  L("redLineNo", "红字行号", 80, (r) => r.redLineNo),
  L("redIssueDate", "红字开票日期", 110, (r) => r.redIssueDate),
  L("redNoticeNo", "红字信息表编号", 200, (r) => r.redNoticeNo ?? ""),
];
const LINK_BLUE: Col<ReversalLinkRow>[] = [
  L("blueNo", "蓝字发票号码", 190, (r) => invNo(r.blueCode, r.blueNum, r.blueENum, r.blueInvKey)),
  L("blueLineNo", "蓝字行号", 80, (r) => r.blueLineNo ?? ""),
  L("blueIssueDate", "蓝字开票日期", 110, (r) => r.blueIssueDate ?? ""),
];
const LINK_TAIL: Col<ReversalLinkRow>[] = [
  L("sellerName", "销方名称", 200, (r) => r.sellerName ?? ""),
  L("matchedQuantity", "匹配数量", 90, (r) => r.matchedQuantity ?? ""),
  money<ReversalLinkRow>("matchedAmount", "匹配金额", (r) => r.matchedAmount),
  money<ReversalLinkRow>("matchedTax", "匹配税额", (r) => r.matchedTax),
  money<ReversalLinkRow>("matchedTotal", "匹配价税合计", (r) => r.matchedTotal),
  L("matchResult", "核对结果", 110, (r) => matchResultText(r.matchResult)),
  L("matchNote", "核对备注", 260, (r) => r.matchNote ?? ""),
];
/** 同一批匹配记录：蓝字视角读作“这批蓝票被谁冲了”，红字视角读作“这批红票冲了谁”，只是哪一边的号码放在前面 */
const LINKS_BLUE_FIRST = [...LINK_BLUE, ...LINK_RED, ...LINK_TAIL];
const LINKS_RED_FIRST = [...LINK_RED, ...LINK_BLUE, ...LINK_TAIL];

// ───────── 数字列（右对齐）与列筛选 ─────────
const NUMERIC = [
  "amount", "tax", "total",
  "reversedAmount", "reversedTax", "reversedTotal",
  "remainAmount", "remainTax", "remainTotal",
  "matchedAmount", "matchedTax", "matchedTotal",
  "unmatchedAmount", "unmatchedTax", "unmatchedTotal",
  "quantity", "unitPrice", "reversedQuantity", "remainQuantity", "matchedQuantity", "voucherCount",
] as const;

/** 行标志只有“折扣行 / 普通行”两种，列筛选里选“折扣行”就只看折扣行 */
const LINE_FLAG_FILTER = {
  lineFlag: [{ value: "折扣行", label: "折扣行" }],
};

const MATCH_RESULT_FILTER = {
  matchResult: MATCH_RESULT_OPTIONS.map((o) => ({ value: o.label, label: o.label })),
} as const;

/** 蓝字明细表底部：本次查询范围内的折扣行笔数与金额（后端统计，不受表格搜索/筛选影响） */
function DiscountFooter({ d }: { d: DiscountStats }) {
  if (d.count === 0) return <span className="text-muted-foreground">无折扣行</span>;
  return (
    <>
      <span className="font-medium">折扣行 {d.count} 笔</span>
      <span>折扣金额 {formatMoney(d.amount)}</span>
      <span>折扣税额 {formatMoney(d.tax)}</span>
      <span>折扣价税合计 {formatMoney(d.total)}</span>
    </>
  );
}

const voucherKey = (r: { voucher: { voucherNo: string | null; postedDate: string | null } | null }) =>
  `${r.voucher?.voucherNo ?? ""}-${r.voucher?.postedDate ?? ""}`;

export default function ReversalQueryPage() {
  const [result, setResult] = useState<ReversalQueryResult | null>(null);
  /** 结果是用哪个视角查出来的：表格按它渲染，不跟表单当前值走（改了视角但没点查询时，旧结果不能套新列） */
  const [resultView, setResultView] = useState<View>("blue");
  const [loading, setLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [tab, setTab] = useState<Tab>("summary");
  // 导出用上次点查询时的条件（不是表单里改了没查的值），视角与已显示的结果一致
  const [lastFilter, setLastFilter] = useState<ReversalQueryFilter | null>(null);

  const blueSummary = useTableLayouts("reversal-blue-summary");
  const blueDetail = useTableLayouts("reversal-blue-detail");
  const redSummary = useTableLayouts("reversal-red-summary");
  const redDetail = useTableLayouts("reversal-red-detail");
  const linkLayouts = useTableLayouts("reversal-link");

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: async ({ value }) => {
      setLoading(true);
      setError(null);
      try {
        const red = value.view === "red";
        const filter: ReversalQueryFilter = {
          view: value.view,
          issueDateFrom: value.issueDateFrom || null,
          issueDateTo: value.issueDateTo || null,
          invoiceNumber: value.invoiceNumber || null,
          sellerName: value.sellerName || null,
          sellerTaxNo: value.sellerTaxNo || null,
          state: !red && value.state.length ? value.state : null,
          matchResult: value.matchResult.length ? value.matchResult : null,
          expand: value.expand || null,
        };
        const res = await commands.queryReversalOverview(filter);
        if (res.status === "ok") {
          setLastFilter(filter);
          setResult(res.data);
          setResultView(value.view);
        } else setError(res.error || "查询失败");
      } catch (e) {
        setError(e instanceof Error ? e.message : "查询失败");
      } finally {
        setLoading(false);
      }
    },
  });

  const isRed = resultView === "red";
  const activeLayouts =
    tab === "link" ? linkLayouts : tab === "summary" ? (isRed ? redSummary : blueSummary) : isRed ? redDetail : blueDetail;

  const tabs: { value: Tab; label: string }[] = [
    { value: "summary", label: "汇总表" },
    { value: "detail", label: "明细表" },
    { value: "link", label: "对应关系" },
  ];

  // 一次导出会把汇总表和明细表一起写进一个文件，所以三个 tab 的工具栏里是同一个按钮，标签写明范围
  const exportNode = useMemo(
    () => (
    <ExportButton
      label="导出汇总+明细"
      defaultName={isRed ? "红冲查询_红字" : "红冲查询_蓝字"}
      disabled={!lastFilter || !result || (result.summaryTotal === 0 && result.detailTotal === 0)}
      run={(path) =>
        commands.exportReversal(
          lastFilter!,
          isRed ? RED_SUMMARY_EXPORT : BLUE_SUMMARY_EXPORT,
          isRed ? RED_DETAIL_EXPORT : BLUE_DETAIL_EXPORT,
          path,
        )
      }
    />
    ),
    [isRed, lastFilter, result],
  );

  const summaryTable = useMemo(
    () =>
      isRed ? (
        <DataTable
          toolbarSlot={exportNode}
          key="red-summary"
          layouts={redSummary}
          columns={RED_SUMMARY}
          data={result?.redSummary ?? []}
          loading={loading}
          total={result?.summaryTotal ?? 0}
          truncated={!!result && result.redSummary.length < result.summaryTotal}
          numericColumns={NUMERIC}
          filterColumns={MATCH_RESULT_FILTER}
          rowKey={(r) => `${r.headerId}-${voucherKey(r)}`}
        />
      ) : (
        <DataTable
          toolbarSlot={exportNode}
          key="blue-summary"
          layouts={blueSummary}
          columns={BLUE_SUMMARY}
          data={result?.summary ?? []}
          loading={loading}
          total={result?.summaryTotal ?? 0}
          truncated={!!result && result.summary.length < result.summaryTotal}
          numericColumns={NUMERIC}
          rowKey={(r) => `${r.headerId}-${voucherKey(r)}`}
        />
      ),
    [isRed, result, loading, redSummary, blueSummary, exportNode],
  );
  const detailTable = useMemo(
    () =>
      isRed ? (
        <DataTable
          toolbarSlot={exportNode}
          key="red-detail"
          layouts={redDetail}
          columns={RED_DETAIL}
          data={result?.redDetails ?? []}
          loading={loading}
          total={result?.detailTotal ?? 0}
          truncated={!!result && result.redDetails.length < result.detailTotal}
          numericColumns={NUMERIC}
          filterColumns={MATCH_RESULT_FILTER}
          rowKey={(r) => `${r.detailId}-${voucherKey(r)}`}
        />
      ) : (
        <DataTable
          toolbarSlot={exportNode}
          key="blue-detail"
          layouts={blueDetail}
          columns={BLUE_DETAIL}
          data={result?.details ?? []}
          loading={loading}
          total={result?.detailTotal ?? 0}
          truncated={!!result && result.details.length < result.detailTotal}
          numericColumns={NUMERIC}
          filterColumns={LINE_FLAG_FILTER}
          footerSlot={result && <DiscountFooter d={result.discount} />}
          rowKey={(r) => `${r.detailId}-${voucherKey(r)}`}
        />
      ),
    [isRed, result, loading, redDetail, blueDetail, exportNode],
  );

  return (
    <QueryShell
      searching={loading}
      error={error}
      onSubmit={() => form.handleSubmit()}
      layout={<LayoutSelect layouts={activeLayouts} />}
      onReset={() => form.reset()}
      basicFields={
        <>
          <form.Field name="view">
            {(f) => (
              <RadioField label="查询对象" value={f.state.value} onChange={(v) => f.handleChange(v)} options={VIEW_OPTIONS} />
            )}
          </form.Field>
          <form.Field name="issueDateFrom">
            {(fromF) => (
              <form.Field name="issueDateTo">
                {(toF) => (
                  <form.Subscribe selector={(s) => s.values.view}>
                    {(view) => (
                      <DateRangeField
                        label={view === "red" ? "红字发票开票日期" : "蓝字发票开票日期"}
                        fromField={fromF}
                        toField={toF}
                      />
                    )}
                  </form.Subscribe>
                )}
              </form.Field>
            )}
          </form.Field>
          <form.Field name="invoiceNumber">
            {(f) => <TextField field={f} label="发票号码" placeholder="发票代码+号码或数电号码" />}
          </form.Field>
          <form.Field name="sellerName">
            {(f) => <TextField field={f} label="销方名称" />}
          </form.Field>
          <form.Subscribe selector={(s) => s.values.view}>
            {(view) =>
              view === "red" ? (
                <form.Field name="matchResult">
                  {(f) => <SelectField field={f} label="核对结果" options={MATCH_RESULT_OPTIONS} />}
                </form.Field>
              ) : (
                <form.Field name="state">
                  {(f) => <SelectField field={f} label="发票状态" options={INV_STATE_OPTIONS} />}
                </form.Field>
              )
            }
          </form.Subscribe>
        </>
      }
      advancedFields={
        <>
          <form.Field name="sellerTaxNo">
            {(f) => <TextField field={f} label="销方识别号" />}
          </form.Field>
          <form.Field name="expand">
            {(f) => (
              <FieldShell label="按凭证展开">
                <Select
                  items={EXPAND_OPTIONS}
                  value={f.state.value}
                  onValueChange={(v) => f.handleChange((v ?? "") as Expand)}
                >
                  <SelectTrigger className="w-full min-w-0">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {EXPAND_OPTIONS.map((o) => (
                      <SelectItem key={o.value} value={o.value}>
                        {o.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </FieldShell>
            )}
          </form.Field>
        </>
      }
    >
      {/* 汇总 / 明细 / 对应关系：各表都保持挂载，切换不丢列筛选和滚动位置 */}
      <div role="tablist" className="flex w-fit shrink-0 gap-0.5 rounded-xl bg-slate-100 p-1 dark:bg-slate-900/60">
        {tabs.map((t) => (
          <button
            key={t.value}
            role="tab"
            type="button"
            aria-selected={tab === t.value}
            onClick={() => setTab(t.value)}
            className={cn(
              "rounded-lg px-4 py-1 text-xs font-medium transition-[background-color,color,box-shadow] duration-200 motion-reduce:transition-none",
              tab === t.value ? "bg-primary text-primary-foreground shadow-sm" : "text-muted-foreground hover:bg-background/60 hover:text-foreground",
            )}
          >
            {t.label}
          </button>
        ))}
      </div>

      <div hidden={tab !== "summary"} className="flex min-h-0 flex-1 flex-col">{summaryTable}</div>
      <div hidden={tab !== "detail"} className="flex min-h-0 flex-1 flex-col">{detailTable}</div>
      <div hidden={tab !== "link"} className="flex min-h-0 flex-1 flex-col">
        <DataTable
          toolbarSlot={exportNode}
          key={isRed ? "link-red" : "link-blue"}
          layouts={linkLayouts}
          columns={isRed ? LINKS_RED_FIRST : LINKS_BLUE_FIRST}
          data={result?.links ?? []}
          loading={loading}
          total={result?.linkTotal ?? 0}
          truncated={!!result && result.links.length < result.linkTotal}
          numericColumns={NUMERIC}
          filterColumns={MATCH_RESULT_FILTER}
          rowKey={(r) => r.id}
        />
      </div>
    </QueryShell>
  );
}
