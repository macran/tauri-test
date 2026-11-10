// DataTable 工具栏的"列设置"：搜索 + 点击行显隐 + 拖拽手柄排序。
// 拖拽用 pointer 事件而非 HTML5 draggable：tauri 开启文件拖放后 WebView 不触发网页内的 drag/drop。

import { Columns, DotsSix, MagnifyingGlass, PushPin } from "@phosphor-icons/react";
import { useState } from "react";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { cn } from "@/lib/utils";
import { usePointerReorder } from "./usePointerReorder";

export interface ColumnItem {
  id: string;
  label: string;
  visible: boolean;
  /** 是否固定在左侧 */
  pinned?: boolean;
}

export function ColumnConfigurator({
  items,
  onToggle,
  onMove,
  onTogglePin,
  pinFull,
}: {
  items: ColumnItem[];
  onToggle: (id: string) => void;
  /** 把 from 列挪到 to 列的位置 */
  onMove: (from: string, to: string) => void;
  /** 传入后每列出现图钉按钮（固定/取消固定） */
  onTogglePin?: (id: string) => void;
  /** 固定列已达上限：未固定的列图钉禁用 */
  pinFull?: boolean;
}) {
  const [search, setSearch] = useState("");
  const { drag, start, justDragged } = usePointerReorder("data-item-id", onMove);

  const q = search.trim().toLowerCase();
  const shown = q ? items.filter((it) => it.label.toLowerCase().includes(q)) : items;
  const visibleCount = items.filter((it) => it.visible).length;

  return (
    <Popover onOpenChange={(open) => !open && setSearch("")}>
      <PopoverTrigger render={<Button type="button" variant="outline" size="sm" />}>
        <Columns data-icon="inline-start" />
        列设置
      </PopoverTrigger>
      <PopoverContent align="end" className="w-72 gap-2">
        <div className="relative">
          <MagnifyingGlass className="absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
          <Input
            value={search}
            onChange={(e) => setSearch(e.target.value)}
            placeholder="搜索列…"
            className="pl-7"
          />
        </div>
        <p className="text-[11px] text-muted-foreground">
          已显示 {visibleCount} / {items.length} 列 · 点击切换显隐，拖动右侧手柄调整顺序{onTogglePin && "，图钉固定到左侧"}
        </p>
        <div className="thin-scrollbar max-h-80 overflow-y-auto">
          {shown.map((it) => (
            <div
              key={it.id}
              data-item-id={it.id}
              onClick={() => !justDragged() && onToggle(it.id)}
              className={cn(
                "flex cursor-pointer items-center gap-2 rounded-md px-2 py-1 select-none hover:bg-muted",
                drag?.id === it.id && "bg-primary/10 ring-1 ring-primary/30",
              )}
            >
              <Checkbox checked={it.visible} className="pointer-events-none" tabIndex={-1} />
              <span className={cn("min-w-0 flex-1 truncate", !it.visible && "text-muted-foreground")}>
                {it.label}
              </span>
              {onTogglePin && (
                <button
                  type="button"
                  onClick={(e) => {
                    e.stopPropagation();
                    onTogglePin(it.id);
                  }}
                  disabled={!it.pinned && pinFull}
                  aria-label={it.pinned ? "取消固定" : "固定到左侧"}
                  title={it.pinned ? "取消固定" : pinFull ? "固定列已达上限" : "固定到左侧"}
                  className={cn(
                    "rounded p-0.5 hover:bg-background disabled:opacity-30",
                    it.pinned ? "text-primary" : "text-foreground/50 hover:text-foreground",
                  )}
                >
                  <PushPin weight={it.pinned ? "fill" : "regular"} className="size-4" />
                </button>
              )}
              <span
                onClick={(e) => e.stopPropagation()}
                onPointerDown={start(it.id)}
                aria-label="拖动排序"
                className="cursor-grab touch-none rounded p-0.5 text-foreground/70 hover:bg-background hover:text-foreground"
              >
                <DotsSix weight="bold" className="size-5" />
              </span>
            </div>
          ))}
          {shown.length === 0 && (
            <p className="py-4 text-center text-xs text-muted-foreground">没有匹配的列</p>
          )}
        </div>
      </PopoverContent>
    </Popover>
  );
}
