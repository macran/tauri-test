// src/components/query/overview/ColumnPanel.tsx
//
// 综合查询的"选择列"侧栏：按数据来源分组，多值记录组带基数标记，可整组勾选。
// 选中的列按注册表顺序排列，发票号码固定在最前。

import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import {
  Drawer,
  DrawerContent,
  DrawerFooter,
  DrawerHeader,
  DrawerTitle,
} from "@/components/ui/drawer";
import { COLS, GROUPS, hueStyle } from "./columns";

const FIRST = "header_inv_unq_key";
const ORDER = Object.fromEntries(
  COLS.map((c, i) => [c.id, c.id === FIRST ? -1 : i])
);

/** 按注册表顺序整理列 id，丢弃已不存在的 */
export const sortCols = (ids: string[]) =>
  [...new Set([FIRST, ...ids])]
    .filter((id) => id in ORDER)
    .sort((a, b) => ORDER[a] - ORDER[b]);

export function ColumnPanel({
  open,
  onOpenChange,
  selected,
  onChange,
}: {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  selected: string[];
  onChange: (ids: string[]) => void;
}) {
  const sel = new Set(selected);
  const toggle = (ids: string[], on: boolean) =>
    onChange(
      sortCols(
        on ? [...selected, ...ids] : selected.filter((x) => !ids.includes(x))
      )
    );

  return (
    <Drawer open={open} onOpenChange={onOpenChange} swipeDirection="right">
      <DrawerContent className="h-dvh w-[min(440px,100vw)]">
        <DrawerHeader className="border-b pb-3">
          <DrawerTitle>选择列</DrawerTitle>
        </DrawerHeader>
        <div className="thin-scrollbar flex-1 overflow-y-auto px-4 py-2">
          <p className="text-[11px] text-muted-foreground">
            按数据来源分组。标有“0–n”的组是多值记录组；特种类型组只对对应类型的发票有值，其余显示“—”。
          </p>
          {Object.entries(GROUPS).map(([g, meta]) => {
            const cs = COLS.filter((c) => c.group === g);
            const ids = cs.map((c) => c.id);
            return (
              <section
                key={g}
                className="mt-3 border-t-[3px] border-t-(--hue) pt-1.5"
                style={hueStyle(g)}
              >
                <h3 className="mb-1 flex items-center gap-2 text-[13px] font-semibold text-(--hue)">
                  {meta.label}
                  <span className="font-mono text-[10.5px] font-normal text-muted-foreground">
                    {meta.card}
                  </span>
                  <label className="ml-auto flex cursor-pointer items-center gap-1 text-[11px] font-normal text-muted-foreground">
                    <Checkbox
                      checked={ids.every((id) => sel.has(id))}
                      onCheckedChange={(c) => toggle(ids, c === true)}
                    />
                    整组
                  </label>
                </h3>
                <div className="grid grid-cols-2 gap-x-3 gap-y-0.5">
                  {cs.map((c) => (
                    <label
                      key={c.id}
                      className="flex cursor-pointer items-center gap-1.5 py-0.5"
                    >
                      <Checkbox
                        checked={sel.has(c.id)}
                        disabled={c.id === FIRST}
                        onCheckedChange={(on) => toggle([c.id], on === true)}
                      />
                      {c.label}
                    </label>
                  ))}
                </div>
              </section>
            );
          })}
        </div>
        <DrawerFooter className="flex-row justify-end border-t pt-3">
          <Button
            type="button"
            variant="ghost"
            onClick={() => onChange([FIRST])}
          >
            清空
          </Button>
          <Button type="button" onClick={() => onOpenChange(false)}>
            完成
          </Button>
        </DrawerFooter>
      </DrawerContent>
    </Drawer>
  );
}
