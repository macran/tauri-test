// src/components/query/overview/OverviewTable.tsx
//
// 综合查询专用结果表：行虚拟化（react-virtual，行高随内容测量）+ 两层表头
// （来源分组色带 / 字段）+ 多值记录「槽位」。
//  - 汇总模式下多值用 " / " 连接，或逐条堆叠；悬停任一值，同一行内同组同序号的值一起高亮
//  - 展开模式下被展开的组逐行一条记录，表头打"逐行"标记
//  - 第一列固定为发票号码，点击打开该 item 的关联记录；列宽可拖拽调整

import { useVirtualizer } from "@tanstack/react-virtual";
import { Fragment, useRef } from "react";
import type { ItemOverviewRow } from "@/bindings";
import {
  type ColDef,
  cellText,
  GROUPS,
  hueStyle,
  isExpanded,
  type OvRow,
} from "./columns";

const MIN_W = 60;
const MAX_W = 800;
const BAND_HEIGHT = 30;
const FIELD_HEIGHT = 44; // 字段行（带列宽拖拽手柄）
const cell = "border-r border-b px-2 py-1.5 align-top whitespace-nowrap";

function Slots({ c, row }: { c: ColDef; row: OvRow }) {
  const slot = (i: number, text: string, key = i) => (
    <span
      key={key}
      className="ov-slot"
      data-g={c.group}
      data-i={i}
      data-n={i + 1}
    >
      {text}
    </span>
  );
  if (isExpanded(c.group, row.expand)) {
    if (row.none || !row.rec)
      return <span className="text-muted-foreground italic">无记录</span>;
    return slot(row.idx ?? 0, c.fmt(c.get(row.rec, row.it)));
  }
  const recs = row.it.groups[c.group];
  if (!recs) return <span className="text-muted-foreground">—</span>; // 此 item 不适用该组
  if (!recs.length)
    return <span className="text-muted-foreground italic">无记录</span>;
  return (
    <>
      {recs.map((r, i) => (
        <Fragment key={i}>
          {i > 0 && (
            <span className="stack-hide mx-0.5 text-muted-foreground">/</span>
          )}
          {slot(i, c.fmt(c.get(r, row.it)))}
        </Fragment>
      ))}
    </>
  );
}

