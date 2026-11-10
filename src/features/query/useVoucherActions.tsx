// src/components/query/useVoucherActions.tsx
//
// bank / AP / IC 三个凭证查询页共用的"行内操作"：编辑（对话框）+ 删除（单行/批量）。
// 返回值直接喂给 DataTable 的 renderActions / onDeleteSelected，dialog 放在页面里渲染。

import { PencilSimple, Trash } from "@phosphor-icons/react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { toast } from "@/components/ui/toast";
import { type EditField, type EditValues, RowEditDialog } from "./RowEditDialog";
import type { DataTableProps } from "./DataTable";

type Result<T> = { status: "ok"; data: T } | { status: "error"; error: string };

export function useVoucherActions<T extends { id: number }, E>({
  title,
  fields,
  toInitial,
  toEdit,
  update,
  remove,
  refresh,
}: {
  title: string;
  fields: EditField[];
  toInitial: (row: T) => EditValues;
  toEdit: (row: T, values: EditValues) => E;
  update: (edit: E) => Promise<Result<null>>;
  remove: (ids: number[]) => Promise<Result<number>>;
  refresh: () => Promise<void>;
}) {
  const [editing, setEditing] = useState<T | null>(null);

  const renderActions: NonNullable<DataTableProps<T>["renderActions"]> = (row, { requestDelete }) => (
    <div className="flex items-center gap-1">
      <Button type="button" variant="ghost" size="icon-xs" aria-label="编辑" onClick={() => setEditing(row)}>
        <PencilSimple />
      </Button>
      <Button
        type="button"
        variant="ghost"
        size="icon-xs"
        aria-label="删除"
        className="text-destructive"
        onClick={() => requestDelete([row])}
      >
        <Trash />
      </Button>
    </div>
  );

  const onDeleteSelected = async (rows: T[]) => {
    const r = await remove(rows.map((x) => x.id));
    // 抛出让 DataTable 的确认框显示原因并保持打开
    if (r.status === "error") throw new Error(r.error);
    toast.add({ type: "success", title: `已删除 ${r.data} 条` });
    await refresh();
  };

  const dialog = editing && (
    <RowEditDialog
      title={title}
      fields={fields}
      initial={toInitial(editing)}
      onClose={() => setEditing(null)}
      onSave={async (values) => {
        const r = await update(toEdit(editing, values));
        if (r.status === "error") return r.error;
        toast.add({ type: "success", title: "已保存" });
        setEditing(null);
        await refresh();
        return null;
      }}
    />
  );

  return { renderActions, onDeleteSelected, dialog };
}
