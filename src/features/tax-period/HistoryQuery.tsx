// 转出查询 / 印花税查询共用的只读外壳：纳税所属期范围 → 区间内每个纳税所属期的明细。
// 两个视图：
//   「仅已确认」 = 已确认或已锁定的明细（锁定后的就是申报给税务局的清单，永远不变）
//   「全部」     = 还包括本所属期待确认的转出 / 计入明细（当期匹配成功或人工修改的）
// 没有任何确认、锁定、修改的操作。每行带纳税所属期、批次、转出 / 印花税凭证信息。

import { MagnifyingGlass } from "@phosphor-icons/react";
import type { ColumnDef } from "@tanstack/react-table";
import { type ReactNode, useMemo, useState } from "react";

import type { ConfirmInfo } from "@/bindings";
import { Card, CardContent } from "@/components/ui/card";
import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import { Switch } from "@/components/ui/switch";
import { toast } from "@/components/ui/toast";
import { DataTable, type dataTableFeatures } from "@/features/query/DataTable";

import { Field, TaxPeriodField, TaxPeriodForm } from "./TaxPeriodForm";
import { type FieldErrors, historyRangeSchema, validate } from "./schema";

export type HistoryView = "confirmed" | "all";

/** 两个模块的行都带这些字段 */
export type HistoryBase = { detailId: number; confirm: ConfirmInfo | null; counted: boolean };

/** queryPeriod = 该行所属的纳税所属期 */
export type HistoryRow<T> = T & { queryPeriod: string };
type Column<T> = ColumnDef<typeof dataTableFeatures, HistoryRow<T>>;

/** 批次列：未确认为「待确认」；别的所属期确认的写明所属期 */
export function batchText(c: ConfirmInfo | null): string {
  if (!c) return "待确认";
  const state = c.state === "locked" ? "已锁定" : "已确认";
  const batch = c.batchNo > 1 ? `补充批次 ${c.batchNo}` : "批次 1";
  return c.sameRun ? `${batch} ${state}` : `${c.filingPeriod} ${batch} ${state}`;
}

/** 各模块只需给出自己的业务列；纳税所属期、批次、凭证信息这里统一加。凭证列叫「转出/印花税凭证」，区别于原记账凭证。 */
export function historyColumns<T extends HistoryBase>(
  business: Column<T>[],
  voucherLabel: string,
): Column<T>[] {
  return [
    { id: "queryPeriod", header: "纳税所属期", size: 100, accessorFn: (r) => r.queryPeriod },
    { id: "batch", header: "批次", size: 140, accessorFn: (r) => batchText(r.confirm) },
    ...business,
    { id: "acctVoucherNo", header: `${voucherLabel}凭证号`, size: 130, accessorFn: (r) => r.confirm?.voucherNo ?? "" },
    { id: "acctPostedDate", header: `${voucherLabel}入账日期`, size: 120, accessorFn: (r) => r.confirm?.postedDate ?? "" },
    { id: "acctPreparer", header: "做账人", size: 80, accessorFn: (r) => r.confirm?.preparer ?? "" },
    { id: "confirmedBy", header: "确认人", size: 80, accessorFn: (r) => r.confirm?.confirmedBy ?? "" },
    { id: "lockedAt", header: "锁定时间", size: 150, accessorFn: (r) => r.confirm?.lockedAt ?? "" },
  ];
}

const FILTERS = { queryPeriod: "auto", batch: "auto", acctVoucherNo: "auto" } as const;

