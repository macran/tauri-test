// src/components/query/DataTable.tsx
//
// 所有查询页面共用的结果表格：一次性取回全部匹配行（后端有 MAX_ROWS
// 兜底上限），用 @tanstack/react-virtual 做行虚拟化。
//
//  - table-layout: fixed + 显式列宽：列位置不随滚动到的行内容变化
//  - 列宽可拖拽调整；内容超出列宽以省略号截断（纯 CSS，悬停显示全文）
//  - 表头可拖拽调换列顺序；"列设置"可显隐/排序；"布局"可保存多套并记住上次使用
//  - 过滤在前端对已加载结果做：全局搜索 + 低基数列多选筛选（filterColumns）。
//    表格自身只用 TanStack 管列宽/显隐/顺序，筛选是纯函数，不依赖行模型。
//  - 传 onDeleteSelected 出现勾选列与批量删除；传 renderActions 出现操作列

import {
  type ColumnDef,
  columnOrderingFeature,
  columnResizingFeature,
  columnSizingFeature,
  columnVisibilityFeature,
  type RowData,
  tableFeatures,
  useTable,
} from "@tanstack/react-table";
import { useVirtualizer } from "@tanstack/react-virtual";
import { MagnifyingGlass, Trash, X } from "@phosphor-icons/react";
import { useEffect, useMemo, useRef, useState } from "react";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { type ComboboxOption, OptionCombobox } from "@/components/ui/option-combobox";
import {
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { ColumnMenu, LayoutMenu } from "./DataTableMenus";
import { reconcileLayout, type TableLayout, type TableLayouts } from "./table-layout";

export const dataTableFeatures = tableFeatures({
  columnSizingFeature,
  columnResizingFeature,
  columnVisibilityFeature,
  columnOrderingFeature,
});
const features = dataTableFeatures;

const ROW_HEIGHT = 36;
const VIEWPORT_HEIGHT = 560;
const SELECT_COL_WIDTH = 36;
const ACTIONS_COL_WIDTH = 96;
const AUTO_OPTIONS_LIMIT = 300;

type FilterValue = string | number;
type FilterSpec = ComboboxOption<FilterValue>[] | "auto";

export interface DataTableProps<T extends RowData> {
  /** 页面持有的布局状态（useTableLayouts）；选中项由表单里的 LayoutSelect 控制 */
  layouts: TableLayouts;
  columns: ColumnDef<typeof features, T>[];
  data: T[];
  loading: boolean;
  total: number;
  truncated?: boolean;
  rowKey: (row: T) => string | number;
  /** 提供列筛选的列：列 id → 固定选项（枚举列）或 "auto"（取结果集中出现过的值）。须为稳定引用 */
  filterColumns?: Record<string, FilterSpec>;
  /** 传入后出现勾选列和"删除所选"；确认后调用，成功 resolve 后清空勾选 */
  onDeleteSelected?: (rows: T[]) => Promise<void>;
  /** 传入后出现固定在最右的操作列；requestDelete 弹出统一的删除确认 */
  renderActions?: (row: T, ctx: { requestDelete: (rows: T[]) => void }) => React.ReactNode;
}

interface AccessorDef<T> {
  id?: string;
  accessorKey?: string;
  accessorFn?: (row: T) => unknown;
  header?: unknown;
}

function columnId<T>(c: AccessorDef<T>): string {
  return c.id ?? (c.accessorKey as string);
}

function columnGetter<T>(c: AccessorDef<T>): (row: T) => unknown {
  return c.accessorFn ?? ((row) => (row as Record<string, unknown>)[c.accessorKey as string]);
}

export function DataTable<T extends RowData>({
  layouts,
  columns,
  data,
  loading,
  total,
  truncated,
  rowKey,
  filterColumns,
  onDeleteSelected,
  renderActions,
}: DataTableProps<T>) {
  const defs = columns as unknown as AccessorDef<T>[];
  const columnIds = useMemo(() => defs.map(columnId), [defs]);
  const getters = useMemo(
    () => new Map(defs.map((c) => [columnId(c), columnGetter(c)] as const)),
    [defs],
  );
  const labels = useMemo(
    () =>
      new Map(
        defs.map((c) => [columnId(c), typeof c.header === "string" ? c.header : columnId(c)] as const),
      ),
    [defs],
  );

  // ───────── 布局（列顺序 / 显隐 / 列宽）─────────
  const defaultLayout = useMemo<TableLayout>(
    () => ({ columnOrder: columnIds, columnVisibility: {}, columnSizing: {} }),
    [columnIds],
  );
  const [columnOrder, setColumnOrder] = useState(columnIds);
  const [columnVisibility, setColumnVisibility] = useState<Record<string, boolean>>({});
  const [columnSizing, setColumnSizing] = useState<Record<string, number>>({});

  // 表单里选了别的布局（或保存/删除后选中项变化）时应用；null = 默认
  const selectedLayout = layouts.selectedItem?.layout;
  useEffect(() => {
    const l = reconcileLayout(selectedLayout ?? defaultLayout, columnIds);
    setColumnOrder(l.columnOrder);
    setColumnVisibility(l.columnVisibility);
    setColumnSizing(l.columnSizing);
    // 只在"选中项变化"时应用：重新加载列表（如设默认）不应冲掉用户尚未保存的调整
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [layouts.selected, defaultLayout, columnIds]);

  // ───────── 过滤 ─────────
  const [search, setSearch] = useState("");
  const [colFilters, setColFilters] = useState<Record<string, FilterValue[]>>({});

  const fixedOptions = useMemo(() => {
    const m = new Map<string, Map<string, string>>();
    for (const [id, spec] of Object.entries(filterColumns ?? {})) {
      if (spec !== "auto") m.set(id, new Map(spec.map((o) => [String(o.value), o.label])));
    }
    return m;
  }, [filterColumns]);

  const autoOptions = useMemo(() => {
    const out: Record<string, ComboboxOption<FilterValue>[]> = {};
    for (const [id, spec] of Object.entries(filterColumns ?? {})) {
      const get = getters.get(id);
      if (spec !== "auto" || !get) continue;
      const seen = new Set<string>();
      for (const r of data) {
        seen.add(String(get(r) ?? ""));
        if (seen.size > AUTO_OPTIONS_LIMIT) break;
      }
      out[id] = [...seen].sort().map((v) => ({ value: v, label: v || "（空）" }));
    }
    return out;
  }, [data, filterColumns, getters]);

  // 每行拼成一个小写字符串，搜索时只做一次 includes
  const haystack = useMemo(
    () =>
      data.map((r) =>
        [...getters.entries()]
          .map(([id, get]) => {
            const v = get(r);
            if (v == null) return "";
            return fixedOptions.get(id)?.get(String(v)) ?? String(v);
          })
          .join("\u0001")
          .toLowerCase(),
      ),
    [data, getters, fixedOptions],
  );

  const [debouncedSearch, setDebouncedSearch] = useState("");
  useEffect(() => {
    const t = setTimeout(() => setDebouncedSearch(search), 200);
    return () => clearTimeout(t);
  }, [search]);

  const filtered = useMemo(() => {
    const q = debouncedSearch.trim().toLowerCase();
    const active = Object.entries(colFilters)
      .filter(([, v]) => v.length > 0)
      .map(([id, v]) => [getters.get(id), new Set(v.map(String))] as const);
    if (!q && active.length === 0) return data;
    return data.filter(
      (r, i) =>
        (!q || haystack[i].includes(q)) &&
        active.every(([get, set]) => set.has(String(get?.(r) ?? ""))),
    );
  }, [data, haystack, debouncedSearch, colFilters, getters]);

  // ───────── 勾选 / 删除 ─────────
  const [selected, setSelected] = useState<Set<string | number>>(new Set());
  const [pendingDelete, setPendingDelete] = useState<T[] | null>(null);
  const [deleting, setDeleting] = useState(false);
  const [deleteError, setDeleteError] = useState<string | null>(null);
  // 新查询结果到来时，旧勾选作废
  useEffect(() => setSelected(new Set()), [data]);

  const selectedRows = useMemo(
    () => filtered.filter((r) => selected.has(rowKey(r))),
    [filtered, selected, rowKey],
  );
  const allSelected = filtered.length > 0 && selectedRows.length === filtered.length;

  // ───────── 表格 ─────────
  const table = useTable({
    key: `query-${layouts.scope}`,
    features,
    data: filtered,
    columns,
    defaultColumn: { size: 140, minSize: 60, maxSize: 800 },
    columnResizeMode: "onChange",
    state: { columnOrder, columnVisibility, columnSizing },
    onColumnOrderChange: setColumnOrder,
    onColumnVisibilityChange: setColumnVisibility,
    onColumnSizingChange: setColumnSizing,
  });

  const rows = table.getRowModel().rows;
  const scrollRef = useRef<HTMLDivElement>(null);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => ROW_HEIGHT,
    overscan: 12,
  });
  const virtualItems = virtualizer.getVirtualItems();
  const paddingTop = virtualItems.length > 0 ? virtualItems[0].start : 0;
  const paddingBottom =
    virtualItems.length > 0
      ? virtualizer.getTotalSize() - virtualItems[virtualItems.length - 1].end
      : 0;

  const visibleCount = table.getVisibleLeafColumns().length;
  const extraCols = (onDeleteSelected ? 1 : 0) + (renderActions ? 1 : 0);
  const spanAll = visibleCount + extraCols;
  const tableWidth =
    table.getTotalSize() +
    (onDeleteSelected ? SELECT_COL_WIDTH : 0) +
    (renderActions ? ACTIONS_COL_WIDTH : 0);

  const columnItems = columnOrder
    .filter((id) => labels.has(id))
    .map((id) => ({
      id,
      label: labels.get(id) as string,
      visible: columnVisibility[id] !== false,
    }));

  const moveColumn = (from: string, to: string) =>
    setColumnOrder((prev) => {
      const next = [...prev];
      next.splice(next.indexOf(from), 1);
      next.splice(prev.indexOf(to), 0, from);
      return next;
    });

  const dragId = useRef<string | null>(null);

  // 被截断的单元格悬停时才把全文放进 title，避免给每个单元格都挂 title
  const showFullText = (e: React.MouseEvent) => {
    const td = (e.target as HTMLElement).closest("td");
    if (td && !td.title && td.scrollWidth > td.clientWidth) td.title = td.textContent ?? "";
  };

  const confirmDelete = async () => {
    if (!pendingDelete || !onDeleteSelected) return;
    setDeleting(true);
    setDeleteError(null);
    try {
      await onDeleteSelected(pendingDelete);
      setSelected(new Set());
      setPendingDelete(null);
    } catch (e) {
      // 删除被拒绝（如发票已参与计算）时留在对话框里显示原因
      setDeleteError(e instanceof Error ? e.message : String(e));
    } finally {
      setDeleting(false);
    }
  };

  return (
    <div className="flex flex-col gap-2">
      <div className="flex flex-wrap items-center gap-2">
        <div className="relative w-64">
          <MagnifyingGlass className="absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="在结果中搜索…"
            className="pr-7 pl-7"
          />
          {search && (
            <button
              type="button"
              aria-label="清除"
              onClick={() => setSearch("")}
              className="absolute top-1/2 right-2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
            >
              <X className="size-3.5" />
            </button>
          )}
        </div>
        <div className="ml-auto flex items-center gap-2">
          {onDeleteSelected && (
            <Button
              type="button"
              variant="destructive"
              size="sm"
              disabled={selectedRows.length === 0}
              onClick={() => setPendingDelete(selectedRows)}
            >
              <Trash data-icon="inline-start" />
              删除所选{selectedRows.length > 0 && `（${selectedRows.length}）`}
            </Button>
          )}
          <ColumnMenu
            items={columnItems}
            onToggle={(id) => setColumnVisibility((v) => ({ ...v, [id]: v[id] === false }))}
            onMove={(id, delta) => {
              const i = columnOrder.indexOf(id);
              const target = columnOrder[i + delta];
              if (target) moveColumn(id, target);
            }}
          />
          <LayoutMenu
            mine={layouts.items.filter((i) => i.isMine)}
            selected={layouts.selectedItem}
            onSave={(name, isPrivate) =>
              void layouts.save(name, { columnOrder, columnVisibility, columnSizing }, isPrivate)
            }
            onSetDefault={(id) => void layouts.setDefault(id)}
            onDelete={(id) => void layouts.remove(id)}
          />
        </div>
      </div>

      <div
        ref={scrollRef}
        className="thin-scrollbar overflow-auto rounded-md border"
        style={{ maxHeight: VIEWPORT_HEIGHT }}
      >
        <table
          className="table-fixed caption-bottom border-collapse text-xs"
          style={{ width: tableWidth }}
        >
          <TableHeader>
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow key={headerGroup.id}>
                {onDeleteSelected && (
                  <TableHead
                    className="sticky top-0 left-0 z-20 border-r bg-muted px-2 shadow-[inset_0_-1px_0_var(--border)]"
                    style={{ width: SELECT_COL_WIDTH }}
                  >
                    <Checkbox
                      checked={allSelected}
                      indeterminate={selectedRows.length > 0 && !allSelected}
                      onCheckedChange={(c) =>
                        setSelected(c ? new Set(filtered.map(rowKey)) : new Set())
                      }
                    />
                  </TableHead>
                )}
                {headerGroup.headers.map((header) => {
                  const id = header.column.id;
                  const spec = filterColumns?.[id];
                  const options = spec === "auto" ? autoOptions[id] : spec;
                  return (
                    <TableHead
                      key={header.id}
                      className="sticky top-0 z-10 border-r bg-muted px-2 shadow-[inset_0_-1px_0_var(--border)]"
                      style={{ width: header.getSize() }}
                      onDragOver={(e) => e.preventDefault()}
                      onDrop={() => {
                        if (dragId.current && dragId.current !== id) moveColumn(dragId.current, id);
                        dragId.current = null;
                      }}
                    >
                      <div className="flex items-center gap-1 pr-2">
                        <span
                          draggable
                          onDragStart={() => {
                            dragId.current = id;
                          }}
                          className="min-w-0 flex-1 cursor-grab truncate"
                        >
                          {header.isPlaceholder ? null : <table.FlexRender header={header} />}
                        </span>
                        {options && (
                          <OptionCombobox
                            variant="icon"
                            options={options}
                            value={colFilters[id] ?? []}
                            onChange={(v) => setColFilters((f) => ({ ...f, [id]: v }))}
                          />
                        )}
                      </div>
                      {header.column.getCanResize() && (
                        <div
                          onMouseDown={header.getResizeHandler()}
                          onTouchStart={header.getResizeHandler()}
                          className="absolute top-0 right-0 h-full w-1.5 cursor-col-resize touch-none select-none hover:bg-primary/40"
                        />
                      )}
                    </TableHead>
                  );
                })}
                {renderActions && (
                  <TableHead
                    className="sticky top-0 right-0 z-20 border-l bg-muted px-2 shadow-[inset_0_-1px_0_var(--border)]"
                    style={{ width: ACTIONS_COL_WIDTH }}
                  >
                    操作
                  </TableHead>
                )}
              </TableRow>
            ))}
          </TableHeader>

          <TableBody onMouseOver={showFullText}>
            {loading ? (
              <TableRow>
                <TableCell colSpan={spanAll} className="h-24 text-center text-muted-foreground">
                  正在查询……
                </TableCell>
              </TableRow>
            ) : rows.length === 0 ? (
              <TableRow>
                <TableCell colSpan={spanAll} className="h-24 text-center text-muted-foreground">
                  没有符合条件的记录
                </TableCell>
              </TableRow>
            ) : (
              <>
                {paddingTop > 0 && (
                  <TableRow>
                    <TableCell colSpan={spanAll} style={{ height: paddingTop, padding: 0 }} />
                  </TableRow>
                )}
                {virtualItems.map((virtualRow) => {
                  const row = rows[virtualRow.index];
                  const key = rowKey(row.original);
                  return (
                    <TableRow
                      key={key}
                      data-index={virtualRow.index}
                      ref={virtualizer.measureElement}
                      className="h-9"
                      data-state={selected.has(key) ? "selected" : undefined}
                    >
                      {onDeleteSelected && (
                        <TableCell className="sticky left-0 z-10 border-r bg-background px-2">
                          <Checkbox
                            checked={selected.has(key)}
                            onCheckedChange={(c) =>
                              setSelected((s) => {
                                const next = new Set(s);
                                if (c) next.add(key);
                                else next.delete(key);
                                return next;
                              })
                            }
                          />
                        </TableCell>
                      )}
                      {row.getVisibleCells().map((cell) => (
                        <TableCell key={cell.id} className="truncate border-r px-2 py-0">
                          <table.FlexRender cell={cell} />
                        </TableCell>
                      ))}
                      {renderActions && (
                        <TableCell className="sticky right-0 z-10 border-l bg-background px-2 py-0">
                          {renderActions(row.original, {
                            requestDelete: setPendingDelete,
                          })}
                        </TableCell>
                      )}
                    </TableRow>
                  );
                })}
                {paddingBottom > 0 && (
                  <TableRow>
                    <TableCell colSpan={spanAll} style={{ height: paddingBottom, padding: 0 }} />
                  </TableRow>
                )}
              </>
            )}
          </TableBody>
        </table>
      </div>

      <p className="text-xs text-muted-foreground">
        {loading
          ? "正在查询……"
          : [
              truncated
                ? `共 ${total} 条，结果过多仅显示前 ${data.length} 条，请缩小查询范围（建议按月查询）`
                : `共 ${total} 条`,
              filtered.length !== data.length && `当前筛选后 ${filtered.length} 条`,
              truncated &&
                filtered.length !== data.length &&
                "（筛选只作用于已加载的行）",
            ]
              .filter(Boolean)
              .join("，")}
      </p>

      <AlertDialog
        open={pendingDelete !== null}
        onOpenChange={(open) => {
          if (open || deleting) return;
          setPendingDelete(null);
          setDeleteError(null);
        }}
      >
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>确认删除 {pendingDelete?.length ?? 0} 条记录？</AlertDialogTitle>
            <AlertDialogDescription className={deleteError ? "text-destructive" : undefined}>
              {deleteError ?? "删除后无法恢复。"}
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel disabled={deleting}>取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              disabled={deleting}
              onClick={(e) => {
                e.preventDefault();
                void confirmDelete();
              }}
            >
              {deleting ? "删除中…" : "删除"}
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
