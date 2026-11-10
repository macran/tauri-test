import { useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Label } from "@/components/ui/label";
import { pickSavePath, stApi, twApi } from "../api";
import { RunStatusBadge } from "../components/Badges";
import { fen, periodLabel, ratePpmLabel } from "../format";
import type { StampSummaryRow, StampTemplateSettings, TaxRun } from "../types";
import { StampTemplateSettingsDialog } from "./StampTemplateSettingsDialog";

/** 按纳税所属期汇总，并按税务局模版导出《印花税信息采集表》 */
export function StampCollectionPanel({ refreshKey, presetRunId }: { refreshKey?: number; presetRunId?: number | null }) {
  const [runs, setRuns] = useState<TaxRun[]>([]);
  const [runId, setRunId] = useState<number | null>(null);
  const [batchNo, setBatchNo] = useState<number | null>(null);
  const [settings, setSettings] = useState<StampTemplateSettings | null>(null);
  const [rows, setRows] = useState<StampSummaryRow[]>([]);
  const [settingsOpen, setSettingsOpen] = useState(false);
  const [loading, setLoading] = useState(false);

  useEffect(() => {
    twApi
      .listRuns("stamp_tax")
      .then((all) => {
        const filed = all.filter((r) => r.filingPeriod);
        setRuns(filed);
        setRunId((cur) => presetRunId ?? cur ?? filed[0]?.id ?? null);
      })
      .catch((e) => toast.error((e as Error).message));
    stApi.getTemplateSettings().then(setSettings).catch(() => undefined);
  }, [refreshKey, presetRunId]);

  const run = useMemo(() => runs.find((r) => r.id === runId) ?? null, [runs, runId]);
  const confirmedBatches = run?.batches.filter((b) => b.confirmedAt) ?? [];

  const loadSummary = useCallback(async () => {
    if (!runId || !settings) return;
    setLoading(true);
    try {
      setRows(await stApi.summary(runId, settings.groupLevel, batchNo, false));
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, [runId, batchNo, settings]);

  useEffect(() => {
    void loadSummary();
  }, [loadSummary]);

  const totals = rows.reduce((a, r) => ({ base: a.base + r.baseAmount, tax: a.tax + r.taxAmount, n: a.n + r.lineCount }), { base: 0, tax: 0, n: 0 });
  const level = settings?.groupLevel === "parent" ? "父级税目" : "子目";

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <div className="flex flex-wrap items-end gap-3">
        <div className="grid gap-1">
          <Label htmlFor="col-run">纳税所属期</Label>
          <select
            id="col-run"
            className="h-8 min-w-56 rounded-md border bg-background px-2 text-sm"
            value={runId ?? ""}
            onChange={(e) => {
              setRunId(Number(e.target.value) || null);
              setBatchNo(null);
            }}
          >
            {runs.length === 0 && <option value="">（还没有已确认的纳税所属期）</option>}
            {runs.map((r) => (
              <option key={r.id} value={r.id}>
                {periodLabel(r.filingPeriod)} · 来源凭证期间 {r.srcPeriods?.map(periodLabel).join("；") || periodLabel(r.srcPeriod)}
              </option>
            ))}
          </select>
        </div>
        <div className="grid gap-1">
          <Label htmlFor="col-batch">范围</Label>
          <select
            id="col-batch"
            className="h-8 rounded-md border bg-background px-2 text-sm"
            value={batchNo ?? ""}
            onChange={(e) => setBatchNo(e.target.value ? Number(e.target.value) : null)}
          >
            <option value="">全部已确认批次</option>
            {confirmedBatches.map((b) => (
              <option key={b.id} value={b.batchNo}>
                仅批次 {b.batchNo}
                {b.batchNo > 1 ? "（补充）" : ""}
              </option>
            ))}
          </select>
        </div>
        {run && <RunStatusBadge status={run.status} />}
        <span className="text-sm text-muted-foreground">汇总口径：{level}</span>
        <div className="ml-auto flex gap-2">
          <Button variant="outline" onClick={() => setSettingsOpen(true)}>
            模版设置
          </Button>
          <Button
            disabled={!run || rows.length === 0}
            onClick={async () => {
              if (!run) return;
              if (!settings?.templatePath) {
                toast.error("请先在“模版设置”中选择税务局模版");
                setSettingsOpen(true);
                return;
              }
              const path = await pickSavePath(`印花税信息采集表_${periodLabel(run.filingPeriod)}${batchNo ? `_批次${batchNo}` : ""}.xlsx`);
              if (!path) return;
              try {
                const r = await stApi.exportCollectionForm(run.id, batchNo, path);
                toast.success(`已导出 ${r.lineCount} 个${level}，印花税合计 ${fen(r.taxTotal)} 元`);
              } catch (e) {
                toast.error((e as Error).message);
              }
            }}
          >
            导出信息采集表
          </Button>
        </div>
      </div>

      {run?.status === "reopened" && (
        <p className="rounded-md bg-amber-50 px-3 py-2 text-sm text-amber-900 dark:bg-amber-950 dark:text-amber-100">
          该纳税所属期已解锁，补充批次尚未确认，汇总只包含已确认部分。
        </p>
      )}

      <div className="overflow-auto rounded-md border">
        <table className="w-full text-sm">
          <thead className="bg-muted text-xs text-muted-foreground">
            <tr className="text-left">
              <th className="px-3 py-2 font-medium">税目</th>
              <th className="px-3 py-2 font-medium">子目</th>
              <th className="px-3 py-2 text-right font-medium">税率</th>
              <th className="px-3 py-2 text-right font-medium">明细行</th>
              <th className="px-3 py-2 text-right font-medium">计税金额</th>
              <th className="px-3 py-2 text-right font-medium">应纳税额</th>
            </tr>
          </thead>
          <tbody>
            {rows.map((r) => (
              <tr key={`${r.itemCode}-${r.subItemCode ?? ""}-${r.ratePpm ?? ""}`} className="border-t">
                <td className="px-3 py-1.5">{r.itemName}</td>
                <td className="px-3 py-1.5">{r.subItemName ?? ""}</td>
                <td className="px-3 py-1.5 text-right tabular-nums">{r.ratePpm == null ? "多税率" : ratePpmLabel(r.ratePpm)}</td>
                <td className="px-3 py-1.5 text-right tabular-nums">{r.lineCount}</td>
                <td className="px-3 py-1.5 text-right tabular-nums">{fen(r.baseAmount)}</td>
                <td className="px-3 py-1.5 text-right font-medium tabular-nums">{fen(r.taxAmount)}</td>
              </tr>
            ))}
            {rows.length === 0 && (
              <tr>
                <td colSpan={6} className="px-3 py-10 text-center text-muted-foreground">
                  {loading ? "加载中…" : "没有已确认的印花税明细"}
                </td>
              </tr>
            )}
          </tbody>
          {rows.length > 0 && (
            <tfoot className="border-t-2 font-semibold">
              <tr>
                <td className="px-3 py-2" colSpan={3}>
                  合计
                </td>
                <td className="px-3 py-2 text-right tabular-nums">{totals.n}</td>
                <td className="px-3 py-2 text-right tabular-nums">{fen(totals.base)}</td>
                <td className="px-3 py-2 text-right tabular-nums">{fen(totals.tax)}</td>
              </tr>
            </tfoot>
          )}
        </table>
      </div>
      <p className="text-xs text-muted-foreground">
        应纳税额 = 该{level === "子目" ? "子目" : "子目"}计税金额合计 × 税率（四舍五入到分）；按父级汇总时为各子目税额之和。明细清单中的行级税额仅供参考，可能存在分位尾差。
      </p>

      <StampTemplateSettingsDialog open={settingsOpen} onOpenChange={setSettingsOpen} onSaved={setSettings} />
    </div>
  );
}