export function HistoryQuery<T extends HistoryBase>({
  columns,
  voucherLabel,
  onlyLabel,
  numericColumns,
  summarize,
  fetchPeriods,
  initialPeriod,
  getError,
  exportButton,
}: {
  columns: Column<T>[];
  /** 凭证列前缀：「转出」或「印花税」 */
  voucherLabel: string;
  /** 数字列 id（表头与内容右对齐） */
  numericColumns: readonly string[];
  /** 「只查××」勾选框的文字，如「只查转出」 */
  onlyLabel: string;
  /** 对已确认（含已锁定）的明细汇总，返回要显示的文字 */
  summarize: (rows: T[]) => string;
  fetchPeriods: (
    from: string,
    to: string,
    view: HistoryView,
  ) => Promise<
    | { status: "ok"; data: { review: { filingFrom: string; filingTo: string; rows: T[] } }[] }
    | { status: "error"; error: string }
  >;
  initialPeriod: string;
  getError: (e: unknown) => string;
  /** 导出按钮（后端按条件重新查询）；不传则不显示 */
  /** 导出按钮；range 是最近一次查询的范围，disabled = 还没查询过或查到 0 条 */
  exportButton?: (range: { from: string; to: string; view: HistoryView; disabled: boolean }) => ReactNode;
}) {
  const [from, setFrom] = useState(initialPeriod);
  const [to, setTo] = useState(initialPeriod);
  const [view, setView] = useState<HistoryView>("all");
  const [errors, setErrors] = useState<FieldErrors>({});
  const [rows, setRows] = useState<HistoryRow<T>[]>([]);
  const [only, setOnly] = useState(false);
  const [loading, setLoading] = useState(false);
  // 导出放在表格工具栏，它要导出的是「表格里这批数据」，所以用最近一次查询的范围，而不是表单里此刻的输入
  const [lastRange, setLastRange] = useState<{ from: string; to: string; view: HistoryView } | null>(null);

  const cols = useMemo(() => historyColumns<T>(columns, voucherLabel), [columns, voucherLabel]);

  async function run() {
    const checked = validate(historyRangeSchema, { periodFrom: from, periodTo: to });
    if (!checked.ok) return setErrors(checked.errors);
    setErrors({});
    setLoading(true);
    try {
      const result = await fetchPeriods(checked.data.periodFrom, checked.data.periodTo, view);
      if (result.status !== "ok") throw new Error(getError(result.error));
      const next = result.data.flatMap((p) => {
        const r = p.review;
        const queryPeriod = r.filingFrom === r.filingTo ? r.filingFrom : `${r.filingFrom}-${r.filingTo}`;
        return r.rows.map((row) => ({ ...row, queryPeriod }));
      });
      setRows(next);
      setLastRange({ from: checked.data.periodFrom, to: checked.data.periodTo, view });
      if (next.length === 0) toast.add({ type: "info", title: "所选纳税所属期内没有结果" });
    } catch (error) {
      toast.add({ type: "error", title: "查询失败", description: getError(error) });
    } finally {
      setLoading(false);
    }
  }

  // 「只查」：只留最终为「是」的行（counted = 转出 / 计入，且不是别处已确认的）
  const shown = useMemo(() => (only ? rows.filter((r) => r.counted) : rows), [rows, only]);
  // 合计只算已确认 / 已锁定的明细；待确认的不算，单独提示条数
  const confirmedRows = useMemo(() => shown.filter((r) => r.counted && r.confirm), [shown]);
  const pendingCount = useMemo(() => shown.filter((r) => r.counted && !r.confirm).length, [shown]);

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col gap-3 p-3">
      <Card>
        <CardContent>
          <TaxPeriodForm
            running={loading}
            onRun={() => void run()}
            onReset={() => {
              setFrom(initialPeriod);
              setTo(initialPeriod);
              setErrors({});
              setRows([]);
              setOnly(false);
              setView("all");
              setLastRange(null);
            }}
            runLabel="查询"
            runIcon={<MagnifyingGlass data-icon="inline-start" weight="bold" />}
          >
            <TaxPeriodField label="纳税所属期（起）" value={from} onChange={setFrom} error={errors.periodFrom} />
            <TaxPeriodField label="纳税所属期（止）" value={to} onChange={setTo} error={errors.periodTo} />
            <Field label="范围">
              {/* 默认「全部」；打开后只看已确认 / 已锁定 */}
              <Label className="h-(--control-h) gap-2 text-xs text-foreground">
                <Switch
                  size="sm"
                  checked={view === "confirmed"}
                  onCheckedChange={(on) => setView(on ? "confirmed" : "all")}
                  aria-label="仅已确认"
                />
                仅已确认
              </Label>
            </Field>
          </TaxPeriodForm>
        </CardContent>
      </Card>

      <Card className="min-h-0 flex-1">
        <CardContent className="flex min-h-0 flex-1 flex-col gap-2">
          <DataTable
            footerSlot={
              <>
                {rows.length > 0 && (
                  <span className="font-medium">
                    {summarize(confirmedRows)}（已确认或已锁定，共 {confirmedRows.length} 条）
                  </span>
                )}
                {pendingCount > 0 && (
                  <span className="text-muted-foreground">另有 {pendingCount} 条尚未确认，不计入合计</span>
                )}
              </>
            }
            filterSlot={
              <Label className="flex items-center gap-2 text-xs">
                <Checkbox checked={only} onCheckedChange={(c) => setOnly(c === true)} aria-label={onlyLabel} />
                {onlyLabel}
              </Label>
            }
            toolbarSlot={exportButton?.({
              from: lastRange?.from ?? from,
              to: lastRange?.to ?? to,
              view: lastRange?.view ?? view,
              disabled: !lastRange || rows.length === 0,
            })}
            columns={cols}
            numericColumns={numericColumns}
            data={shown}
            loading={loading}
            total={shown.length}
            rowKey={(r) => `${r.queryPeriod}-${r.detailId}`}
            filterColumns={FILTERS}
          />
        </CardContent>
      </Card>
    </div>
  );
}
