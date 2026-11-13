// src/components/ReversalMatch.tsx
// 红冲核对页面（红冲管理的一个标签，标题由 TabbedPage 提供）：表单卡片（红字发票开票日期区间 + 重置/核对）+ 结果卡片（异常明细表 + 底部统计）。
// 布局与进项转出匹配页一致；核对逻辑（commands.matchReversalInvoices）不变。

import type { ColumnDef } from "@tanstack/react-table";
import { useCallback, useMemo, useState } from "react";

import {
  commands,
  type ReversalAbnormalItem,
  type ReversalMatchSummary,
} from "@/bindings";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { toast } from "@/components/ui/toast";
import { DataTable, type dataTableFeatures } from "@/features/query/DataTable";
import { defaultDateRange, parseLocalDate, toIsoDate } from "@/features/query/fields";
import { Field, TaxPeriodForm } from "@/features/tax-period/TaxPeriodForm";
import { Win11DatePicker } from "@/components/ui/win11-date-picker";

type Column = ColumnDef<typeof dataTableFeatures, ReversalAbnormalItem>;

const TYPE_LABEL: Record<ReversalAbnormalItem["matchResultType"], string> = {
  ok: "成功",
  blueMissing: "无蓝字发票",
  abnormal: "红字发票异常",
};

const COLUMNS: Column[] = [
  { id: "redInvUnqKey", header: "红票号码", size: 190, accessorFn: (r) => r.redInvUnqKey },
  { id: "redLineNo", header: "行号", size: 60, accessorFn: (r) => r.redLineNo },
  { id: "matchResultType", header: "类型", size: 110, accessorFn: (r) => TYPE_LABEL[r.matchResultType] },
  { id: "blueInvUnqKey", header: "蓝票号码", size: 190, accessorFn: (r) => r.blueInvUnqKey ?? "" },
  { id: "blueIssueDate", header: "开票日期", size: 100, accessorFn: (r) => r.blueIssueDate?.slice(0, 10) ?? "" },
  { id: "blueSellerName", header: "销售方名称", size: 200, accessorFn: (r) => r.blueSellerName ?? "" },
  { id: "blueSellerTaxNo", header: "销售方税号", size: 170, accessorFn: (r) => r.blueSellerTaxNo ?? "" },
  { id: "matchNote", header: "备注", size: 360, accessorFn: (r) => r.matchNote ?? "" },
];

const FILTERS = {
  matchResultType: Object.values(TYPE_LABEL)
    .filter((v) => v !== TYPE_LABEL.ok)
    .map((v) => ({ value: v, label: v })),
} as const;

function getErrorMessage(error: unknown): string {
  if (typeof error === "string") return error;
  return (error as Error)?.message ?? "未知错误";
}

export default function ReversalMatch() {
  // 默认区间与查询页一致：近 2 年 ~ 今天
  const [{ from: initFrom, to: initTo }] = useState(defaultDateRange);
  const [startDate, setStartDate] = useState(initFrom);
  const [endDate, setEndDate] = useState(initTo);
  const [summary, setSummary] = useState<ReversalMatchSummary | null>(null);
  const [running, setRunning] = useState(false);

  const rangeError =
    startDate && endDate && startDate > endDate ? "开始日期不能晚于结束日期" : undefined;
  const isRangeValid = startDate !== "" && endDate !== "" && !rangeError;

  const runMatch = useCallback(async () => {
    if (!isRangeValid) return;
    setRunning(true);
    try {
      const result = await commands.matchReversalInvoices(startDate, endDate);
      if (result.status !== "ok") throw new Error(getErrorMessage(result.error ?? "核对失败"));
      setSummary(result.data);
      const { abnormalCount, blueMissingCount, okCount } = result.data;
      if (okCount === 0 && abnormalCount === 0 && blueMissingCount === 0) {
        toast.add({ type: "info", title: "所选区间内没有待核对的红字发票" });
      }
    } catch (error) {
      toast.add({ type: "error", title: "核对失败", description: getErrorMessage(error) });
    } finally {
      setRunning(false);
    }
  }, [startDate, endDate, isRangeValid]);

  const handleReset = () => {
    const range = defaultDateRange();
    setStartDate(range.from);
    setEndDate(range.to);
    setSummary(null);
  };

  const items = useMemo(() => summary?.abnormalItems ?? [], [summary]);

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col gap-3 p-3">
        <Card>
          <CardContent>
            <TaxPeriodForm
              running={running}
              runDisabled={!isRangeValid}
              onRun={() => void runMatch()}
              onReset={handleReset}
              runLabel="核对"
            >
              <Field label="红字发票开票日期（起）" error={rangeError}>
                <Win11DatePicker
                  clearable={false}
                  aria-label="开始日期"
                  aria-invalid={!!rangeError}
                  value={parseLocalDate(startDate)}
                  onChange={(d) => setStartDate(toIsoDate(d))}
                  className="w-full"
                />
              </Field>
              <Field label="红字发票开票日期（止）">
                <Win11DatePicker
                  clearable={false}
                  aria-label="结束日期"
                  aria-invalid={!!rangeError}
                  value={parseLocalDate(endDate)}
                  onChange={(d) => setEndDate(toIsoDate(d))}
                  className="w-full"
                />
              </Field>
            </TaxPeriodForm>
          </CardContent>
        </Card>

        <Card className="min-h-0 flex-1">
          <CardContent className="flex min-h-0 flex-1 flex-col gap-2">
            <DataTable
              columns={COLUMNS}
              data={items}
              loading={running}
              total={items.length}
              rowKey={(r) => `${r.redInvUnqKey}-${r.redLineNo}-${r.matchResultType}`}
              filterColumns={FILTERS}
              resetKey={summary?.matchRunNo}
            />

            {summary && (
              <div className="flex flex-wrap items-center gap-2">
                <Badge variant="secondary">核对编号 #{summary.matchRunNo}</Badge>
                <Badge variant="secondary">成功匹配 {summary.okCount} 条</Badge>
                <Badge variant="secondary">无蓝字发票 {summary.blueMissingCount} 条</Badge>
                <Badge variant={summary.abnormalCount > 0 ? "destructive" : "secondary"}>
                  红字发票异常 {summary.abnormalCount} 条
                </Badge>
                {items.length === 0 && (
                  <span className="text-xs text-muted-foreground">本次所有红冲发票均核对成功</span>
                )}
              </div>
            )}
          </CardContent>
        </Card>
    </div>
  );
}
