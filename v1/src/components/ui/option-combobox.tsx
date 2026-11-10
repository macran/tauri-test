"use client"

// 多选下拉：基于 ui/combobox 封装。
//  - 触发器：未选显示 placeholder（默认"全部"）；选 1 项显示名称；多于 1 项显示"名称 +N"
//  - 弹层：顶部搜索、可多选列表、底部"全选/清除"
//  - 有选中项时触发器右侧出现 × 一键清空
// 查询表单与表格列筛选共用。

import { Combobox as ComboboxPrimitive } from "@base-ui/react"
import { CheckIcon, FunnelIcon, XIcon } from "lucide-react"

import { Button } from "@/components/ui/button"
import {
  Combobox,
  ComboboxContent,
  ComboboxEmpty,
  ComboboxInput,
  ComboboxItem,
  ComboboxList,
  ComboboxTrigger,
} from "@/components/ui/combobox"
import { cn } from "@/lib/utils"

export interface ComboboxOption<V extends string | number> {
  value: V
  label: string
}

export function OptionCombobox<V extends string | number>({
  options,
  value,
  onChange,
  placeholder = "全部",
  className,
  variant = "field",
}: {
  options: ComboboxOption<V>[]
  value: V[]
  onChange: (next: V[]) => void
  placeholder?: string
  className?: string
  /** field：表单输入框样式；icon：仅一个筛选图标（表头列筛选用），有选中时高亮 */
  variant?: "field" | "icon"
}) {
  const selected = options.filter((o) => value.includes(o.value))

  const isIcon = variant === "icon"

  return (
    <div className={cn("relative", isIcon ? "inline-flex" : "w-full", className)}>
      <Combobox
        multiple
        items={options}
        value={selected}
        onValueChange={(next: ComboboxOption<V>[]) =>
          onChange(next.map((o) => o.value))
        }
        itemToStringLabel={(o: ComboboxOption<V>) => o.label}
        isItemEqualToValue={(a: ComboboxOption<V>, b: ComboboxOption<V>) =>
          a.value === b.value
        }
      >
        {isIcon ? (
          <ComboboxPrimitive.Trigger
            aria-label="筛选"
            className={cn(
              "inline-flex size-5 items-center justify-center rounded-none text-muted-foreground hover:bg-accent hover:text-foreground",
              selected.length > 0 && "bg-primary/10 text-primary"
            )}
          >
            <FunnelIcon className="size-3" />
          </ComboboxPrimitive.Trigger>
        ) : (
        <ComboboxTrigger
          className={cn(
            "flex h-8 w-full items-center justify-between gap-1.5 rounded-none border border-input bg-transparent py-1 pr-2 pl-2.5 text-xs transition-colors outline-none select-none focus-visible:border-ring focus-visible:ring-1 focus-visible:ring-ring/50 disabled:cursor-not-allowed disabled:opacity-50 dark:bg-input/30 dark:hover:bg-input/50"
          )}
        >
          <span
            className={cn(
              "flex min-w-0 flex-1 items-center gap-1.5 text-left",
              selected.length > 0 && "pr-6"
            )}
          >
            {selected.length === 0 ? (
              <span className="truncate text-muted-foreground">{placeholder}</span>
            ) : (
              <>
                <span className="truncate">{selected[0].label}</span>
                {selected.length > 1 && (
                  <span className="shrink-0 bg-muted px-1.5 text-[11px] font-medium text-muted-foreground">
                    +{selected.length - 1}
                  </span>
                )}
              </>
            )}
          </span>
        </ComboboxTrigger>
        )}

        <ComboboxContent className={isIcon ? "w-64 min-w-56" : "min-w-56"}>
          <ComboboxInput showTrigger={false} placeholder="搜索…" />
          <ComboboxEmpty>无匹配项</ComboboxEmpty>
          <ComboboxList>
            {(o: ComboboxOption<V>) => (
              <ComboboxItem key={o.value} value={o}>
                {o.label}
              </ComboboxItem>
            )}
          </ComboboxList>
          <div className="flex items-center justify-between border-t px-1 py-1">
            <Button
              type="button"
              variant="ghost"
              size="xs"
              onClick={() => onChange(options.map((o) => o.value))}
            >
              <CheckIcon data-icon="inline-start" />
              全选
            </Button>
            <Button
              type="button"
              variant="ghost"
              size="xs"
              disabled={selected.length === 0}
              onClick={() => onChange([])}
            >
              清除
            </Button>
          </div>
        </ComboboxContent>
      </Combobox>

      {!isIcon && selected.length > 0 && (
        <button
          type="button"
          aria-label="清除"
          onClick={() => onChange([])}
          className="absolute top-1/2 right-8 -translate-y-1/2 text-muted-foreground hover:text-foreground"
        >
          <XIcon className="size-3.5" />
        </button>
      )}
    </div>
  )
}
