// src/components/query/overview/ColumnPanel.tsx
//
// 综合查询的"选择列"侧栏。每一列只出现一次：
//   搜索框（带清除）→ 统计行 → 一个个可折叠的组。
//   组头：折叠箭头 · 拖动整组 · 组名 · 已选/总数 · 整组复选框（部分选中显示横线）。
//   组内列：拖动排序（仅已选列）· 复选框 · 列名 · 图钉（固定到左侧，同时勾选）。
// 固定列单独成区放在最顶部（它们来自不同的组，是表格左侧的一个整体）；下面的组只列普通列。
// 组的顺序跟表格里的色带顺序走：有已选列的组按它在滚动区中的位置排，没有的排最后。
// 同一时间只展开一个组（手风琴）；搜索时所有命中的组一起展开。
// 发票号码永远在最前并固定，不能取消。
// 拖拽用 pointer 事件（usePointerReorder），不用 HTML5 draggable：Tauri 开启文件拖放后 WebView 不触发网页内的 drag/drop。

import { CaretDown, CaretRight, DotsSix, PushPin } from "@phosphor-icons/react";
import { useMemo, useState } from "react";
import { AppButton } from "@/components/app-button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Drawer,
  DrawerContent,
  DrawerFooter,
  DrawerHeader,
  DrawerTitle,
} from "@/components/ui/drawer";
import { cn } from "@/lib/utils";
import { ClearableInput } from "../fields";
import { MAX_FROZEN } from "../table-layout";
import { usePointerReorder } from "../usePointerReorder";
import { COLS, GROUPS, hueStyle, LEGACY_COL_IDS } from "./columns";

export const FIRST = "header_inv_unq_key";
const BY_ID = new Map(COLS.map((c) => [c.id, c]));
const GROUP_KEYS = Object.keys(GROUPS);

/** 整理列 id：发票号码始终在最前，去重，丢弃已不存在的列。**不按注册表重排**——顺序由用户决定。 */
export const normalizeCols = (ids: string[]) =>
  [FIRST, ...new Set(ids.map((id) => LEGACY_COL_IDS[id] ?? id).filter((id) => id !== FIRST))].filter((id) =>
    BY_ID.has(id),
  );

/** 面板里组的排列：有已选（非固定）列的组按"第一个已选列在滚动区中的位置"，没有的排最后（注册表顺序）。
 *  固定列不参与：它们在顶部单独成区，不能因为某组里有一列被固定就把整组顶到前面。 */
function orderGroups(selected: string[], nFrozen: number) {
  const pos = new Map<string, number>();
  selected.slice(nFrozen).forEach((id, i) => {
    const g = BY_ID.get(id)?.group;
    if (g && !pos.has(g)) pos.set(g, i);
  });
  const withSel = GROUP_KEYS.filter((g) => pos.has(g)).sort((a, b) => pos.get(a)! - pos.get(b)!);
  return [...withSel, ...GROUP_KEYS.filter((g) => !pos.has(g))];
}

/** 组内列的显示顺序：已选的（不含固定列）按表格中的相对顺序，未选的跟在后面按注册表顺序。
 *  固定列在顶部固定区里显示，这里不再出现，保证每列只出现一次。 */
function colsOf(group: string, selected: string[], nFrozen: number) {
  const sel = selected.slice(nFrozen).filter((id) => BY_ID.get(id)?.group === group);
  const pinned = new Set(selected.slice(0, nFrozen));
  const rest = COLS.filter((c) => c.group === group && !sel.includes(c.id) && !pinned.has(c.id)).map((c) => c.id);
  return [...sel, ...rest];
}

/** 勾选：落在该组已选列末尾；该组还没有已选列则追加到整个表格末尾 */
function addCol(selected: string[], id: string) {
  if (selected.includes(id)) return selected;
  const g = BY_ID.get(id)!.group;
  let last = -1;
  selected.forEach((x, i) => {
    if (BY_ID.get(x)?.group === g) last = i;
  });
  const next = selected.slice();
  next.splice(last < 0 ? next.length : last + 1, 0, id);
  return next;
}

