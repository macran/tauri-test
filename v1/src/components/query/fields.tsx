// src/components/query/fields.tsx
//
// 查询表单字段的通用渲染组件。所有查询页面的 form.Field 渲染都复用
// 这几个组件，保证排版、间距、日期组件一致。

import type { AnyFieldApi } from "@tanstack/react-form";
import { XIcon } from "lucide-react";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
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

export function FieldShell({
  label,
  children,
}: {
  label: string;
  children: React.ReactNode;
}) {
  return (
    <div className="flex flex-col gap-1">
      <Label className="text-muted-foreground">{label}</Label>
      {children}
    </div>
  );
}

function ClearableInput({
  onClear,
  ...props
}: React.ComponentProps<"input"> & { onClear: () => void }) {
  const hasValue = props.value !== "" && props.value != null;
  return (
    <div className="relative">
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
        onBlur={field.handleBlur}
        onChange={(e) => field.handleChange(e.target.value)}
        onClear={() => field.handleChange("")}
        placeholder={placeholder}
      />
    </FieldShell>
  );
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
    <FieldShell label={label}>
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
