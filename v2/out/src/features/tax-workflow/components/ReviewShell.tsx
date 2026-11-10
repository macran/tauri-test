import { type ReactNode, useCallback, useEffect, useMemo, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { cn } from "@/lib/utils";
import { pickSavePath, twApi } from "../api";
import { currentPeriod, fen, periodLabel } from "../format";

function nextMonth(p: string) { const y = Number(p.slice(0,4)); const m = Number(p.slice(4,6)); return m === 12 ? `${y+1}01` : `${y}${String(m+1).padStart(2,"0")}`; }
import type {
  ApplyResult,
  BaseLine,
  ConfirmInput,
  ExportResult,
  LineStats,
  MatchSummary,
  PeriodRange,
  TaxModule,
  TaxRun,
} from "../types";
import { RunStatusBadge } from "./Badges";
import { ConfirmRunDialog } from "./ConfirmRunDialog";
import { PeriodRangeInput } from "./PeriodRangeInput";
import { type Column, VirtualTable } from "./VirtualTable";

export type CommonAction =
  | { type: "include" }
  | { type: "exclude" }
  | { type: "acceptSystem" }
  | { type: "restore" }
  | { type: "setRemark" };

export interface ReviewCtx<L, A> {
  run: TaxRun | null;
  editable: boolean;
  apply: (ids: number[], action: A | CommonAction, remark?: string | null) => Promise<void>;
  selectedIds: number[];
  lines: L[];
}

type FilterKey = "all" | "included" | "pending" | "review" | "invalid" | "manual" | "nohit" | "locked" | "novoucher";

export interface ReviewShellProps<L extends BaseLine, A> {
  module: TaxModule;
  moduleLabel: string;
  srcLabel: string;
  includeWord: string; // 转出 / 计入
  taxLabel: string;
  filePrefix: string;
  openRun?: TaxRun | null;
  match: (filingPeriod: PeriodRange, sourcePeriod: PeriodRange) => Promise<MatchSummary>;
  load: (runId: number) => Promise<{ run: TaxRun; lines: L[]; stats: LineStats }>;
  applyOverride: (runId: number, ids: number[], action: A | CommonAction, remark?: string | null) => Promise<ApplyResult>;
  exportPending: (runId: number, path: string) => Promise<ExportResult>;
  confirm: (input: ConfirmInput) => Promise<TaxRun>;
  columns: (ctx: ReviewCtx<L, A>) => Column<L>[];
  lineTax: (l: L) => number;
  /** 模块特有的批量操作（如：指定印花税子目） */
  extraBulk?: (ctx: ReviewCtx<L, A>) => ReactNode;
  /** 需要显示的筛选项 */
  filters: FilterKey[];
  hasVoucher?: (l: L) => boolean;
  invalid?: (l: L) => boolean;
  onChanged?: () => void;
}

export function ReviewShell<L extends BaseLine, A>(p: ReviewShellProps<L, A>) {
  const [filingRange, setFilingRange] = useState<PeriodRange>(() => {
    const prev = currentPeriod(-1);
    return { from: prev, to: prev };
  });
  const [range, setRange] = useState<PeriodRange>(() => {
    const prev = currentPeriod(-1);
    return { from: prev, to: prev };
  });
  const [latestFilingEnd, setLatestFilingEnd] = useState<string | null>(null);
  const [run, setRun] = useState<TaxRun | null>(null);
  const [lines, setLines] = useState<L[]>([]);
  const [stats, setStats] = useState<LineStats | null>(null);
  const [busy, setBusy] = useState<string | null>(null);
  const [filter, setFilter] = useState<FilterKey>("all");
  const [keyword, setKeyword] = useState("");
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [remark, setRemark] = useState("");
  const [confirmOpen, setConfirmOpen] = useState(false);

  const loadRun = useCallback(
    async (runId: number) => {
      const payload = await p.load(runId);
      setRun(payload.run);
      setLines(payload.lines);
      setStats(payload.stats);
      setSelected(new Set());
    },
    [p.load],
  );

  // 纳税所属期是批次入口；后续新批次必须从最近已建立所属期的下一月开始。
  useEffect(() => {
    void twApi.listRuns(p.module).then((runs) => {
      const ends = runs.map((r) => r.filingPeriod?.to).filter((v): v is string => !!v).sort();
      setLatestFilingEnd(ends.at(-1) ?? null);
    }).catch((e) => toast.error((e as Error).message));
  }, [p.module, p.openRun]);

  // 从所属期批次列表点进来；恢复该批次最近一次选过的来源期间。
  useEffect(() => {
    if (p.openRun) {
      if (p.openRun.filingPeriod) setFilingRange(p.openRun.filingPeriod);
      const lastSource = p.openRun.srcPeriods.at(-1);
      if (lastSource) setRange(lastSource);
      void loadRun(p.openRun.id).catch((e) => toast.error((e as Error).message));
    }
  }, [p.openRun, loadRun]);

  useEffect(() => {
    if (p.openRun || !latestFilingEnd || run) return;
    if (filingRange.from <= latestFilingEnd) {
      const next = nextMonth(latestFilingEnd);
      setFilingRange({ from: next, to: next });
    }
  }, [latestFilingEnd, p.openRun, run, filingRange.from]);

  const editable = !!run && run.status !== "confirmed";

  const runBusy = async (label: string, fn: () => Promise<void>) => {
    setBusy(label);
    try {
      await fn();
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setBusy(null);
    }
  };

  const doMatch = () =>
    runBusy("match", async () => {
      const s = await p.match(filingRange, range);
      toast.success(
        `规则 ${s.ruleCount} 条，候选明细 ${s.candidateCount} 行，命中 ${s.hitCount} 行；跳过 人工 ${s.skippedManual} / 已确认 ${s.skippedConfirmed} / 其它期间 ${s.skippedOtherRun}`,
      );
      await loadRun(s.run.id);
      setLatestFilingEnd((prev) => !prev || filingRange.to > prev ? filingRange.to : prev);
      p.onChanged?.();
    });

  const doOpen = () =>
    runBusy("open", async () => {
      const r = await twApi.findRun(p.module, filingRange);
      if (!r) {
        toast.info(`${periodLabel(filingRange)} 尚无结果集；选择认证/凭证期间后点击“匹配”创建`);
        return;
      }
      if (r.filingPeriod) setFilingRange(r.filingPeriod);
      const lastSource = r.srcPeriods.at(-1);
      if (lastSource) setRange(lastSource);
      await loadRun(r.id);
    });

  const apply = useCallback(
    async (ids: number[], action: A | CommonAction, rmk?: string | null) => {
      if (!run || ids.length === 0) return;
      const res = await p.applyOverride(run.id, ids, action, rmk ?? null);
      if (res.skippedLocked > 0) toast.warning(`${res.skippedLocked} 行已确认锁定，未修改`);
      await loadRun(run.id);
    },
    [run, p.applyOverride, loadRun],
  );

  const safeApply = (ids: number[], action: A | CommonAction, rmk?: string | null) =>
    runBusy("apply", () => apply(ids, action, rmk));

  const counts = useMemo(() => {
    const c: Record<FilterKey, number> = {
      all: lines.length,
      included: 0,
      pending: 0,
      review: 0,
      invalid: 0,
      manual: 0,
      nohit: 0,
      locked: 0,
      novoucher: 0,
    };
    for (const l of lines) {
      if (l.finalIncluded) c.included++;
      if (l.pending) c.pending++;
      if (l.needsReview) c.review++;
      if (p.invalid?.(l)) c.invalid++;
      if (l.matchType === "manual") c.manual++;
      if (!l.autoHit && !l.finalIncluded) c.nohit++;
      if (l.locked) c.locked++;
      if (p.hasVoucher && !p.hasVoucher(l)) c.novoucher++;
    }
    return c;
  }, [lines, p.invalid, p.hasVoucher]);

  const filtered = useMemo(() => {
    const kw = keyword.trim().toLowerCase();
    return lines.filter((l) => {
      switch (filter) {
        case "included":
          if (!l.finalIncluded) return false;
          break;
        case "pending":
          if (!l.pending) return false;
          break;
        case "review":
          if (!l.needsReview) return false;
          break;
        case "invalid":
          if (!p.invalid?.(l)) return false;
          break;
        case "manual":
          if (l.matchType !== "manual") return false;
          break;
        case "nohit":
          if (l.autoHit || l.finalIncluded) return false;
          break;
        case "locked":
          if (!l.locked) return false;
          break;
        case "novoucher":
          if (!p.hasVoucher || p.hasVoucher(l)) return false;
          break;
      }
      if (!kw) return true;
      return [l.invoiceNo, l.invoiceCode, l.sellerName, l.sellerTaxNo, l.goodsName, l.taxCode, l.voucherNos, l.hitRules, l.remark]
        .some((x) => x?.toLowerCase().includes(kw));
    });
  }, [lines, filter, keyword, p.invalid, p.hasVoucher]);

  const selectable = filtered.filter((l) => !l.locked);
  const allSelected = selectable.length > 0 && selectable.every((l) => selected.has(l.detailId));
  const selectedIds = [...selected];

  const ctx: ReviewCtx<L, A> = { run, editable, apply: (ids, a, r) => runBusy("apply", () => apply(ids, a, r)), selectedIds, lines };

  const selectCol: Column<L> = {
    key: "_sel",
    sticky: true,
    width: 36,
    align: "center",
    header: (
      <input
        type="checkbox"
        aria-label="全选当前筛选结果"
        disabled={!editable}
        checked={allSelected}
        onChange={() =>
          setSelected(allSelected ? new Set() : new Set(selectable.map((l) => l.detailId)))
        }
      />
    ),
    cell: (l) => (
      <input
        type="checkbox"
        aria-label={`选择 ${l.invoiceNo} 第${l.lineNo}行`}
        disabled={!editable || l.locked}
        checked={selected.has(l.detailId)}
        onChange={() =>
          setSelected((s) => {
            const n = new Set(s);
            n.has(l.detailId) ? n.delete(l.detailId) : n.add(l.detailId);
            return n;
          })
        }
      />
    ),
  };

  const filterLabels: Record<FilterKey, string> = {
    all: "全部",
    included: `${p.includeWord}`,
    pending: "本批待确认",
    review: "待复核",
    invalid: "缺少子目",
    manual: "人工修改",
    nohit: "未命中",
    locked: "已确认",
    novoucher: "无凭证",
  };

  const pendingBatch = run?.batches.find((b) => !b.confirmedAt);

  const doExport = () =>
    runBusy("export", async () => {
      if (!run) return;
      const path = await pickSavePath(`${p.filePrefix}_${periodLabel(run.filingPeriod)}_批次${pendingBatch?.batchNo ?? 1}.xlsx`);
      if (!path) return;
      const r = await p.exportPending(run.id, path);
      toast.success(`已导出 ${r.lineCount} 行，${p.taxLabel} ${fen(r.taxTotal)} 元。交会计做账后回填凭证信息确认。`);
      await loadRun(run.id);
      p.onChanged?.();
    });

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      {/* 期间 & 主操作 */}
      <div className="flex flex-wrap items-center gap-3">
        <div className="grid gap-1">
          <PeriodRangeInput
            label="纳税所属期"
            value={filingRange}
            maxSpanMonths={3}
            minPeriod={!run && latestFilingEnd ? nextMonth(latestFilingEnd) : undefined}
            onChange={(v) => { setFilingRange(v); setRun(null); setLines([]); setStats(null); }}
            disabled={!!busy}
          />
          <span className="pl-1 text-xs text-muted-foreground">
            {latestFilingEnd && !run
              ? `单期或跨期（最多3个月）。最近已建立至 ${latestFilingEnd}；新所属期须从 ${nextMonth(latestFilingEnd)} 起，中断月份可跳过。`
              : "单期或跨期（最多3个月）；年度内可以有中断月份。"}
          </span>
        </div>
        <PeriodRangeInput label={p.srcLabel} value={range} onChange={setRange} disabled={!!busy} />
        <Button onClick={doMatch} disabled={!!busy || run?.status === "confirmed"}>
          {busy === "match" ? "匹配中…" : "匹配 / 加入本所属期"}
        </Button>
        <Button variant="outline" onClick={doOpen} disabled={!!busy}>
          打开所属期结果集
        </Button>
        {run && (
          <div className="ml-auto flex items-center gap-2">
            <RunStatusBadge status={run.status} />
            <span className="text-sm text-muted-foreground">
              纳税所属期 {periodLabel(run.filingPeriod)}
              {pendingBatch && ` · 凭证批次 ${pendingBatch.batchNo}${pendingBatch.batchNo > 1 ? "（补充）" : ""}`}
            </span>
          </div>
        )}
      </div>

      {/* 统计 */}
      {stats && run && (
        <div className="grid grid-cols-2 gap-2 text-sm sm:grid-cols-4 lg:grid-cols-7">
          <Stat label="明细行" value={stats.totalLines} />
          <Stat label="机器命中" value={stats.autoHitCount} />
          <Stat label={`${p.includeWord}行 / ${p.taxLabel}`} value={`${stats.includedCount} / ${fen(stats.includedTax)}`} />
          <Stat label="本批待确认" value={`${stats.pendingCount} / ${fen(stats.pendingTax)}`} tone={stats.pendingCount ? "primary" : undefined} />
          <Stat label="待复核" value={stats.needsReviewCount} tone={stats.needsReviewCount ? "warn" : undefined} />
          {p.invalid && <Stat label="缺少子目" value={stats.invalidCount} tone={stats.invalidCount ? "error" : undefined} />}
          <Stat label="已确认锁定" value={`${stats.confirmedCount} / ${fen(stats.confirmedTax)}`} />
        </div>
      )}
      {stats?.exportStale && (
        <p role="status" className="rounded-md bg-amber-50 px-3 py-2 text-sm text-amber-900 dark:bg-amber-950 dark:text-amber-100">
          本批清单导出后结果又有修改，确认前需要重新导出并交会计核对。
        </p>
      )}
      {run?.status === "confirmed" && (
        <p className="rounded-md bg-muted px-3 py-2 text-sm text-muted-foreground">
          该纳税所属期结果集已确认锁定。若税务局核查需补税，请联系管理员在“纳税所属期批次”中解锁。
        </p>
      )}

      {/* 筛选 & 批量操作 */}
      {run && (
        <div className="flex flex-wrap items-center gap-2">
          <div className="flex flex-wrap gap-1" role="tablist" aria-label="筛选">
            {p.filters.map((k) => (
              <button
                key={k}
                role="tab"
                aria-selected={filter === k}
                onClick={() => setFilter(k)}
                className={cn(
                  "rounded-full border px-2.5 py-0.5 text-xs transition-colors",
                  filter === k ? "border-primary bg-primary text-primary-foreground" : "hover:bg-muted",
                )}
              >
                {filterLabels[k]} <span className="tabular-nums opacity-70">{counts[k]}</span>
              </button>
            ))}
          </div>
          <Input
            placeholder="搜索发票号 / 销方 / 商品 / 凭证 / 规则"
            value={keyword}
            onChange={(e) => setKeyword(e.target.value)}
            className="h-8 w-64"
            aria-label="搜索"
          />
          <div className="ml-auto flex gap-2">
            <Button variant="outline" size="sm" onClick={doExport} disabled={!editable || !!busy}>
              {busy === "export" ? "导出中…" : "导出本批清单"}
            </Button>
            <Button size="sm" onClick={() => setConfirmOpen(true)} disabled={!editable || !!busy}>
              回填凭证并确认
            </Button>
          </div>
        </div>
      )}

      {run && editable && selected.size > 0 && (
        <div className="flex flex-wrap items-center gap-2 rounded-md border bg-muted/40 px-3 py-2 text-sm" role="toolbar" aria-label="批量操作">
          <span className="font-medium">已选 {selected.size} 行</span>
          <Button size="sm" onClick={() => safeApply(selectedIds, { type: "include" }, remark || null)}>
            设为{p.includeWord}
          </Button>
          <Button size="sm" variant="outline" onClick={() => safeApply(selectedIds, { type: "exclude" }, remark || null)}>
            设为不{p.includeWord}
          </Button>
          <Button size="sm" variant="outline" onClick={() => safeApply(selectedIds, { type: "acceptSystem" }, remark || null)}>
            采纳机器结论(复核通过)
          </Button>
          <Button size="sm" variant="ghost" onClick={() => safeApply(selectedIds, { type: "restore" })} title="删除人工结论，重新匹配时会再次参与">
            恢复机器结论
          </Button>
          {p.extraBulk?.(ctx)}
          <Input
            className="h-8 w-56"
            placeholder="备注（随操作一起保存）"
            value={remark}
            onChange={(e) => setRemark(e.target.value)}
            aria-label="备注"
          />
          <Button size="sm" variant="ghost" onClick={() => safeApply(selectedIds, { type: "setRemark" }, remark || null)}>
            仅改备注
          </Button>
          <Button size="sm" variant="ghost" onClick={() => setSelected(new Set())}>
            取消选择
          </Button>
        </div>
      )}

      {run ? (
        <VirtualTable
          className="min-h-[320px] flex-1"
          rows={filtered}
          columns={[selectCol, ...p.columns(ctx)]}
          rowKey={(l) => l.detailId}
          rowClassName={(l) =>
            cn(
              l.locked && "bg-emerald-50/50 dark:bg-emerald-950/20",
              !l.locked && l.needsReview && "bg-amber-50/70 dark:bg-amber-950/20",
              !l.locked && p.invalid?.(l) && "bg-red-50/60 dark:bg-red-950/20",
            )
          }
          empty="没有符合条件的明细"
        />
      ) : (
        <div className="flex flex-1 items-center justify-center rounded-md border border-dashed p-10 text-sm text-muted-foreground">
          先选纳税所属期，再选{p.srcLabel}并点击“匹配 / 加入本所属期”。系统按规则匹配并把该范围全部发票明细列入当前所属期结果集；可继续追加其他来源期间。人工结论不会被再次匹配覆盖。
        </div>
      )}

      <ConfirmRunDialog
        open={confirmOpen}
        onOpenChange={setConfirmOpen}
        run={run}
        stats={stats}
        moduleLabel={p.moduleLabel}
        taxLabel={p.taxLabel}
        onSubmit={async (input) => {
          try {
            const r = await p.confirm(input);
            toast.success(`纳税所属期 ${periodLabel(r.filingPeriod)} 已确认并锁定`);
            setConfirmOpen(false);
            await loadRun(r.id);
            p.onChanged?.();
          } catch (e) {
            toast.error((e as Error).message);
          }
        }}
      />
    </div>
  );
}

function Stat({ label, value, tone }: { label: string; value: ReactNode; tone?: "primary" | "warn" | "error" }) {
  return (
    <div
      className={cn(
        "rounded-md border px-3 py-2",
        tone === "primary" && "border-primary/40",
        tone === "warn" && "border-amber-400 bg-amber-50 dark:bg-amber-950/30",
        tone === "error" && "border-red-400 bg-red-50 dark:bg-red-950/30",
      )}
    >
      <div className="text-xs text-muted-foreground">{label}</div>
      <div className="font-semibold tabular-nums">{value}</div>
    </div>
  );
}
