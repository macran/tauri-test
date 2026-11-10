// src/components/query/table-layout.ts
//
// 结果表格的"布局"：列顺序、列显隐、列宽。每张表（scope）可保存多个命名布局，
// 存在 SQLite 的 user_layouts 表；"默认"布局永远存在，由列定义生成，不落盘。
// 选中哪个布局由页面持有（表单里的布局下拉），DataTable 负责应用和保存。

import { useCallback, useEffect, useRef, useState } from "react";
import { commands } from "@/bindings";
import { toast } from "@/components/ui/toast";

export interface TableLayout {
  columnOrder: string[];
  columnVisibility: Record<string, boolean>;
  columnSizing: Record<string, number>;
}

export const SYSTEM_DEFAULT_LABEL = "系统默认";

export interface LayoutItem {
  id: number;
  name: string;
  owner: string;
  layout: TableLayout;
  isDefault: boolean;
  isPrivate: boolean;
  isMine: boolean;
}

export function useTableLayouts(scope: string) {
  const [items, setItems] = useState<LayoutItem[]>([]);
  /** 当前选中的布局 id；null = 系统默认 */
  const [selected, setSelectedId] = useState<number | null>(null);
  // 用户手动选过之后，就不再让"我的默认布局"覆盖他的选择
  const touched = useRef(false);

  const reload = useCallback(async () => {
    const r = await commands.listLayouts(scope);
    if (r.status !== "ok") return [] as LayoutItem[];
    const next: LayoutItem[] = [];
    for (const rec of r.data) {
      try {
        next.push({ ...rec, layout: JSON.parse(rec.layout) as TableLayout });
      } catch {
        // 损坏的布局直接忽略
      }
    }
    setItems(next);
    return next;
  }, [scope]);

  // 首次加载：我设了默认布局就自动选中它
  useEffect(() => {
    void reload().then((next) => {
      const mine = next.find((i) => i.isMine && i.isDefault);
      if (mine && !touched.current) setSelectedId(mine.id);
    });
  }, [reload]);

  const fail = (what: string, error: string) =>
    toast.add({ type: "error", title: `${what}失败：${error}` });

  const setSelected = useCallback((id: number | null) => {
    touched.current = true;
    setSelectedId(id);
  }, []);

  const save = useCallback(
    async (name: string, layout: TableLayout, isPrivate: boolean) => {
      const r = await commands.saveLayout(scope, name, JSON.stringify(layout), isPrivate);
      if (r.status === "error") return fail("保存布局", r.error);
      await reload();
      setSelected(r.data);
    },
    [scope, reload, setSelected],
  );

  const remove = useCallback(
    async (id: number) => {
      const r = await commands.deleteLayout(id);
      if (r.status === "error") return fail("删除布局", r.error);
      await reload();
      setSelectedId((cur) => (cur === id ? null : cur));
    },
    [reload],
  );

  /** id 为 null：取消我的默认布局（回到系统默认） */
  const setDefault = useCallback(
    async (id: number | null) => {
      const r = await commands.setDefaultLayout(scope, id);
      if (r.status === "error") return fail("设置默认布局", r.error);
      await reload();
    },
    [scope, reload],
  );

  return {
    scope,
    items,
    selected,
    selectedItem: items.find((i) => i.id === selected),
    setSelected,
    save,
    remove,
    setDefault,
  };
}

export type TableLayouts = ReturnType<typeof useTableLayouts>;

/** 保存的布局可能早于后续新增/删除的列：丢弃已不存在的列，新列补在末尾并默认显示。 */
export function reconcileLayout(
  layout: TableLayout,
  columnIds: string[],
): TableLayout {
  const known = new Set(columnIds);
  const order = layout.columnOrder.filter((id) => known.has(id));
  for (const id of columnIds) if (!order.includes(id)) order.push(id);
  return {
    columnOrder: order,
    columnVisibility: Object.fromEntries(
      Object.entries(layout.columnVisibility).filter(([id]) => known.has(id)),
    ),
    columnSizing: Object.fromEntries(
      Object.entries(layout.columnSizing).filter(([id]) => known.has(id)),
    ),
  };
}
