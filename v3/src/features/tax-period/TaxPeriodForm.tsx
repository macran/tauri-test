// 进项税转出 / 印花税 两个匹配页共用的表单外壳：
// 纳税所属期 + 来源期间（由页面传入）+ 布局 + 重置 / 匹配。
// 按钮沿用首页风格（brand / glass），小圆角；重置、匹配都带图标和文字。

import { ArrowCounterClockwise, CircleNotch, Intersect } from "@phosphor-icons/react";
import { type ReactNode, useId } from "react";

import { AppButton } from "@/components/app-button";
import { Label } from "@/components/ui/label";
import { Win11DatePicker } from "@/components/ui/win11-date-picker";
import { LayoutSelect } from "@/features/query/LayoutSelect";
import type { TableLayouts } from "@/features/query/table-layout";
import { cn } from "@/lib/utils";

import type { FieldErrors } from "./schema";

/** "202610" <-> Date(2026-10-01)；税务期间在后端始终是 YYYYMM 字符串。 */
const periodToDate = (period: string) =>
  new Date(Number(period.slice(0, 4)), Number(period.slice(4, 6)) - 1, 1);

const dateToPeriod = (date: Date) =>
  `${date.getFullYear()}${String(date.getMonth() + 1).padStart(2, "0")}`;

export function Field({
  label,
  error,
  children,
  wide = false,
}: {
  label: string;
  error?: string;
  children: ReactNode;
  /** 需要容纳两个控件（如「起 至 止」期间）时占两列，避免被挤出列宽 */
  wide?: boolean;
}) {
  const labelId = useId();
  return (
    // 控件由调用方传入，没法统一设 id：用 group + aria-labelledby 给整组一个名字（日期控件另有自己的 aria-label）
    <div role="group" aria-labelledby={labelId} className={cn("flex flex-col gap-1", wide && "col-span-2")}>
      <Label id={labelId} className="text-xs text-muted-foreground">{label}</Label>
      {children}
      {/* 固定一行高度：错误出现/消失时表单不跳动；过长的错误在列内截断，完整文字放 title */}
      <p
        role={error ? "alert" : undefined}
        title={error}
        className={cn("h-4 truncate text-xs leading-4 text-destructive", !error && "invisible")}
      >
        {error ?? "-"}
      </p>
    </div>
  );
}

export function TaxPeriodField({
  value,
  onChange,
  error,
  label = "税务期间",
}: {
  value: string;
  onChange: (period: string) => void;
  error?: string;
  label?: string;
}) {
  return (
    <Field label={label} error={error}>
      <Win11DatePicker
        precision="month"
        clearable={false}
        aria-label={label}
        aria-invalid={!!error}
        value={periodToDate(value)}
        onChange={(d) => d && onChange(dateToPeriod(d))}
        className="w-full"
      />
    </Field>
  );
}

/** 布局下拉：与其他字段同构（标签 + 控件 + 错误行占位），控件行才能与相邻字段、按钮共线 */
export function LayoutField({ layouts }: { layouts: TableLayouts }) {
  return (
    <Field label="布局">
      <LayoutSelect layouts={layouts} hideLabel />
    </Field>
  );
}

export function TaxPeriodForm({
  children,
  running,
  onRun,
  onReset,
  runLabel = "匹配",
  runDisabled = false,
  runDisabledReason,
  runIcon,
  errors,
  extraActions,
}: {
  /** 税务期间、过滤条件、布局等字段 */
  children: ReactNode;
  running: boolean;
  onRun: () => void;
  onReset: () => void;
  runLabel?: string;
  /** 表单未填完整等原因导致不能提交 */
  runDisabled?: boolean;
  /** 不能提交的原因，显示在按钮旁（只读批次等）；键盘和触屏看不到 title，所以要有可见文字 */
  runDisabledReason?: string;
  /** 默认是匹配图标；查询页传自己的 */
  runIcon?: ReactNode;
  /** 与字段无关的整体错误 */
  errors?: FieldErrors;
  /** 导出等附加按钮，排在重置前 */
  extraActions?: ReactNode;
}) {
  return (
    <form
      className="flex flex-col gap-1"
      onSubmit={(e) => {
        e.preventDefault();
        onRun();
      }}
    >
      {/* 字段按列网格排布：窗口变窄时整列换行，列宽不变，不会出现挤压或错位。
          操作按钮与字段同一行靠右，放不下时整组落到下一行；pb-4 对齐字段下方预留的错误行。 */}
      <div className="flex flex-wrap items-end gap-x-3 gap-y-1">
        <div className="grid min-w-0 grow grid-cols-[repeat(auto-fill,minmax(9rem,12rem))] gap-x-3">{children}</div>
        <div className="ml-auto flex items-center gap-2 pb-4">
          {runDisabledReason && <span className="text-xs text-muted-foreground">{runDisabledReason}</span>}
          {extraActions}
          <AppButton type="button" variant="glass" onClick={onReset} disabled={running}>
            <ArrowCounterClockwise data-icon="inline-start" />
            重置
          </AppButton>
          <AppButton type="submit" variant="brand" disabled={running || runDisabled}>
            {/* 文字不变、只换图标：按钮宽度恒定 */}
            {running ? (
              <CircleNotch data-icon="inline-start" className="animate-spin" />
            ) : (
              (runIcon ?? <Intersect data-icon="inline-start" weight="bold" />)
            )}
            {runLabel}
          </AppButton>
        </div>
      </div>
      {errors?.form && <p className="text-xs text-destructive">{errors.form}</p>}
    </form>
  );
}
