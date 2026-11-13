"use client"

// 多选下拉：基于 ui/combobox 封装。
// - 触发器：未选显示 placeholder（默认"全部"）；选 1 项显示名称；多于 1 项显示"名称 +N"
// - 弹层：顶部搜索、可多选列表、底部"全选/清除"
// - 有选中项时触发器右侧出现 × 一键清空
// 查询表单与表格列筛选共用。

import { Combobox as ComboboxPrimitive } from "@base-ui/react"
import { CheckIcon, FunnelIcon, XIcon } from "lucide-react"
import { useState } from "react"

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
  const [search, setSearch] = useState("")

  const isIcon = variant === "icon"

  // 与 Combobox 自己过滤列表用同一个过滤器（Intl.Collator，忽略大小写 / 重音 / 标点），
  // 这样「列表里显示的」和「全选选中的」一定是同一批
  const { contains } = ComboboxPrimitive.useFilter()

  // 搜索框里有内容时，列表只显示匹配项，「全选」只作用于这些；没有搜索词则是全部
  const keyword = search.trim()
  const visible = keyword
    ? options.filter((o) => contains(o, keyword, (x: ComboboxOption<V>) => x.label))
    : options
  const filtering = keyword !== ""

  return (
    <div className={cn("relative", isIcon ? "inline-flex" : "w-full", className)}>
      <Combobox
        multiple
        items={options}
        value={selected}
        onValueChange={(next: ComboboxOption<V>[]) =>
          onChange(next.map((o) => o.value))
        }
        // 点选条目后保留搜索词，方便继续在过滤结果中多选（失焦/关闭时仍会清空）
        inputValue={search}
        onInputValueChange={(v, details) => {
          if (details.reason === "input-clear" && (details as { isItemPress?: boolean }).isItemPress) {
            details.cancel()
            return
          }
          setSearch(v)
        }}
        itemToStringLabel={(o: ComboboxOption<V>) => o.label}
        isItemEqualToValue={(a: ComboboxOption<V>, b: ComboboxOption<V>) =>
          a.value === b.value
        }
      >
        {isIcon ? (
          <ComboboxPrimitive.Trigger
            aria-label="筛选"
            className={cn(
              "inline-flex size-5 items-center justify-center rounded-none text-muted-foreground hover:bg-muted hover:text-foreground",
              selected.length > 0 && "bg-primary/10 text-primary"
            )}
          >
            <FunnelIcon className="size-3" />
          </ComboboxPrimitive.Trigger>
        ) : (
          <ComboboxTrigger
            className={cn(
              "flex h-(--control-h) w-full items-center justify-between gap-1.5 rounded-md border border-field-border bg-field-bg hover:border-field-border-hover py-1 pr-2 pl-2.5 text-xs transition-[border-color,box-shadow] duration-150 motion-reduce:transition-none outline-none select-none [--field-accent:var(--blue-primary)] dark:[--field-accent:var(--accent-amber)] focus-visible:border-[var(--field-accent)] focus-visible:shadow-[inset_0_-2px_0_var(--field-accent)] disabled:cursor-not-allowed disabled:opacity-50"
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
          <ComboboxInput
            showTrigger={false}
            placeholder="搜索…"
            value={search}
            onClearText={() => setSearch("")}
          />
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
              disabled={visible.length === 0}
              onClick={() =>
                // 有搜索词：把筛选出来的项并入已选（已选的、搜索之外的不动）；无搜索词：全部
                onChange(
                  filtering
                    ? [...new Set([...value, ...visible.map((o) => o.value)])]
                    : options.map((o) => o.value),
                )
              }
            >
              <CheckIcon data-icon="inline-start" />
              {filtering ? `全选筛选结果（${visible.length}）` : "全选"}
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
