import { cn } from "@/lib/utils";
import { matchTypeLabel, runStatusLabel } from "../format";
import type { MatchType, RunStatus } from "../types";

const base = "inline-flex items-center rounded px-1.5 py-0.5 text-xs font-medium whitespace-nowrap";

export function RunStatusBadge({ status }: { status: RunStatus }) {
  const cls = {
    open: "bg-sky-100 text-sky-800 dark:bg-sky-950 dark:text-sky-200",
    confirmed: "bg-emerald-100 text-emerald-800 dark:bg-emerald-950 dark:text-emerald-200",
    reopened: "bg-amber-100 text-amber-800 dark:bg-amber-950 dark:text-amber-200",
  }[status];
  return <span className={cn(base, cls)}>{runStatusLabel[status]}</span>;
}

export function MatchTypeBadge({ type, locked }: { type: MatchType; locked?: boolean }) {
  const cls = {
    autoHit: "bg-blue-50 text-blue-700 dark:bg-blue-950 dark:text-blue-200",
    noHit: "text-muted-foreground",
    reviewed: "bg-teal-50 text-teal-700 dark:bg-teal-950 dark:text-teal-200",
    manual: "bg-orange-100 text-orange-800 dark:bg-orange-950 dark:text-orange-200",
  }[type];
  return (
    <span className={cn(base, cls)}>
      {matchTypeLabel[type]}
      {locked && <span className="ml-1 text-[10px]" aria-label="已确认锁定">已锁</span>}
    </span>
  );
}

export function FlagBadge({ tone, children }: { tone: "warn" | "error"; children: React.ReactNode }) {
  return (
    <span
      className={cn(
        base,
        tone === "warn"
          ? "bg-amber-100 text-amber-800 dark:bg-amber-950 dark:text-amber-200"
          : "bg-red-100 text-red-700 dark:bg-red-950 dark:text-red-200",
      )}
    >
      {children}
    </span>
  );
}
