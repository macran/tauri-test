import { type ReactNode, useCallback, useEffect, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { pickSavePath, twApi } from "../api";
import { fen, periodLabel } from "../format";
import type { ConfirmInput, ExportResult, LineStats, RunEvent, TaxModule, TaxRun } from "../types";
import { AdminReasonDialog } from "./AdminReasonDialog";
import { RunStatusBadge } from "./Badges";
import { ConfirmRunDialog } from "./ConfirmRunDialog";
import { useIsAdmin } from "./useIsAdmin";

export interface RunsPanelProps {
  module: TaxModule;
  moduleLabel: string;
  srcLabel: string; // 认证期间 / 凭证期间
  taxLabel: string; // 转出税额 / 印花税额
  filePrefix: string; // 导出文件名前缀
  loadStats: (runId: number) => Promise<LineStats>;
  exportPending: (runId: number, path: string) => Promise<ExportResult>;
  exportConfirmedBatch: (runId: number, batchNo: number, path: string) => Promise<ExportResult>;
  confirm: (input: ConfirmInput) => Promise<TaxRun>;
  onOpenRun: (run: TaxRun) => void;
  /** 父组件改变该值可触发刷新 */
  refreshKey?: number;
  extraActions?: (run: TaxRun) => ReactNode;
}

type AdminAction = { kind: "unlock" | "relock"; run: TaxRun } | null;

/** 纳税所属期结果集：查看已选来源期间 → 导出 / 回填会计凭证确认 / 管理员解锁 */
export function RunsPanel(p: RunsPanelProps) {
  const admin = useIsAdmin();
  const [runs, setRuns] = useState<TaxRun[]>([]);
  const [loading, setLoading] = useState(false);
  const [confirmRun, setConfirmRun] = useState<{ run: TaxRun; stats: LineStats } | null>(null);
  const [adminAction, setAdminAction] = useState<AdminAction>(null);
  const [events, setEvents] = useState<{ run: TaxRun; list: RunEvent[] } | null>(null);
  const [filter, setFilter] = useState<"all" | "todo" | "done">("all");

  const reload = useCallback(async () => {
    setLoading(true);
    try {
      setRuns(await twApi.listRuns(p.module));
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setLoading(false);
    }
  }, [p.module]);

  useEffect(() => {
    void reload();
  }, [reload, p.refreshKey]);

  const shown = runs.filter((r) => (filter === "all" ? true : filter === "todo" ? r.status !== "confirmed" : r.status === "confirmed"));

  const doExportPending = async (run: TaxRun) => {
    const b = run.batches.find((x) => !x.confirmedAt);
    const path = await pickSavePath(`${p.filePrefix}_${periodLabel(run.filingPeriod)}_批次${b?.batchNo ?? 1}.xlsx`);
    if (!path) return;
    try {
      const r = await p.exportPending(run.id, path);
      toast.success(`已导出 ${r.lineCount} 行，${p.taxLabel} ${fen(r.taxTotal)} 元`);
      void reload();
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  const doExportBatch = async (run: TaxRun, batchNo: number) => {
    const path = await pickSavePath(`${p.filePrefix}_${periodLabel(run.filingPeriod ?? run.srcPeriod)}_批次${batchNo}_已确认.xlsx`);
    if (!path) return;
    try {
      const r = await p.exportConfirmedBatch(run.id, batchNo, path);
      toast.success(`已导出 ${r.lineCount} 行`);
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  const openConfirm = async (run: TaxRun) => {
    try {
      setConfirmRun({ run, stats: await p.loadStats(run.id) });
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  const doDelete = async (run: TaxRun) => {
    if (!window.confirm(`删除纳税所属期 ${periodLabel(run.filingPeriod)} 的未确认结果集和人工修改？`)) return;
    try {
      await twApi.deleteRun(run.id);
      toast.success("已删除");
      void reload();
    } catch (e) {
      toast.error((e as Error).message);
    }
  };

  return (
    <div className="flex min-h-0 flex-col gap-3">
      <div className="flex flex-wrap items-center gap-2">
        <div className="inline-flex rounded-md border p-0.5 text-xs" role="tablist">
          {(
            [
              ["all", "全部"],
              ["todo", "待确认 / 已解锁"],
              ["done", "已确认"],
            ] as const
          ).map(([k, t]) => (
            <button
              key={k}
              role="tab"
              aria-selected={filter === k}
              onClick={() => setFilter(k)}
              className={`rounded px-2 py-1 ${filter === k ? "bg-primary text-primary-foreground" : "text-muted-foreground hover:bg-muted"}`}
            >
              {t}
            </button>
          ))}
        </div>
        <Button size="sm" variant="outline" onClick={() => void reload()} disabled={loading}>
          {loading ? "加载中…" : "刷新"}
        </Button>
        {admin && <span className="text-xs text-muted-foreground">当前为管理员，可按纳税所属期解锁任意已确认结果集</span>}
      </div>

      {shown.length === 0 && !loading && (
        <div className="rounded-md border border-dashed p-8 text-center text-sm text-muted-foreground">
          还没有纳税所属期结果集。到“匹配与复核”先选择纳税所属期，再选{p.srcLabel}开始匹配。
        </div>
      )}

      <ul className="grid gap-3 overflow-auto">
        {shown.map((run) => {
          const pending = run.batches.find((b) => !b.confirmedAt);
          const editable = run.status !== "confirmed";
          const everConfirmed = run.batches.some((b) => b.confirmedAt);
          return (
            <li key={run.id} className="rounded-lg border bg-card p-3">
              <div className="flex flex-wrap items-center gap-x-4 gap-y-2">
                <div className="flex items-baseline gap-2">
                  <span className="text-xs text-muted-foreground">纳税所属期（结果集）</span>
                  <span className="font-semibold tabular-nums">{periodLabel(run.filingPeriod) || "旧批次待迁移"}</span>
                </div>
                <div className="flex min-w-0 items-baseline gap-2">
                  <span className="shrink-0 text-xs text-muted-foreground">已选{p.srcLabel}</span>
                  <span className="max-w-[32rem] truncate text-sm">{run.srcPeriods?.map(periodLabel).join("；") || periodLabel(run.srcPeriod)}</span>
                </div>
                <RunStatusBadge status={run.status} />
                <span className="text-xs text-muted-foreground">
                  最近匹配 {run.lastMatchedAt ?? "—"} · 创建 {run.createdBy}
                </span>
                <div className="ml-auto flex flex-wrap gap-1.5">
                  <Button size="sm" variant="outline" onClick={() => p.onOpenRun(run)}>
                    {editable ? "打开复核" : "查看明细"}
                  </Button>
                  {editable && (
                    <>
                      <Button size="sm" variant="outline" onClick={() => void doExportPending(run)}>
                        导出本批清单
                      </Button>
                      <Button size="sm" onClick={() => void openConfirm(run)}>
                        回填凭证并确认
                      </Button>
                    </>
                  )}
                  {p.extraActions?.(run)}
                  {admin && run.status === "confirmed" && (
                    <Button size="sm" variant="outline" onClick={() => setAdminAction({ kind: "unlock", run })}>
                      解锁该所属期
                    </Button>
                  )}
                  {admin && run.status === "reopened" && (
                    <Button size="sm" variant="outline" onClick={() => setAdminAction({ kind: "relock", run })}>
                      恢复锁定
                    </Button>
                  )}
                  {run.status === "open" && !everConfirmed && (
                    <Button size="sm" variant="ghost" className="text-destructive" onClick={() => void doDelete(run)}>
                      删除
                    </Button>
                  )}
                  <Button
                    size="sm"
                    variant="ghost"
                    onClick={async () => setEvents({ run, list: await twApi.listEvents(run.id) })}
                  >
                    日志
                  </Button>
                </div>
              </div>

              <table className="mt-3 w-full text-xs">
                <thead className="text-muted-foreground">
                  <tr className="border-b text-left">
                    <th className="py-1 pr-2 font-medium">批次</th>
                    <th className="py-1 pr-2 font-medium">导出</th>
                    <th className="py-1 pr-2 text-right font-medium">行数</th>
                    <th className="py-1 pr-2 text-right font-medium">{p.taxLabel}</th>
                    <th className="py-1 pr-2 font-medium">会计凭证号</th>
                    <th className="py-1 pr-2 font-medium">入账日期</th>
                    <th className="py-1 pr-2 font-medium">做账人</th>
                    <th className="py-1 pr-2 font-medium">确认</th>
                    <th className="py-1" />
                  </tr>
                </thead>
                <tbody>
                  {run.batches.map((b) => (
                    <tr key={b.id} className="border-b last:border-0">
                      <td className="py-1 pr-2">
                        {b.batchNo}
                        <span className="ml-1 text-muted-foreground">{b.batchNo === 1 ? "首次" : "补充"}</span>
                      </td>
                      <td className="py-1 pr-2">{b.exportedAt ? `${b.exportedAt} ${b.exportedBy ?? ""}` : "未导出"}</td>
                      <td className="py-1 pr-2 text-right tabular-nums">{b.exportLineCount ?? ""}</td>
                      <td className="py-1 pr-2 text-right tabular-nums">{fen(b.exportTaxTotal)}</td>
                      <td className="py-1 pr-2">{b.acctVoucherNo ?? ""}</td>
                      <td className="py-1 pr-2">{b.acctPostedDate ?? ""}</td>
                      <td className="py-1 pr-2">{b.acctPreparer ?? ""}</td>
                      <td className="py-1 pr-2">
                        {b.confirmedAt ? (
                          `${b.confirmedAt} ${b.confirmedBy}`
                        ) : (
                          <span className="text-amber-700 dark:text-amber-300">待确认</span>
                        )}
                      </td>
                      <td className="py-1 text-right">
                        {b.confirmedAt && (
                          <Button size="sm" variant="ghost" className="h-6 px-2 text-xs" onClick={() => void doExportBatch(run, b.batchNo)}>
                            重新导出
                          </Button>
                        )}
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
              {pending && pending.batchNo > 1 && (
                <p className="mt-2 text-xs text-amber-700 dark:text-amber-300">
                  已解锁：只能新增{p.taxLabel}明细，原已确认明细不可取消。新增部分导出补充清单交会计做账后，按原纳税所属期确认。
                </p>
              )}
            </li>
          );
        })}
      </ul>

      <ConfirmRunDialog
        open={!!confirmRun}
        onOpenChange={(o) => !o && setConfirmRun(null)}
        run={confirmRun?.run ?? null}
        stats={confirmRun?.stats ?? null}
        moduleLabel={p.moduleLabel}
        taxLabel={p.taxLabel}
        onSubmit={async (input) => {
          try {
            await p.confirm(input);
            toast.success("已确认并锁定");
            setConfirmRun(null);
            void reload();
          } catch (e) {
            toast.error((e as Error).message);
          }
        }}
      />

      <AdminReasonDialog
        open={!!adminAction}
        onOpenChange={(o) => !o && setAdminAction(null)}
        title={adminAction?.kind === "unlock" ? "解锁纳税所属期" : "恢复锁定"}
        description={
          adminAction?.kind === "unlock"
            ? `解锁纳税所属期 ${periodLabel(adminAction.run.filingPeriod)} 后，将新开一个补充批次。已确认明细保持锁定，只能新增。`
            : "没有新增明细时可直接恢复锁定，补充批次将被撤销。"
        }
        confirmText={adminAction?.kind === "unlock" ? "解锁" : "恢复锁定"}
        onSubmit={async (reason) => {
          if (!adminAction) return;
          try {
            if (adminAction.kind === "unlock") await twApi.unlockRun(adminAction.run.id, reason);
            else await twApi.relockRun(adminAction.run.id, reason);
            toast.success(adminAction.kind === "unlock" ? "已解锁" : "已恢复锁定");
            setAdminAction(null);
            void reload();
          } catch (e) {
            toast.error((e as Error).message);
          }
        }}
      />

      <Dialog open={!!events} onOpenChange={(o) => !o && setEvents(null)}>
        <DialogContent className="sm:max-w-2xl">
          <DialogHeader>
            <DialogTitle>操作日志 · 纳税所属期 {periodLabel(events?.run.filingPeriod)}</DialogTitle>
          </DialogHeader>
          <ol className="max-h-[60vh] space-y-1 overflow-auto text-sm">
            {events?.list.map((e) => (
              <li key={e.id} className="grid grid-cols-[150px_70px_80px_1fr] gap-2 border-b py-1">
                <span className="tabular-nums text-muted-foreground">{e.at}</span>
                <span>{e.event}</span>
                <span>{e.actor}</span>
                <span className="break-all">{e.detail}</span>
              </li>
            ))}
          </ol>
        </DialogContent>
      </Dialog>
    </div>
  );
}
