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
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
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
import { cn } from "@/lib/utils";
import { usePointerReorder } from "./usePointerReorder";
import { ColumnConfigurator } from "./ColumnConfigurator";
import { LayoutMenu } from "./DataTableMenus";
import { MAX_FROZEN, reconcileLayout, type TableLayout, type TableLayouts } from "./table-layout";

export const dataTableFeatures = tableFeatures({
  columnSizingFeature,
  columnResizingFeature,
  columnVisibilityFeature,
  columnOrderingFeature,
});
const features = dataTableFeatures;

const ROW_HEIGHT = 28;
const VIEWPORT_HEIGHT = 560;
const SELECT_COL_WIDTH = 36;
const ACTIONS_COL_WIDTH = 96;
const AUTO_OPTIONS_LIMIT = 300;

type FilterValue = string | number;
type FilterSpec = ComboboxOption<FilterValue>[] | "auto";

export interface DataTableProps<T extends RowData> {
  /** 页面持有的布局状态（useTableLayouts）；选中项由表单里的 LayoutSelect 控制 */
  /** 不传 = 不提供布局功能（列顺序/显隐/列宽只在本次会话内有效） */
  layouts?: TableLayouts;
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
  /** 传入后出现勾选列；至少勾选一行时，批量操作才出现在搜索框右侧（与搜索框同一行，出现/消失不会让表格移位） */
  renderBulkActions?: (selected: T[], clear: () => void) => React.ReactNode;
  /** 渲染在全局搜索框后面的额外筛选控件（状态由调用方持有） */
  filterSlot?: React.ReactNode;
  /** 工具栏右侧、列设置之前的表格级操作（如导出）：它们作用于这张表的数据，不属于查询表单 */
  toolbarSlot?: React.ReactNode;
  /** 数字列的 id：表头与内容一起右对齐，并用等宽数字让小数点对齐 */
  numericColumns?: readonly string[];
  /** 与底部「共 N 条」同一行、靠右的内容（如合计） */
  footerSlot?: React.ReactNode;
  /** 值变化时清空搜索、列过滤和勾选（如用户重新点了「匹配」） */
  resetKey?: number;
  /** 按行追加样式（如标出人工修改的行） */
  rowClassName?: (row: T) => string | undefined;
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

/** 表头文字按实际字体量出的宽度（px）；canvas 只建一次。 */
let headerCtx: CanvasRenderingContext2D | null | undefined;
function textWidth(text: string): number {
  if (headerCtx === undefined) headerCtx = document.createElement("canvas").getContext("2d");
  if (!headerCtx) return text.length * 14;
  // 与 TableHead 一致：font-medium、继承字号和字体
  const cs = getComputedStyle(document.body);
  headerCtx.font = `500 ${cs.fontSize} ${cs.fontFamily}`;
  return Math.ceil(headerCtx.measureText(text).width);
}

/** 表头里文字之外占的宽度：px-2 ×2 + pr-2 + 边框 1 + 右侧拖拽缓冲 = 约 26；带过滤图标的列再加图标和 gap */
const HEADER_CHROME = 26;
const HEADER_FILTER_ICON = 24;

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
  renderBulkActions,
  filterSlot,
  toolbarSlot,
  rowClassName,
  resetKey = 0,
  footerSlot,
  numericColumns,
}: DataTableProps<T>) {
  const numeric = useMemo(() => new Set(numericColumns ?? []), [numericColumns]);
  const selectable = !!(onDeleteSelected || renderBulkActions);
  // 只有还没有数据时才显示"正在查询"占位；已有数据时保留旧行，避免表格塌缩、滚动位置丢失
  const initialLoading = loading && data.length === 0;
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

  // 默认列宽至少能放下列标题：页面给的 size 若比标题窄就抬高到标题宽度。
  // 只改默认值：用户保存的布局 / 手动拖拽的宽度（columnSizing）优先，不受影响；
  // minSize 保持 60，用户仍可手动拖窄。
  const sizedColumns = useMemo(
    () =>
      (columns as unknown as (AccessorDef<T> & { size?: number })[]).map((c) => {
        if (typeof c.header !== "string") return c;
        const need =
          textWidth(c.header) + HEADER_CHROME + (filterColumns?.[columnId(c)] ? HEADER_FILTER_ICON : 0);
        const size = Math.min(800, Math.max(c.size ?? 140, need));
        return size === (c.size ?? 140) ? c : { ...c, size };
      }) as unknown as typeof columns,
    [columns, filterColumns],
  );

  // ───────── 布局（列顺序 / 显隐 / 列宽）─────────
  const defaultLayout = useMemo<TableLayout>(
    () => ({ columnOrder: columnIds, columnVisibility: {}, columnSizing: {}, frozen: 0 }),
    [columnIds],
  );
  const [columnOrder, setColumnOrder] = useState(columnIds);
  const [columnVisibility, setColumnVisibility] = useState<Record<string, boolean>>({});
  const [columnSizing, setColumnSizing] = useState<Record<string, number>>({});
  /** 固定列数：columnOrder 的前 N 列固定在左侧 */
  const [frozen, setFrozen] = useState(0);

  // 表单里选了别的布局（或保存/删除后选中项变化）时应用；null = 默认
  const selectedLayout = layouts?.selectedItem?.layout;
  // 应用布局带来的列位置变化不是用户操作，不该播放让位动画（见下面的 FLIP）
  const skipFlip = useRef(false);
  // useLayoutEffect：在浏览器绘制之前应用，布局就绪后的第一帧就是用户的列顺序，不会先画一帧旧的
  useLayoutEffect(() => {
    const l = reconcileLayout(selectedLayout ?? defaultLayout, columnIds);
    // 顺序内容没变（例如只隐藏了几列）就保持旧引用：columnOrder 不变，FLIP effect 不会运行。
    // 只有顺序真的变了才标记跳过——否则标记没人消费，会吞掉用户之后第一次手动拖动的动画。
    const sameOrder =
      columnOrder.length === l.columnOrder.length &&
      columnOrder.every((id, i) => id === l.columnOrder[i]);
    if (!sameOrder) {
      skipFlip.current = true;
      setColumnOrder(l.columnOrder);
    }
    setColumnVisibility(l.columnVisibility);
    setColumnSizing(l.columnSizing);
    setFrozen(l.frozen ?? 0);
    // 只在"选中项变化"时应用：重新加载列表（如设默认）不应冲掉用户尚未保存的调整
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [layouts?.selected, defaultLayout, columnIds]);

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
  const [lastResetKey, setLastResetKey] = useState(resetKey);
  if (resetKey !== lastResetKey) {
    // 渲染期间同步更新，避免先用旧过滤画一帧
    setLastResetKey(resetKey);
    setSearch("");
    setColFilters({});
    setSelected(new Set());
  }
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
    key: `query-${layouts?.scope ?? "static"}`,
    features,
    data: filtered,
    columns: sizedColumns,
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
  const extraCols = (selectable ? 1 : 0) + (renderActions ? 1 : 0);
  const spanAll = visibleCount + extraCols;
  const tableWidth =
    table.getTotalSize() +
    (selectable ? SELECT_COL_WIDTH : 0) +
    (renderActions ? ACTIONS_COL_WIDTH : 0);

  // 固定列的 sticky left：选择列宽度 + 前面已显示的固定列宽度之和
  const frozenLeft = new Map<string, number>();
  let leftAcc = selectable ? SELECT_COL_WIDTH : 0;
  const frozenIds = new Set(columnOrder.slice(0, frozen));
  for (const c of table.getVisibleLeafColumns()) {
    if (!frozenIds.has(c.id)) continue;
    frozenLeft.set(c.id, leftAcc);
    leftAcc += c.getSize();
  }
  const lastFrozenId = [...frozenLeft.keys()].pop();

  const columnItems = columnOrder
    .filter((id) => labels.has(id))
    .map((id) => ({
      id,
      label: labels.get(id) as string,
      visible: columnVisibility[id] !== false,
      pinned: columnOrder.indexOf(id) < frozen,
    }));

  const moveColumn = (from: string, to: string) => {
    const fromIdx = columnOrder.indexOf(from);
    const toIdx = columnOrder.indexOf(to);
    const next = [...columnOrder];
    next.splice(fromIdx, 1);
    next.splice(toIdx, 0, from);
    setColumnOrder(next);
    // 拖过固定区边界时，固定列数跟着变
    if (fromIdx >= frozen && toIdx < frozen) setFrozen(Math.min(frozen + 1, MAX_FROZEN));
    else if (fromIdx < frozen && toIdx >= frozen) setFrozen(frozen - 1);
  };

  /** 固定：挪到固定区末尾；取消固定：挪到固定区之后第一个位置 */
  const togglePin = (id: string) => {
    const pinning = columnOrder.indexOf(id) >= frozen;
    if (pinning && frozen >= MAX_FROZEN) return;
    const next = columnOrder.filter((c) => c !== id);
    next.splice(pinning ? frozen : frozen - 1, 0, id);
    setColumnOrder(next);
    setFrozen(pinning ? frozen + 1 : frozen - 1);
  };

  const { drag, start: startDrag } = usePointerReorder("data-col-id", moveColumn);

  // 列顺序变化时的让位动画（FLIP）：对比换位前后各列表头的 x，给该列所有单元格先反向平移再滑回。
  const colLefts = useRef(new Map<string, number>());
  useLayoutEffect(() => {
    const root = scrollRef.current;
    if (!root) return;
    if (skipFlip.current) {
      skipFlip.current = false;
      return;
    }
    colLefts.current.forEach((prev, id) => {
      const head = root.querySelector<HTMLElement>(`thead [data-col-id="${id}"]`);
      const dx = head ? prev - head.getBoundingClientRect().left : 0;
      if (Math.abs(dx) < 1) return;
      const cells = root.querySelectorAll<HTMLElement>(`[data-col-id="${id}"]`);
      cells.forEach((el) => {
        el.style.transition = "none";
        el.style.transform = `translateX(${dx}px)`;
      });
      void root.offsetWidth; // 强制回流，使下面的过渡生效
      cells.forEach((el) => {
        el.style.transition = "transform 150ms ease";
        el.style.transform = "";
      });
    });
  }, [columnOrder]);
  // 每次提交后记录各列位置，供下次换位对比（须声明在上面的 effect 之后）
  useLayoutEffect(() => {
    const m = new Map<string, number>();
    scrollRef.current
      ?.querySelectorAll<HTMLElement>("thead [data-col-id]")
      .forEach((el) => m.set(el.dataset.colId!, el.getBoundingClientRect().left));
    colLefts.current = m;
  });

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

  const bulkBar =
    renderBulkActions && selectedRows.length > 0
      ? renderBulkActions(selectedRows, () => setSelected(new Set()))
      : null;

  return (
    <div className="flex min-h-0 min-w-0 flex-col gap-2">
      <div className="flex shrink-0 flex-wrap items-center gap-2">
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
        {filterSlot}
        {bulkBar && <div className="flex items-center gap-2 border-l pl-2 text-sm">{bulkBar}</div>}
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
          {toolbarSlot}
          <ColumnConfigurator
            items={columnItems}
            onToggle={(id) => setColumnVisibility((v) => ({ ...v, [id]: v[id] === false }))}
            onMove={moveColumn}
            onTogglePin={togglePin}
            pinFull={frozen >= MAX_FROZEN}
          />
          {layouts && (
            <LayoutMenu
              mine={layouts.items.filter((i) => i.isMine)}
              selected={layouts.selectedItem}
              onSave={(name, isPrivate) =>
                void layouts.save(name, { columnOrder, columnVisibility, columnSizing, frozen }, isPrivate)
              }
              onSetDefault={(id) => void layouts.setDefault(id)}
              onDelete={(id) => void layouts.remove(id)}
            />
          )}
        </div>
      </div>

      <div
        ref={scrollRef}
        className={cn(
          "thin-scrollbar table-scrollbar min-h-0 overflow-auto rounded-md border transition-opacity",
          loading && !initialLoading && "pointer-events-none opacity-60",
        )}
        style={{ maxHeight: VIEWPORT_HEIGHT }}
      >
        {layouts && !layouts.ready ? (
          <div className="flex h-24 items-center justify-center text-xs text-muted-foreground">
            正在加载布局……
          </div>
        ) : (
        <table
          // border-separate：border-collapse 下 sticky 单元格的边框不随格移动，固定列之间会漏出缝隙
          className="table-fixed caption-bottom border-separate border-spacing-0 text-xs"
          style={{ width: tableWidth }}
        >
          <TableHeader className="[&_th]:font-semibold">
            {table.getHeaderGroups().map((headerGroup) => (
              <TableRow key={headerGroup.id}>
                {selectable && (
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
                      className={cn(
                        "sticky top-0 z-10 border-r bg-muted px-2 shadow-[inset_0_-1px_0_var(--border)]",
                        frozenLeft.has(id) && "z-20",
                        id === lastFrozenId && "border-r-2 border-r-primary/50",
                        drag?.id === id && "bg-primary/20",
                      )}
                      style={{ width: header.getSize(), left: frozenLeft.get(id) }}
                      data-col-id={id}
                    >
                      <div className="flex items-center gap-1 pr-2">
                        <span
                          onPointerDown={startDrag(id)}
                          className={cn(
                            "min-w-0 flex-1 cursor-grab touch-none truncate select-none",
                            numeric.has(id) && "text-right",
                          )}
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
            {initialLoading ? (
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
                      className={cn("h-7", rowClassName?.(row.original))}
                      data-state={selected.has(key) ? "selected" : undefined}
                    >
                      {selectable && (
                        <TableCell
                          className="sticky left-0 z-10 border-r border-b bg-background px-2"
                          style={{ width: SELECT_COL_WIDTH }}
                        >
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
                        <TableCell
                          key={cell.id}
                          data-col-id={cell.column.id}
                          className={cn(
                            "truncate border-r border-b px-2 py-0",
                            frozenLeft.has(cell.column.id) && "sticky z-10 bg-background",
                            cell.column.id === lastFrozenId && "border-r-2 border-r-primary/50",
                            numeric.has(cell.column.id) && "text-right tabular-nums",
                          )}
                          style={{ left: frozenLeft.get(cell.column.id) }}
                        >
                          <table.FlexRender cell={cell} />
                        </TableCell>
                      ))}
                      {renderActions && (
                        <TableCell className="sticky right-0 z-10 border-l border-b bg-background px-2 py-0">
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
        )}
      </div>

      {drag && (
        <div
          className="pointer-events-none fixed z-50 rounded-md border bg-background px-2 py-1 text-xs font-medium shadow-lg"
          style={{ left: drag.x + 12, top: drag.y + 12 }}
        >
          {labels.get(drag.id)}
        </div>
      )}

      <div className="flex flex-wrap items-center gap-x-5 gap-y-1">
      <p className="text-xs text-muted-foreground">
        {initialLoading
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
      {footerSlot && <div className="ml-auto flex flex-wrap items-center gap-x-5 gap-y-1 text-xs">{footerSlot}</div>}
      </div>

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
