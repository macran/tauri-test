// src/components/query/LayoutSelect.tsx
//
// 查询表单里的"布局"下拉：每次只能选一个，默认 = 不选。选择立即作用于结果表格。

import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { FieldShell } from "./fields";
import { type LayoutItem, SYSTEM_DEFAULT_LABEL, type TableLayouts } from "./table-layout";

const SYSTEM = "system";

function label(i: LayoutItem) {
  return [
    i.name,
    i.isMine ? "" : ` · ${i.owner}`,
    i.isPrivate ? "（私有）" : "",
    i.isMine && i.isDefault ? "（我的默认）" : "",
  ].join("");
}

export function LayoutSelect({
  layouts,
  hideLabel = false,
}: {
  layouts: TableLayouts;
  /** 外层已有标签（如表单里的 Field）时隐藏自带的“布局”标签 */
  hideLabel?: boolean;
}) {
  const items = [
    { value: SYSTEM, label: SYSTEM_DEFAULT_LABEL },
    ...layouts.items.map((i) => ({ value: String(i.id), label: label(i) })),
  ];
  const select = (
      <Select
        items={items}
        value={layouts.selected === null ? SYSTEM : String(layouts.selected)}
        onValueChange={(v) => layouts.setSelected(v === SYSTEM ? null : Number(v))}
      >
        <SelectTrigger className="w-full min-w-36">
          <SelectValue />
        </SelectTrigger>
        <SelectContent>
          {items.map((i) => (
            <SelectItem key={i.value} value={i.value}>
              {i.label}
            </SelectItem>
          ))}
        </SelectContent>
      </Select>
  );
  return hideLabel ? select : <FieldShell label="布局">{select}</FieldShell>;
}
