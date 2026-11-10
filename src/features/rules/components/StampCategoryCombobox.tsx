import { ChevronsUpDown } from "lucide-react";
import { useMemo, useState } from "react";

import type { StampTaxCategoryOption } from "@/bindings";
import { Button } from "@/components/ui/button";
import {
  Command,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from "@/components/ui/command";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { cn } from "@/lib/utils";

type Node = StampTaxCategoryOption & { children: StampTaxCategoryOption[] };

/** 300 ppm → "0.03%"，与复核表下拉一致 */
const rateText = (ppm: number) => `${ppm / 10000}%`;

/**
 * 印花税税目选择：两级（税目 → 子目）、可搜索、显示税率。
 * 税率为空的税目（如“产权转移书据”）本身不可选，必须选子目。
 * 搜税目名带出其全部子目；搜子目名带出所属税目。
 * value / onChange 使用税目名称（与规则的 rule_category 一致，Excel 导入也按名称匹配）。
 */
export function StampCategoryCombobox({
  options,
  value,
  onChange,
  id,
  invalid,
}: {
  options: StampTaxCategoryOption[];
  value: string;
  onChange: (name: string) => void;
  id?: string;
  invalid?: boolean;
}) {
  const [open, setOpen] = useState(false);
  const [search, setSearch] = useState("");

  const tree = useMemo<Node[]>(
    () =>
      options
        .filter((o) => !o.parentCode)
        .map((p) => ({ ...p, children: options.filter((c) => c.parentCode === p.code) })),
    [options],
  );
  const byCode = useMemo(() => new Map(options.map((o) => [o.code, o])), [options]);

  // shortcut: 名称重名时只会选中第一个，税目表目前无重名；重名后改为按 code 存
  const selected = options.find((o) => o.name === value);
  const selectedParent = selected?.parentCode ? byCode.get(selected.parentCode) : undefined;

  const pick = (item: StampTaxCategoryOption) => {
    onChange(item.name);
    setOpen(false);
  };

  return (
    <Popover
      open={open}
      onOpenChange={(next) => {
        setOpen(next);
        if (!next) setSearch("");
      }}
    >
      <PopoverTrigger
        render={
          <Button
            id={id}
            type="button"
            variant="outline"
            role="combobox"
            aria-expanded={open}
            aria-invalid={invalid}
            className={cn("w-full justify-between font-normal", !selected && "text-muted-foreground")}
          />
        }
      >
        <span className="truncate">
          {selected
            ? selectedParent
              ? `${selectedParent.name} › ${selected.name}`
              : selected.name
            : "请选择印花税税目"}
        </span>
        <ChevronsUpDown className="size-4 shrink-0 opacity-50" />
      </PopoverTrigger>

      <PopoverContent align="start" className="w-(--anchor-width) min-w-[420px] p-0">
        <Command>
          <CommandInput placeholder="搜索税目或子目…" value={search} onValueChange={setSearch} />
          <CommandList className="max-h-80">
            <CommandEmpty>没有匹配的税目</CommandEmpty>
            {tree.map((parent) => {
              const selectable = parent.ratePpm != null;
              return (
                <CommandGroup key={parent.code}>
                  <CommandItem
                    value={parent.code}
                    keywords={[parent.name, ...parent.children.map((c) => c.name)]}
                    disabled={!selectable}
                    data-checked={value === parent.name}
                    onSelect={() => pick(parent)}
                    className="font-medium"
                  >
                    <span className="min-w-0 flex-1 truncate">{parent.name}</span>
                    <span className="text-xs text-muted-foreground">
                      {selectable ? rateText(parent.ratePpm!) : "须选择子目"}
                    </span>
                  </CommandItem>

                  {parent.children.map((child) => (
                    <CommandItem
                      key={child.code}
                      value={child.code}
                      keywords={[child.name, parent.name]}
                      data-checked={value === child.name}
                      onSelect={() => pick(child)}
                      className="pl-6"
                    >
                      <span className="min-w-0 flex-1 truncate" title={child.name}>
                        {child.name}
                        {search && (
                          <span className="ml-2 text-xs text-muted-foreground">· {parent.name}</span>
                        )}
                      </span>
                      {child.ratePpm != null && (
                        <span className="text-xs text-muted-foreground">{rateText(child.ratePpm)}</span>
                      )}
                    </CommandItem>
                  ))}
                </CommandGroup>
              );
            })}
          </CommandList>
        </Command>
      </PopoverContent>
    </Popover>
  );
}
