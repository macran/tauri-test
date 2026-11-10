// src/components/query/RowEditDialog.tsx
//
// 凭证（bank / AP / IC）单行编辑对话框：由字段清单驱动，只渲染允许编辑的字段。
// onSave 返回错误信息（失败）或 null（成功，对话框随即关闭）。

import { useState } from "react";
import { parseLocalDate, toIsoDate } from "@/features/query/fields";
import { AppButton } from "@/components/app-button";
import {
  Dialog,
  DialogContent,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { Win11DatePicker } from "@/components/ui/win11-date-picker";
import type { EnumOption } from "@/lib/enum-options";

export type EditField =
  | { key: string; label: string; type: "text"; required?: boolean }
  | { key: string; label: string; type: "date"; required?: boolean }
  | { key: string; label: string; type: "enum"; options: EnumOption[] };

export type EditValues = Record<string, string | number | null>;

export function RowEditDialog({
  title,
  fields,
  initial,
  onSave,
  onClose,
}: {
  title: string;
  fields: EditField[];
  initial: EditValues;
  onSave: (values: EditValues) => Promise<string | null>;
  onClose: () => void;
}) {
  const [values, setValues] = useState<EditValues>(initial);
  const [saving, setSaving] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const set = (key: string, v: string | number | null) =>
    setValues((prev) => ({ ...prev, [key]: v }));

  const submit = async () => {
    const missing = fields.find(
      (f) => f.type !== "enum" && f.required && !String(values[f.key] ?? "").trim(),
    );
    if (missing) {
      setError(`${missing.label}不能为空`);
      return;
    }
    setSaving(true);
    setError(null);
    const err = await onSave(values);
    setSaving(false);
    if (err) setError(err);
  };

  return (
    <Dialog open onOpenChange={(open) => !open && !saving && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
        </DialogHeader>

        <form
          className="flex flex-col gap-3"
          onSubmit={(e) => {
            e.preventDefault();
            void submit();
          }}
        >
          {fields.map((f) => (
            <div key={f.key} className="flex flex-col gap-1">
              <Label className="text-muted-foreground">
                {f.label}
                {f.type !== "enum" && f.required && <span className="text-destructive"> *</span>}
              </Label>
              {f.type === "text" && (
                <Input
                  value={String(values[f.key] ?? "")}
                  onChange={(e) => set(f.key, e.target.value)}
                />
              )}
              {f.type === "date" && (
                <Win11DatePicker
                  value={parseLocalDate(String(values[f.key] ?? ""))}
                  onChange={(d) => set(f.key, toIsoDate(d))}
                />
              )}
              {f.type === "enum" && (
                <Select
                  items={f.options.map((o) => ({ value: String(o.value), label: o.label }))}
                  value={String(values[f.key])}
                  onValueChange={(v) => set(f.key, Number(v))}
                >
                  <SelectTrigger className="w-full">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {f.options.map((o) => (
                      <SelectItem key={o.value} value={String(o.value)}>
                        {o.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              )}
            </div>
          ))}

          {error && <p className="text-xs text-destructive">{error}</p>}

          <DialogFooter>
            <AppButton type="button" variant="glass" disabled={saving} onClick={onClose}>
              取消
            </AppButton>
            <AppButton type="submit" variant="brand" disabled={saving}>
              {saving ? "保存中…" : "保存"}
            </AppButton>
          </DialogFooter>
        </form>
      </DialogContent>
    </Dialog>
  );
}
