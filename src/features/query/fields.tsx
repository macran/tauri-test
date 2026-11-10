// src/components/query/fields.tsx
//
// 查询表单字段的通用渲染组件。所有查询页面的 form.Field 渲染都复用
// 这几个组件，保证排版、间距、日期组件一致。

import type { AnyFieldApi } from "@tanstack/react-form";
import { XIcon } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { RadioGroup, RadioGroupItem } from "@/components/ui/radio-group";
import { cn } from "@/lib/utils";
import { OptionCombobox } from "@/components/ui/option-combobox";
import { Win11DatePicker } from "@/components/ui/win11-date-picker";
import type { EnumOption } from "@/lib/enum-options";

export function parseLocalDate(value: string | undefined | null): Date | undefined {
  if (!value) return undefined;
  const [y, m, d] = value.split("-").map(Number);
  if (!y || !m || !d) return undefined;
  return new Date(y, m - 1, d);
}

export function toIsoDate(date: Date | undefined): string {
  if (!date) return "";
  const y = date.getFullYear();
  const m = String(date.getMonth() + 1).padStart(2, "0");
  const d = String(date.getDate()).padStart(2, "0");
  return `${y}-${m}-${d}`;
}

/** 日期范围默认值：今天往前 2 年 ~ 今天（本地时区，不用 toISOString 避免差一天）。 */
export function defaultDateRange(): { from: string; to: string } {
  const today = new Date();
  const from = new Date(today.getFullYear() - 2, today.getMonth(), today.getDate());
  return { from: toIsoDate(from), to: toIsoDate(today) };
}

/** 期间默认值：当月，格式与 Win11DatePicker 的 month 精度一致（yyyy-MM-01 经 toIsoDate 归一）。 */
export function defaultMonth(): string {
  const today = new Date();
  return toIsoDate(new Date(today.getFullYear(), today.getMonth(), 1));
}

export function FieldShell({
  label,
  children,
  wide = false,
}: {
  label: string;
  children: React.ReactNode;
  /** 日期范围等需要容纳两个输入框的字段 */
  wide?: boolean;
}) {
  return (
    // 固定宽度而非等分网格：宽度够就同排放下，不够换行，输入框不会被压扁
    <div className={cn("flex min-w-0 max-w-full shrink-0 flex-col gap-1", wide ? "w-[300px]" : "w-[150px]")}>
      <Label className="text-muted-foreground">{label}</Label>
      {children}
    </div>
  );
}

export function ClearableInput({
  onClear,
  className,
  ...props
}: React.ComponentProps<"input"> & { onClear: () => void }) {
  const hasValue = props.value !== "" && props.value != null;
  return (
    // className 作用在外层容器上（间距等布局类），输入框自己只管给清除按钮留位置
    <div className={cn("relative", className)}>
      <Input {...props} className={hasValue ? "pr-7" : undefined} />
      {hasValue && (
        <button
          type="button"
          aria-label="清除"
          onClick={onClear}
          className="absolute top-1/2 right-2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
        >
          <XIcon className="size-3.5" />
        </button>
      )}
    </div>
  );
}

export function TextField({
  field,
  label,
  placeholder,
}: {
  field: AnyFieldApi;
  label: string;
  placeholder?: string;
}) {
  return (
    <FieldShell label={label}>
      <ClearableInput
        value={field.state.value ?? ""}
        onBlur={() => {
          trimField(field);
          field.handleBlur();
        }}
        // 回车提交时输入框还没失焦，这里先去掉前后空格
        onKeyDown={(e) => e.key === "Enter" && trimField(field)}
        onChange={(e) => field.handleChange(e.target.value)}
        onClear={() => field.handleChange("")}
        placeholder={placeholder}
      />
    </FieldShell>
  );
}

function trimField(field: AnyFieldApi) {
  const v = field.state.value;
  if (typeof v === "string" && v !== v.trim()) field.handleChange(v.trim());
}

export function NumberField({
  field,
  label,
  placeholder,
}: {
  field: AnyFieldApi;
  label: string;
  placeholder?: string;
}) {
  return (
    <FieldShell label={label}>
      <ClearableInput
        type="number"
        value={field.state.value ?? ""}
        onBlur={field.handleBlur}
        onChange={(e) =>
          field.handleChange(e.target.value === "" ? null : Number(e.target.value))
        }
        onClear={() => field.handleChange(null)}
        placeholder={placeholder}
      />
    </FieldShell>
  );
}

export function SelectField({
  field,
  label,
  options,
  placeholder = "全部",
}: {
  field: AnyFieldApi;
  label: string;
  options: EnumOption[];
  placeholder?: string;
}) {
  return (
    <FieldShell label={label}>
      <OptionCombobox
        options={options}
        value={field.state.value ?? []}
        onChange={field.handleChange}
        placeholder={placeholder}
      />
    </FieldShell>
  );
}

/** 单选组（radio）：选项少且必须选一个，如查询对象。点圆点或文字都能选中。 */
export function RadioField<V extends string | number>({
  label,
  value,
  onChange,
  options,
  wide = false,
}: {
  label: string;
  value: V;
  onChange: (v: V) => void;
  options: { value: V; label: string }[];
  wide?: boolean;
}) {
  return (
    <FieldShell label={label} wide={wide}>
      <RadioGroup
        value={value}
        onValueChange={(v) => onChange(v as V)}
        aria-label={label}
        className="flex h-(--control-h) w-full flex-row items-center gap-4"
      >
        {options.map((o) => (
          <Label key={o.value} className="flex cursor-pointer items-center gap-1.5 whitespace-pre-line text-xs">
            <RadioGroupItem value={o.value} />
            {o.label}
          </Label>
        ))}
      </RadioGroup>
    </FieldShell>
  );
}

export function DateField({
  field,
  label,
  precision = "day",
}: {
  field: AnyFieldApi;
  label: string;
  precision?: "day" | "month";
}) {
  return (
    <FieldShell label={label}>
      <Win11DatePicker
        precision={precision}
        value={parseLocalDate(field.state.value)}
        onChange={(d) => field.handleChange(toIsoDate(d))}
      />
    </FieldShell>
  );
}

export function DateRangeField({
  fromField,
  toField,
  label,
}: {
  fromField: AnyFieldApi;
  toField: AnyFieldApi;
  label: string;
}) {
  return (
    <FieldShell label={label} wide>
      <div className="flex items-center gap-1.5">
        <Win11DatePicker
          value={parseLocalDate(fromField.state.value)}
          onChange={(d) => fromField.handleChange(toIsoDate(d))}
        />
        <span className="shrink-0 text-xs text-muted-foreground">至</span>
        <Win11DatePicker
          value={parseLocalDate(toField.state.value)}
          onChange={(d) => toField.handleChange(toIsoDate(d))}
        />
      </div>
    </FieldShell>
  );
}
