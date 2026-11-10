import { open as openFile } from "@tauri-apps/plugin-dialog";
import { useEffect, useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Dialog, DialogContent, DialogDescription, DialogFooter, DialogHeader, DialogTitle } from "@/components/ui/dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { stApi } from "../api";
import type { StampTemplateSettings, TemplateColumns } from "../types";

const COLUMN_FIELDS: { key: keyof TemplateColumns; label: string }[] = [
  { key: "seq", label: "序号" },
  { key: "itemName", label: "税目名称" },
  { key: "itemCode", label: "税目代码" },
  { key: "subItemName", label: "子目名称" },
  { key: "subItemCode", label: "子目代码" },
  { key: "periodFrom", label: "税款所属期起" },
  { key: "periodTo", label: "税款所属期止" },
  { key: "baseAmount", label: "计税金额" },
  { key: "rate", label: "税率" },
  { key: "taxAmount", label: "应纳税额" },
  { key: "lineCount", label: "明细行数" },
];

const PLACEHOLDERS = "{filingFrom} {filingTo} {periodStart} {periodEnd} {totalBase} {totalTax} {today} {user}";

export function StampTemplateSettingsDialog({
  open,
  onOpenChange,
  onSaved,
}: {
  open: boolean;
  onOpenChange: (o: boolean) => void;
  onSaved?: (s: StampTemplateSettings) => void;
}) {
  const [s, setS] = useState<StampTemplateSettings | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    if (open) stApi.getTemplateSettings().then(setS).catch((e) => toast.error((e as Error).message));
  }, [open]);

  const setCol = (k: keyof TemplateColumns, v: string) =>
    setS((x) => (x ? { ...x, columns: { ...x.columns, [k]: v.trim().toUpperCase() || null } } : x));

  return (
    <Dialog open={open} onOpenChange={onOpenChange}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>印花税信息采集表 · 模版设置</DialogTitle>
          <DialogDescription>
            选择税务局提供的 Excel 模版（.xlsx），并指定数据起始行和各字段写入的列。导出时保留模版原有格式和公式。
          </DialogDescription>
        </DialogHeader>
        {!s ? (
          <p className="text-sm text-muted-foreground">加载中…</p>
        ) : (
          <div className="grid max-h-[65vh] gap-4 overflow-auto pr-1">
            <div className="grid gap-1.5">
              <Label>模版文件</Label>
              <div className="flex gap-2">
                <Input readOnly value={s.templatePath ?? ""} placeholder="未选择" />
                <Button
                  variant="outline"
                  onClick={async () => {
                    const p = await openFile({ multiple: false, filters: [{ name: "Excel 模版", extensions: ["xlsx", "xlsm"] }] });
                    if (typeof p === "string") setS({ ...s, templatePath: p });
                  }}
                >
                  选择…
                </Button>
              </div>
              <p className="text-xs text-muted-foreground">数据库在共享盘时，建议把模版也放到共享盘，所有人路径一致。</p>
            </div>

            <div className="grid gap-3 sm:grid-cols-3">
              <div className="grid gap-1.5">
                <Label htmlFor="tpl-sheet">工作表名称</Label>
                <Input id="tpl-sheet" placeholder="留空 = 第一个" value={s.sheetName ?? ""} onChange={(e) => setS({ ...s, sheetName: e.target.value || null })} />
              </div>
              <div className="grid gap-1.5">
                <Label htmlFor="tpl-row">数据起始行</Label>
                <Input
                  id="tpl-row"
                  type="number"
                  min={1}
                  value={s.startRow}
                  onChange={(e) => setS({ ...s, startRow: Math.max(1, Number(e.target.value) || 1) })}
                />
              </div>
              <fieldset className="grid gap-1.5">
                <legend className="mb-1.5 text-sm font-medium">汇总口径</legend>
                <div className="flex gap-3 text-sm">
                  {(
                    [
                      ["child", "按子目"],
                      ["parent", "按父级税目"],
                    ] as const
                  ).map(([v, t]) => (
                    <label key={v} className="flex items-center gap-1">
                      <input type="radio" name="tpl-level" checked={s.groupLevel === v} onChange={() => setS({ ...s, groupLevel: v })} />
                      {t}
                    </label>
                  ))}
                </div>
              </fieldset>
            </div>

            <div>
              <Label>字段 → 列（填列字母，如 B；留空不写）</Label>
              <div className="mt-2 grid grid-cols-2 gap-2 sm:grid-cols-4">
                {COLUMN_FIELDS.map((f) => (
                  <label key={f.key} className="flex items-center justify-between gap-2 rounded border px-2 py-1 text-sm">
                    <span className="text-muted-foreground">{f.label}</span>
                    <input
                      aria-label={`${f.label}所在列`}
                      className="h-7 w-14 rounded border bg-background px-1 text-center uppercase"
                      maxLength={3}
                      value={s.columns[f.key] ?? ""}
                      onChange={(e) => setCol(f.key, e.target.value)}
                    />
                  </label>
                ))}
              </div>
              <p className="mt-1 text-xs text-muted-foreground">计税金额、应纳税额以“元”写入；税率写小数（万分之三 = 0.0003）；所属期写 YYYY-MM-DD。</p>
            </div>

            <div>
              <div className="flex items-center justify-between">
                <Label>固定单元格（表头的纳税人、所属期等）</Label>
                <Button size="sm" variant="outline" onClick={() => setS({ ...s, fixedCells: [...s.fixedCells, { cell: "", value: "" }] })}>
                  添加
                </Button>
              </div>
              <ul className="mt-2 grid gap-2">
                {s.fixedCells.map((fc, i) => (
                  <li key={i} className="flex gap-2">
                    <Input
                      aria-label="单元格地址"
                      className="w-24 uppercase"
                      placeholder="B3"
                      value={fc.cell}
                      onChange={(e) => {
                        const next = [...s.fixedCells];
                        next[i] = { ...fc, cell: e.target.value.toUpperCase() };
                        setS({ ...s, fixedCells: next });
                      }}
                    />
                    <Input
                      aria-label="内容"
                      placeholder="如：{periodStart} 至 {periodEnd}"
                      value={fc.value}
                      onChange={(e) => {
                        const next = [...s.fixedCells];
                        next[i] = { ...fc, value: e.target.value };
                        setS({ ...s, fixedCells: next });
                      }}
                    />
                    <Button variant="ghost" size="sm" onClick={() => setS({ ...s, fixedCells: s.fixedCells.filter((_, j) => j !== i) })}>
                      删除
                    </Button>
                  </li>
                ))}
              </ul>
              <p className="mt-1 text-xs text-muted-foreground">可用占位符：{PLACEHOLDERS}</p>
            </div>
          </div>
        )}
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            取消
          </Button>
          <Button
            disabled={!s || saving}
            onClick={async () => {
              if (!s) return;
              setSaving(true);
              try {
                const clean = { ...s, fixedCells: s.fixedCells.filter((f) => f.cell.trim()) };
                await stApi.saveTemplateSettings(clean);
                toast.success("模版设置已保存");
                onSaved?.(clean);
                onOpenChange(false);
              } catch (e) {
                toast.error((e as Error).message);
              } finally {
                setSaving(false);
              }
            }}
          >
            {saving ? "保存中…" : "保存"}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
