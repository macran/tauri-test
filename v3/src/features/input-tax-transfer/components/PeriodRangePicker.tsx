import { Win11DatePicker } from "@/components/ui/win11-date-picker";

/** "202601" <-> Date(2026-01-01)；期间在后端始终是 YYYYMM 字符串。 */
const toDate = (period: string) =>
  new Date(Number(period.slice(0, 4)), Number(period.slice(4, 6)) - 1, 1);

const toPeriod = (date: Date) =>
  `${date.getFullYear()}${String(date.getMonth() + 1).padStart(2, "0")}`;

function PeriodSelect({
  value,
  label,
  onChange,
}: {
  value: string;
  label: string;
  onChange: (value: string) => void;
}) {
  return (
    <Win11DatePicker
      precision="month"
      clearable={false}
      aria-label={label}
      value={toDate(value)}
      onChange={(d) => d && onChange(toPeriod(d))}
      className="w-32"
    />
  );
}

type PeriodRangePickerProps = {
  periodFrom: string;
  periodTo: string;
  onChange: (periodFrom: string, periodTo: string) => void;
};

/**
 * 始终显示「起 至 止」两个期间；起止相同就是单期，不同就是跨期，没有单独的开关。
 * 联动规则（让单期用户仍然只操作一个框）：
 *   - 改起始：原来是单期，或新起始晚于结束 → 结束跟随起始；原来已是区间且仍有效 → 结束不动。
 *   - 改结束：只改结束；早于起始的组合由 zod（matchFormSchema）报「结束期间不能早于起始期间」。
 */
export function PeriodRangePicker({ periodFrom, periodTo, onChange }: PeriodRangePickerProps) {
  function handleFromChange(next: string) {
    const wasSingle = periodFrom === periodTo;
    onChange(next, wasSingle || next > periodTo ? next : periodTo);
  }

  return (
    <div className="flex items-center gap-2">
      <PeriodSelect value={periodFrom} label="起始期间" onChange={handleFromChange} />
      <span className="shrink-0 text-xs text-muted-foreground">至</span>
      <PeriodSelect value={periodTo} label="结束期间" onChange={(next) => onChange(periodFrom, next)} />
    </div>
  );
}
