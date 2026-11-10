import { useVirtualizer } from "@tanstack/react-virtual";
import { type ReactNode, useRef } from "react";
import { cn } from "@/lib/utils";

export interface Column<T> {
  key: string;
  header: ReactNode;
  width: number;
  align?: "left" | "right" | "center";
  /** 固定在左侧 */
  sticky?: boolean;
  cell: (row: T, index: number) => ReactNode;
}

interface Props<T> {
  rows: T[];
  columns: Column<T>[];
  rowKey: (row: T) => string | number;
  rowClassName?: (row: T) => string | undefined;
  rowHeight?: number;
  className?: string;
  empty?: ReactNode;
}

/** 轻量虚拟滚动表格（数万行明细不卡）。列宽固定，横向可滚动，表头吸顶。 */
export function VirtualTable<T>({
  rows,
  columns,
  rowKey,
  rowClassName,
  rowHeight = 34,
  className,
  empty,
}: Props<T>) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const v = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => rowHeight,
    overscan: 16,
  });
  const totalWidth = columns.reduce((s, c) => s + c.width, 0);
  let stickyLeft = 0;
  const stickyOffsets = columns.map((c) => {
    const left = stickyLeft;
    if (c.sticky) stickyLeft += c.width;
    return left;
  });
  const alignCls = (a?: string) => (a === "right" ? "justify-end text-right" : a === "center" ? "justify-center" : "");

  return (
    <div
      ref={scrollRef}
      className={cn("relative min-h-0 overflow-auto rounded-md border bg-background", className)}
      role="table"
      aria-rowcount={rows.length}
    >
      <div style={{ width: totalWidth }} className="sticky top-0 z-20 flex border-b bg-muted text-xs font-medium text-muted-foreground" role="row">
        {columns.map((c, i) => (
          <div
            key={c.key}
            role="columnheader"
            className={cn("flex shrink-0 items-center px-2 py-2", alignCls(c.align), c.sticky && "sticky z-10 bg-muted")}
            style={{ width: c.width, left: c.sticky ? stickyOffsets[i] : undefined }}
          >
            {c.header}
          </div>
        ))}
      </div>
      {rows.length === 0 ? (
        <div className="flex h-40 items-center justify-center text-sm text-muted-foreground">{empty ?? "暂无数据"}</div>
      ) : (
        <div style={{ height: v.getTotalSize(), width: totalWidth }} className="relative">
          {v.getVirtualItems().map((vi) => {
            const row = rows[vi.index];
            return (
              <div
                key={rowKey(row)}
                role="row"
                className={cn("absolute left-0 flex border-b text-sm hover:bg-muted/50", rowClassName?.(row))}
                style={{ top: 0, transform: `translateY(${vi.start}px)`, height: rowHeight, width: totalWidth }}
              >
                {columns.map((c, i) => (
                  <div
                    key={c.key}
                    role="cell"
                    className={cn(
                      "flex shrink-0 items-center overflow-hidden px-2 whitespace-nowrap",
                      alignCls(c.align),
                      c.sticky && "sticky z-10 bg-inherit",
                    )}
                    style={{ width: c.width, left: c.sticky ? stickyOffsets[i] : undefined }}
                  >
                    {c.cell(row, vi.index)}
                  </div>
                ))}
              </div>
            );
          })}
        </div>
      )}
    </div>
  );
}

/** 截断文本 + title 悬浮显示全部 */
export function Clip({ children, className }: { children: ReactNode; className?: string }) {
  return (
    <span className={cn("truncate", className)} title={typeof children === "string" ? children : undefined}>
      {children}
    </span>
  );
}
