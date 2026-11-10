import { Checkbox } from "@/components/ui/checkbox";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";

const now = new Date();
const CURRENT_YEAR = now.getFullYear();

const YEAR_OPTIONS = Array.from(
  { length: 6 },
  (_, index) => CURRENT_YEAR - 4 + index
);

const MONTH_OPTIONS = Array.from({ length: 12 }, (_, index) =>
  String(index + 1).padStart(2, "0")
);

type PeriodSelectProps = {
  value: string;
  label: string;
  onChange: (value: string) => void;
};

function PeriodSelect({
  value,
  label,
  onChange,
}: PeriodSelectProps) {
  const year = value.slice(0, 4);
  const month = value.slice(4, 6);

  return (
    <div className="flex items-center gap-1">
      <Select
        value={year}
        onValueChange={(nextYear) => {
          onChange(`${nextYear}${month}`);
        }}
      >
        <SelectTrigger className="w-24" aria-label={`${label}年份`}>
          <SelectValue placeholder="年份" />
        </SelectTrigger>

        <SelectContent>
          {YEAR_OPTIONS.map((option) => (
            <SelectItem key={option} value={String(option)}>
              {option}年
            </SelectItem>
          ))}
        </SelectContent>
      </Select>

      <Select
        value={month}
        onValueChange={(nextMonth) => {
          onChange(`${year}${nextMonth}`);
        }}
      >
        <SelectTrigger className="w-20" aria-label={`${label}月份`}>
          <SelectValue placeholder="月份" />
        </SelectTrigger>

        <SelectContent>
          {MONTH_OPTIONS.map((option) => (
            <SelectItem key={option} value={option}>
              {Number(option)}月
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
    </div>
  );
}

type PeriodRangePickerProps = {
  periodFrom: string;
  periodTo: string;

  /**
   * 是否启用期间区间。
   *
   * 不能通过 periodFrom !== periodTo 推断，
   * 因为用户刚勾选区间时两个期间可能暂时相同。
   */
  rangeEnabled: boolean;

  onRangeEnabledChange: (enabled: boolean) => void;
  onChange: (periodFrom: string, periodTo: string) => void;
};

/**
 * 单期：
 *   rangeEnabled = false
 *   例如 202601
 *
 * 跨期间：
 *   rangeEnabled = true
 *   例如 202601 至 202603
 */
export function PeriodRangePicker({
  periodFrom,
  periodTo,
  rangeEnabled,
  onRangeEnabledChange,
  onChange,
}: PeriodRangePickerProps) {
  function handleFromChange(nextPeriod: string) {
    if (rangeEnabled) {
      // 跨期间时，只修改起始期间。
      onChange(nextPeriod, periodTo);
    } else {
      // 单期间时，起止期间始终保持一致。
      onChange(nextPeriod, nextPeriod);
    }
  }

  function handleRangeToggle(checked: boolean) {
    onRangeEnabledChange(checked);
  }

  return (
    <div className="flex flex-wrap items-center gap-3">
      <PeriodSelect
        value={periodFrom}
        label="起始期间"
        onChange={handleFromChange}
      />

      <div className="flex items-center gap-2">
        <Checkbox
          id="input-tax-transfer-range"
          checked={rangeEnabled}
          onCheckedChange={(checked) => {
            handleRangeToggle(checked === true);
          }}
        />

        <Label
          htmlFor="input-tax-transfer-range"
          className="text-sm font-normal"
        >
          跨多个期间
        </Label>
      </div>

      {rangeEnabled && (
        <>
          <span className="text-sm text-muted-foreground">至</span>

          <PeriodSelect
            value={periodTo}
            label="结束期间"
            onChange={(nextPeriod) => {
              onChange(periodFrom, nextPeriod);
            }}
          />
        </>
      )}
    </div>
  );
}