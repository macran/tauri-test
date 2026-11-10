import type { MatchType, PeriodRange, RunStatus } from "./types";

const yuanFmt = new Intl.NumberFormat("zh-CN", { minimumFractionDigits: 2, maximumFractionDigits: 2 });

/** 分 → "1,234.56" */
export const fen = (v: number | null | undefined) => (v == null ? "" : yuanFmt.format(v / 100));

/** "1,234.56" / "1234.5" → 分；非法返回 null */
export function parseYuanToFen(s: string): number | null {
  const t = s.replace(/[,，\s]/g, "");
  if (!/^\d+(\.\d{0,2})?$/.test(t)) return null;
  const [i, d = ""] = t.split(".");
  return Number(i) * 100 + Number((d + "00").slice(0, 2));
}

/** 百万分之 → "万分之三(0.03%)" 之类的可读形式 */
export function ratePpmLabel(ppm: number | null | undefined): string {
  if (ppm == null) return "";
  const pct = ppm / 10000;
  return `${pct.toFixed(pct < 0.1 ? 3 : 2).replace(/0+$/, "").replace(/\.$/, "")}%`;
}

export const periodLabel = (p: PeriodRange | null | undefined) =>
  !p ? "" : p.from === p.to ? p.from : `${p.from}-${p.to}`;

/** "2026-10" ⇄ "202610" */
export const monthToPeriod = (m: string) => m.replace("-", "");
export const periodToMonth = (p: string) => (p.length === 6 ? `${p.slice(0, 4)}-${p.slice(4)}` : "");

export function currentPeriod(offsetMonths = 0): string {
  const d = new Date();
  d.setDate(1);
  d.setMonth(d.getMonth() + offsetMonths);
  return `${d.getFullYear()}${String(d.getMonth() + 1).padStart(2, "0")}`;
}

export const matchTypeLabel: Record<MatchType, string> = {
  autoHit: "机器匹配",
  noHit: "未命中",
  reviewed: "机器匹配(已复核)",
  manual: "人工修改",
};

export const runStatusLabel: Record<RunStatus, string> = {
  open: "未确认",
  confirmed: "已确认锁定",
  reopened: "已解锁(补充中)",
};

export const todayIso = () => {
  const d = new Date();
  return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
};