export function OverviewTable({
  rows,
  cols,
  stack,
  loading,
  sizes,
  onResize,
  onOpenItem,
}: {
  rows: OvRow[];
  cols: ColDef[];
  stack: boolean;
  loading: boolean;
  /** 用户拖拽后的列宽（列 id → px），没拖过的列用默认宽度 */
  sizes: Record<string, number>;
  onResize: (id: string, width: number) => void;
  onOpenItem: (it: ItemOverviewRow) => void;
}) {
  const scrollRef = useRef<HTMLDivElement>(null);
  const lit = useRef<HTMLElement[]>([]);
  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => 34,
    overscan: 10,
  });
  const items = virtualizer.getVirtualItems();
  const padTop = items.length ? items[0].start : 0;
  const padBottom = items.length
    ? virtualizer.getTotalSize() - items[items.length - 1].end
    : 0;

  const frozen = cols[0]?.id === "header_inv_unq_key";
  const span = cols.length;
  const widthOf = (c: ColDef) => sizes[c.id] ?? c.size;
  const width = cols.reduce((a, c) => a + widthOf(c), 0);

  const startResize = (e: React.MouseEvent, c: ColDef) => {
    e.preventDefault();
    const x0 = e.clientX;
    const w0 = widthOf(c);
    const move = (m: MouseEvent) =>
      onResize(c.id, Math.min(MAX_W, Math.max(MIN_W, w0 + m.clientX - x0)));
    const up = () => {
      window.removeEventListener("mousemove", move);
      window.removeEventListener("mouseup", up);
    };
    window.addEventListener("mousemove", move);
    window.addEventListener("mouseup", up);
  };
  const expand = rows[0]?.expand ?? null;

  // 连续同组的列合并成一个色带单元；冻结的发票号码列单独成带
  const bands: { group: string; n: number; first: boolean }[] = [];
  cols.forEach((c, i) => {
    const last = bands[bands.length - 1];
    if (last && last.group === c.group && !(frozen && i === 1)) last.n++;
    else bands.push({ group: c.group, n: 1, first: frozen && i === 0 });
  });

  const highlight = (e: React.MouseEvent) => {
    for (const x of lit.current) x.classList.remove("lit");
    lit.current = [];
    const s = (e.target as HTMLElement).closest<HTMLElement>(".ov-slot");
    if (!s) return;
    const tr = s.closest("tr");
    if (!tr) return;
    lit.current = [
      ...tr.querySelectorAll<HTMLElement>(
        `.ov-slot[data-g="${s.dataset.g}"][data-i="${s.dataset.i}"]`
      ),
    ];
    for (const x of lit.current) x.classList.add("lit");
    // 被截断的单元格悬停时才挂 title
    const td = s.closest("td");
    if (td && !td.title && td.scrollWidth > td.clientWidth)
      td.title = td.textContent ?? "";
  };

  const stickyFirst = { left: 0 };
  return (
    <div
      ref={scrollRef}
      className="thin-scrollbar overflow-auto rounded-md border bg-background"
      style={{ maxHeight: 640 }}
    >
      <table
        className={`table-fixed border-separate border-spacing-0 text-xs ${stack ? "ov-stack" : ""}`}
        style={{ width }}
      >
        <colgroup>
          {cols.map((c) => (
            <col key={c.id} style={{ width: widthOf(c) }} />
          ))}
        </colgroup>
        <thead>
          <tr>
            {bands.map((b, i) => {
              const g = GROUPS[b.group];
              const exp = isExpanded(b.group, expand);
              return (
                <th
                  key={i}
                  colSpan={b.n}
                  className={`sticky top-0 border-r border-b border-t-[3px] border-t-(--hue) bg-(--bandbg) px-2 text-left font-semibold text-(--hue) ${b.first ? "z-30" : "z-20"}`}
                  style={{
                    ...hueStyle(b.group),
                    height: BAND_HEIGHT,
                    ...(b.first ? stickyFirst : null),
                    ...(exp
                      ? { boxShadow: "inset 0 -2px 0 var(--hue)" }
                      : null),
                  }}
                >
                  {g.label}
                  <span className="ml-1.5 font-mono text-[10.5px] font-normal text-muted-foreground">
                    {g.card}
                  </span>
                  {exp && (
                    <span className="ml-1.5 rounded-sm bg-(--hue) px-1 text-[10px] text-white">
                      按此展开
                    </span>
                  )}
                </th>
              );
            })}
          </tr>
          <tr>
            {cols.map((c, i) => {
              const g = GROUPS[c.group];
              return (
                <th
                  key={c.id}
                  className={`sticky border-r border-b bg-background px-2 pr-3 font-medium text-muted-foreground ${c.align ? "text-right" : "text-left"} ${frozen && i === 0 ? "z-30" : "z-20"}`}
                  style={{
                    ...hueStyle(c.group),
                    top: BAND_HEIGHT,
                    height: FIELD_HEIGHT,
                    ...(frozen && i === 0 ? stickyFirst : null),
                  }}
                >
                  {c.label}
                  {g.multi && (
                    <span className="ml-1 rounded-sm border border-(--hue) px-1 text-[9.5px] font-normal text-(--hue)">
                      {isExpanded(c.group, expand) ? "逐行" : "多值"}
                    </span>
                  )}
                  <div
                    onMouseDown={(e) => startResize(e, c)}
                    className="absolute top-0 right-0 h-full w-1.5 cursor-col-resize select-none hover:bg-primary/40"
                  />
                </th>
              );
            })}
          </tr>
        </thead>
        <tbody onMouseOver={highlight}>
          {loading || rows.length === 0 ? (
            <tr>
              <td
                colSpan={span}
                className="h-24 text-center text-muted-foreground"
              >
                {loading
                  ? "正在查询……"
                  : "没有符合条件的发票明细。调整查询条件后再试。"}
              </td>
            </tr>
          ) : (
            <>
              {padTop > 0 && (
                <tr>
                  <td colSpan={span} style={{ height: padTop, padding: 0 }} />
                </tr>
              )}
              {items.map((v) => {
                const row = rows[v.index];
                return (
                  <tr
                    key={row.key}
                    data-index={v.index}
                    ref={virtualizer.measureElement}
                    className="hover:bg-muted/40"
                  >
                    {cols.map((c, i) => (
                      <td
                        key={c.id}
                        className={`${cell} overflow-hidden text-ellipsis ${c.align ? "text-right tabular-nums" : ""} ${frozen && i === 0 ? "sticky z-10 bg-background" : ""} ${row.idx && !GROUPS[c.group].multi ? "text-muted-foreground" : ""}`}
                        style={{
                          ...hueStyle(c.group),
                          ...(frozen && i === 0 ? stickyFirst : null),
                        }}
                      >
                        {frozen && i === 0 ? (
                          <button
                            type="button"
                            className="block max-w-full truncate text-left font-medium text-primary hover:text-primary/80"
                            title="查看关联记录"
                            onClick={() => onOpenItem(row.it)}
                          >
                            {cellText(c, row.it)}
                            {row.expand && (
                              <small className="block font-normal text-muted-foreground">
                                {row.none
                                  ? "无此类记录"
                                  : `第 ${(row.idx ?? 0) + 1} 条`}
                              </small>
                            )}
                          </button>
                        ) : GROUPS[c.group].multi ? (
                          <Slots c={c} row={row} />
                        ) : (
                          cellText(c, row.it)
                        )}
                      </td>
                    ))}
                  </tr>
                );
              })}
              {padBottom > 0 && (
                <tr>
                  <td
                    colSpan={span}
                    style={{ height: padBottom, padding: 0 }}
                  />
                </tr>
              )}
            </>
          )}
        </tbody>
      </table>
    </div>
  );
}
