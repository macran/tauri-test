import { useEffect, useMemo, useState } from "react";
import { stApi } from "../api";
import { ratePpmLabel } from "../format";
import type { StampCategory } from "../types";

export interface CategoryGroup {
  parent: StampCategory;
  children: StampCategory[]; // 可选的子目（有税率）；无子目且自身有税率时 = [parent]
}

export function useStampCategories() {
  const [list, setList] = useState<StampCategory[]>([]);
  useEffect(() => {
    stApi.listCategories().then(setList).catch(() => setList([]));
  }, []);
  const groups = useMemo<CategoryGroup[]>(() => {
    const parents = list.filter((c) => !c.parentCode);
    return parents.map((p) => {
      const kids = list.filter((c) => c.parentCode === p.code && c.ratePpm != null);
      return { parent: p, children: kids.length ? kids : p.ratePpm != null ? [p] : [] };
    });
  }, [list]);
  const byCode = useMemo(() => new Map(list.map((c) => [c.code, c])), [list]);
  return { list, groups, byCode };
}

/** 原生 select（虚拟表格内性能最好），按税目分组 */
export function CategorySelect({
  value,
  groups,
  onChange,
  disabled,
  ariaLabel,
  className,
}: {
  value: string | null;
  groups: CategoryGroup[];
  onChange: (code: string) => void;
  disabled?: boolean;
  ariaLabel: string;
  className?: string;
}) {
  return (
    <select
      aria-label={ariaLabel}
      className={
        className ??
        "h-7 w-full rounded border bg-background px-1 text-xs focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring disabled:opacity-60"
      }
      value={value ?? ""}
      disabled={disabled}
      onChange={(e) => e.target.value && onChange(e.target.value)}
    >
      <option value="">— 选择子目 —</option>
      {groups
        .filter((g) => g.children.length)
        .map((g) => (
          <optgroup key={g.parent.code} label={g.parent.name}>
            {g.children.map((c) => (
              <option key={c.code} value={c.code}>
                {c.name}（{ratePpmLabel(c.ratePpm)}）
              </option>
            ))}
          </optgroup>
        ))}
    </select>
  );
}
