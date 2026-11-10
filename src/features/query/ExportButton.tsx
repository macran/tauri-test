// 通用“导出 Excel”按钮：选保存路径 → 调后端命令（后端按当前条件重新查询并写文件）。
// 各页面只需提供 run(path)，即对应的 commands.exportXxx 调用。

import { useState } from "react";
import { CircleNotch, DownloadSimple } from "@phosphor-icons/react";
import { save } from "@tauri-apps/plugin-dialog";
import type { ExportColumn } from "@/bindings";
import { AppButton } from "@/components/app-button";
import { toast } from "@/components/ui/toast";

/** 表格列 → 后端导出列（id 即后端字段名；金额列传 money，枚举列传 labels） */
export type ExportCol = ExportColumn;
export const moneyCol = (key: string, title: string): ExportCol => ({ key, title, kind: "money", labels: null });
export const textCol = (key: string, title: string, labels?: Record<string, string>): ExportCol => ({
  key,
  title,
  kind: null,
  labels: labels ?? null,
});
export const enumCols = (opts: { value: number; label: string }[]) =>
  Object.fromEntries(opts.map((o) => [String(o.value), o.label]));

export function ExportButton({
  defaultName,
  label = "导出",
  disabled,
  run,
}: {
  /** 不含扩展名，如 "综合查询" */
  defaultName: string;
  /** 按钮文字，默认“导出” */
  label?: string;
  disabled?: boolean;
  run: (path: string) => Promise<{ status: "ok"; data: number } | { status: "error"; error: string }>;
}) {
  const [busy, setBusy] = useState(false);

  async function onClick() {
    const stamp = new Date().toISOString().slice(0, 10).replace(/-/g, "");
    const path = await save({
      defaultPath: `${defaultName}_${stamp}.xlsx`,
      filters: [{ name: "Excel", extensions: ["xlsx"] }],
    });
    if (!path) return;
    setBusy(true);
    try {
      const r = await run(path);
      if (r.status === "ok") toast.add({ type: "success", title: "导出完成", description: `${r.data} 行 → ${path}` });
      else toast.add({ type: "error", title: "导出失败", description: r.error });
    } catch (e) {
      toast.add({ type: "error", title: "导出失败", description: e instanceof Error ? e.message : String(e) });
    } finally {
      setBusy(false);
    }
  }

  return (
    <AppButton type="button" variant="glass" disabled={disabled || busy} onClick={() => void onClick()}>
      {busy ? <CircleNotch data-icon="inline-start" className="animate-spin" /> : <DownloadSimple data-icon="inline-start" />}
      {label}
    </AppButton>
  );
}