/** 滚动区里连续同组的列合成块（固定区之后） */
function toBlocks(ids: string[], startAt: number) {
  const blocks: { group: string; ids: string[] }[] = [];
  ids.slice(startAt).forEach((id) => {
    const g = BY_ID.get(id)!.group;
    const last = blocks[blocks.length - 1];
    if (last && last.group === g) last.ids.push(id);
    else blocks.push({ group: g, ids: [id] });
  });
  return blocks;
}

export function ColumnPanel({
  open,
  onOpenChange,
  selected,
  onChange,
  frozenCount,
  onFrozenChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  selected: string[];
  onChange: (ids: string[]) => void;
  /** 前 N 列固定（至少 1：发票号码） */
  frozenCount: number;
  onFrozenChange: (n: number) => void;
}) {
  const [search, setSearch] = useState("");
  // 手风琴：同一时间只展开一个组（null = 全部收起）。搜索时不受它限制。
  const [openGroup, setOpenGroup] = useState<string | null>(null);
  const q = search.trim().toLowerCase();
  const sel = useMemo(() => new Set(selected), [selected]);
  const nFrozen = Math.max(1, Math.min(frozenCount, selected.length, MAX_FROZEN));
  const ordered = useMemo(() => orderGroups(selected, nFrozen), [selected, nFrozen]);

  const setCols = (ids: string[], on: boolean) => {
    if (on) onChange(ids.reduce(addCol, selected));
    else {
      const drop = new Set(ids.filter((id) => id !== FIRST));
      const next = selected.filter((x) => !drop.has(x));
      // 被取消的列若在固定区里，固定数同步减少
      const removedFrozen = selected.slice(0, nFrozen).filter((x) => drop.has(x)).length;
      onChange(next);
      if (removedFrozen) onFrozenChange(Math.max(1, nFrozen - removedFrozen));
    }
  };

  // 图钉：已固定 → 取消，紧挨着留在固定区后面；未固定 → 挪到固定区末尾 +1（未选中的会同时勾选）
  const pin = (id: string) => {
    if (id === FIRST) return;
    const base = selected.includes(id) ? selected : addCol(selected, id);
    const i = base.indexOf(id);
    const next = base.slice();
    if (i < nFrozen) {
      next.splice(i, 1);
      next.splice(nFrozen - 1, 0, id);
      onChange(next);
      onFrozenChange(nFrozen - 1);
    } else if (nFrozen < MAX_FROZEN) {
      next.splice(i, 1);
      next.splice(nFrozen, 0, id);
      onChange(next);
      onFrozenChange(nFrozen + 1);
    }
  };

  // 组内拖列：只在已选列之间、同一个组、同一个区（固定区 / 滚动区）内换位
  const colDrag = usePointerReorder("data-col-id", (from, to) => {
    const fi = selected.indexOf(from);
    const ti = selected.indexOf(to);
    if (fi < 0 || ti < 0 || from === FIRST || to === FIRST) return;
    if (fi < nFrozen !== ti < nFrozen) return; // 固定与否只由图钉决定
    // 固定区里的列来自不同的组，可以互相换位；滚动区里只能在同一个组内换位（保持色带完整）
    if (fi >= nFrozen && BY_ID.get(from)?.group !== BY_ID.get(to)?.group) return;
    const next = selected.slice();
    next.splice(fi, 1);
    next.splice(ti, 0, from);
    onChange(next);
  });

  // 拖整组：该组在滚动区里的已选列整块挪到目标组的位置。目标组没有已选列时忽略。
  const groupDrag = usePointerReorder("data-group-id", (from, to) => {
    const blocks = toBlocks(selected, nFrozen);
    const a = blocks.findIndex((b) => b.group === from);
    const b = blocks.findIndex((x) => x.group === to);
    if (a < 0 || b < 0 || a === b) return;
    const next = blocks.slice();
    const [m] = next.splice(a, 1);
    next.splice(b, 0, m);
    onChange([...selected.slice(0, nFrozen), ...next.flatMap((x) => x.ids)]);
  });

  const matches = (label: string) => !q || label.toLowerCase().includes(q);
  const frozenIds = selected.slice(0, nFrozen);
  // 搜索时只看组里还会列出的列（固定列在顶部固定区显示，不在组里重复）；命中只来自固定列的组不用展开一个空壳
  const hasMatch = (g: string) =>
    !q || colsOf(g, selected, nFrozen).some((id) => matches(BY_ID.get(id)!.label));

  // 一列一行：固定区和各组里共用同一份渲染
  const colRow = (id: string) => {
    const c = BY_ID.get(id)!;
    const on = sel.has(id);
    const pinned = on && selected.indexOf(id) < nFrozen;
    return (
      <div
        key={id}
        data-col-id={on ? id : undefined}
        className={cn(
          "flex items-center gap-1.5 rounded-md px-1.5 py-0.5 select-none hover:bg-muted",
          colDrag.drag?.id === id && "bg-primary/10 ring-1 ring-primary/30",
        )}
      >
        <span
          onPointerDown={on && id !== FIRST ? colDrag.start(id) : undefined}
          aria-label="拖动排序"
          className={cn(
            "touch-none rounded p-0.5 text-foreground/60",
            on && id !== FIRST ? "cursor-grab hover:bg-background hover:text-foreground" : "opacity-20",
          )}
        >
          <DotsSix weight="bold" className="size-4" />
        </span>
        <label className="flex min-w-0 flex-1 cursor-pointer items-center gap-1.5">
          <Checkbox checked={on} disabled={id === FIRST} onCheckedChange={(v) => setCols([id], v === true)} />
          <span className={cn("truncate", !on && "text-muted-foreground")}>{c.label}</span>
          {/* 固定区里的列来自不同的组，标出所属组，免得认不出 */}
          {pinned && <span className="shrink-0 text-[10.5px] text-muted-foreground">{GROUPS[c.group].label}</span>}
        </label>
        <button
          type="button"
          disabled={id === FIRST || (!pinned && nFrozen >= MAX_FROZEN)}
          onClick={() => pin(id)}
          aria-label={pinned ? "取消固定" : "固定到左侧"}
          title={
            id === FIRST
              ? "发票号码始终固定"
              : pinned
                ? "取消固定"
                : nFrozen >= MAX_FROZEN
                  ? "固定列已达上限"
                  : "固定到左侧"
          }
          className={cn(
            "rounded p-0.5 hover:bg-background disabled:opacity-30",
            pinned ? "text-primary" : "text-foreground/40 hover:text-foreground",
          )}
        >
          <PushPin weight={pinned ? "fill" : "regular"} className="size-4" />
        </button>
      </div>
    );
  };

  const shownFrozen = frozenIds.filter((id) => matches(BY_ID.get(id)!.label));

  return (
    <Drawer open={open} onOpenChange={onOpenChange} swipeDirection="right">
      <DrawerContent className="h-dvh w-[min(440px,100vw)]">
        <DrawerHeader className="border-b pb-3">
          <DrawerTitle>选择列</DrawerTitle>
          <ClearableInput
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            onClear={() => setSearch("")}
            placeholder="搜索列…"
            className="mt-2"
          />
          <p className="mt-1 text-[11px] text-muted-foreground">
            已显示 {selected.length} / {COLS.length} 列，固定 {nFrozen} 列（最多 {MAX_FROZEN}）
          </p>
        </DrawerHeader>

        <div className="thin-scrollbar flex-1 overflow-y-auto px-4 py-2">
          {/* ── 固定列：始终在最顶部，不管下面的组展开还是收起 ── */}
          {(!q || shownFrozen.length > 0) && (
            <section className="mb-3 rounded-md border border-primary/30 bg-primary/5 p-1">
              <h3 className="flex items-center gap-1.5 px-1.5 py-0.5 text-[12px] font-semibold text-primary">
                <PushPin weight="fill" className="size-3.5" />
                固定在左侧
                <span className="font-mono text-[10.5px] font-normal text-muted-foreground">
                  {nFrozen}/{MAX_FROZEN}
                </span>
              </h3>
              {shownFrozen.map(colRow)}
            </section>
          )}

          {/* ── 各数据来源组（手风琴：同一时间只展开一个；搜索时命中的组全部展开） ── */}
          {ordered.filter(hasMatch).map((g) => {
            const meta = GROUPS[g];
            const ids = COLS.filter((c) => c.group === g).map((c) => c.id);
            const n = ids.filter((id) => sel.has(id)).length; // 含固定列：它们仍属于这个组
            const expanded = !!q || openGroup === g;
            const canDragGroup = selected.slice(nFrozen).some((id) => BY_ID.get(id)?.group === g);
            return (
              <section
                key={g}
                data-group-id={g}
                className={cn(
                  "mb-2 border-t-[3px] border-t-(--hue) pt-1",
                  groupDrag.drag?.id === g && "bg-primary/10",
                )}
                style={hueStyle(g)}
              >
                <h3 className="flex items-center gap-1.5 text-[13px] font-semibold text-(--hue)">
                  <button
                    type="button"
                    aria-expanded={expanded}
                    aria-label={expanded ? "折叠" : "展开"}
                    onClick={() => setOpenGroup(openGroup === g ? null : g)}
                    className="rounded p-0.5 hover:bg-muted"
                  >
                    {expanded ? <CaretDown className="size-3.5" /> : <CaretRight className="size-3.5" />}
                  </button>
                  <span
                    onPointerDown={canDragGroup ? groupDrag.start(g) : undefined}
                    aria-label="拖动整组"
                    title={canDragGroup ? "拖动整组" : "没有可移动的已选列"}
                    className={cn(
                      "touch-none rounded p-0.5",
                      canDragGroup ? "cursor-grab hover:bg-muted" : "opacity-30",
                    )}
                  >
                    <DotsSix weight="bold" className="size-4" />
                  </span>
                  <button
                    type="button"
                    onClick={() => setOpenGroup(openGroup === g ? null : g)}
                    className="min-w-0 truncate text-left"
                  >
                    {meta.label}
                  </button>
                  <span className="font-mono text-[10.5px] font-normal text-muted-foreground">{meta.card}</span>
                  <label className="ml-auto flex cursor-pointer items-center gap-1 text-[11px] font-normal text-muted-foreground">
                    <Checkbox
                      checked={n === ids.length}
                      indeterminate={n > 0 && n < ids.length}
                      onCheckedChange={(c) => setCols(ids, c === true)}
                    />
                    {n}/{ids.length}
                  </label>
                </h3>

                {expanded &&
                  colsOf(g, selected, nFrozen)
                    .filter((id) => matches(BY_ID.get(id)!.label))
                    .map(colRow)}
              </section>
            );
          })}
          {q && !ordered.some(hasMatch) && (
            <p className="py-8 text-center text-xs text-muted-foreground">没有匹配的列</p>
          )}
        </div>

        <DrawerFooter className="flex-row justify-end border-t pt-3">
          <AppButton
            type="button"
            variant="glass"
            onClick={() => {
              onChange([FIRST]);
              onFrozenChange(1);
            }}
          >
            清空
          </AppButton>
          <AppButton type="button" variant="brand" onClick={() => onOpenChange(false)}>
            完成
          </AppButton>
        </DrawerFooter>
      </DrawerContent>
    </Drawer>
  );
}
