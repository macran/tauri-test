import { useEffect, useState } from "react";
import { Label } from "@/components/ui/label";
import { cn } from "@/lib/utils";
import { monthToPeriod, periodToMonth } from "../format";
import type { PeriodRange } from "../types";

interface Props {
  label: string;
  value: PeriodRange;
  onChange: (v: PeriodRange) => void;
  disabled?: boolean;
  className?: string;
  /** 最早允许选择的起始月（YYYYMM），用于递增批次 */
  minPeriod?: string;
  /** 允许的最大跨度月数，含起止月；所属期设为 3 */
  maxSpanMonths?: number;
}

/** 单期 / 跨期 选择。值为 YYYYMM */
export function PeriodRangeInput({ label, value, onChange, disabled, className, minPeriod, maxSpanMonths }: Props) {
  const [cross, setCross] = useState(value.from !== value.to);
  useEffect(() => {
    if (value.from !== value.to) setCross(true);
  }, [value.from, value.to]);

  const addMonths = (period: string, count: number) => {
    const year = Number(period.slice(0, 4));
    const month = Number(period.slice(4, 6)) - 1 + count;
    const y = year + Math.floor(month / 12);
    const m = ((month % 12) + 12) % 12 + 1;
    return `${y}${String(m).padStart(2, "0")}`;
  };
  const maxTo = maxSpanMonths ? addMonths(value.from, maxSpanMonths - 1) : undefined;
  const setFrom = (m: string) => {
    const candidate = monthToPeriod(m);
    if (!candidate) return;
    const from = minPeriod && candidate < minPeriod ? minPeriod : candidate;
    const hardMax = maxSpanMonths ? addMonths(from, maxSpanMonths - 1) : undefined;
    let to = cross && value.to >= from ? value.to : from;
    if (hardMax && to > hardMax) to = hardMax;
    onChange({ from, to });
  };
  const setTo = (m: string) => {
    let to = monthToPeriod(m);
    if (!to) return;
    if (to < value.from) to = value.from;
    if (maxTo && to > maxTo) to = maxTo;
    onChange({ from: value.from, to });
  };
  const toggle = (c: boolean) => {
    setCross(c);
    if (!c) onChange({ from: value.from, to: value.from });
  };

  const monthCls =
    "h-8 rounded-md border bg-background px-2 text-sm tabular-nums focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-50";

  return (
    <div className={cn("flex flex-wrap items-center gap-2", className)}>
      <Label className="text-sm text-muted-foreground">{label}</Label>
      <div className="inline-flex rounded-md border p-0.5 text-xs" role="radiogroup" aria-label={`${label}类型`}>
        {[
          { v: false, t: "单期" },
          { v: true, t: "跨期" },
        ].map((o) => (
          <button
            key={o.t}
            type="button"
            role="radio"
            aria-checked={cross === o.v}
            disabled={disabled}
            onClick={() => toggle(o.v)}
            className={cn(
              "rounded px-2 py-1 transition-colors",
              cross === o.v ? "bg-primary text-primary-foreground" : "text-muted-foreground hover:bg-muted",
            )}
          >
            {o.t}
          </button>
        ))}
      </div>
      <input
        type="month"
        aria-label={`${label}起`}
        className={monthCls}
        value={periodToMonth(value.from)}
        min={minPeriod ? periodToMonth(minPeriod) : undefined}
        onChange={(e) => setFrom(e.target.value)}
        disabled={disabled}
      />
      {cross && (
        <>
          <span className="text-muted-foreground">至</span>
          <input
            type="month"
            aria-label={`${label}止`}
            className={monthCls}
            value={periodToMonth(value.to)}
            min={periodToMonth(value.from)}
            max={maxTo ? periodToMonth(maxTo) : undefined}
            onChange={(e) => setTo(e.target.value)}
            disabled={disabled}
          />
        </>
      )}
    </div>
  );
}
