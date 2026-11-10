// 分类页通用外壳：一个大类下多个子功能，选中哪个就展示哪个。
// 顶部下拉切换（查询页）。
// 访问过的子页保持挂载，切走再切回不丢表单/查询结果。
import { useState, type ComponentType } from 'react'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Tabs, TabsContent } from '@/components/ui/tabs'

export interface SubTabEntry {
  value: string
  label: string
  Component: ComponentType
}

export default function SubTabs({
  title,
  entries,
}: {
  title: string
  entries: SubTabEntry[]
}) {
  const [active, setActive] = useState(entries[0].value)
  const [visited, setVisited] = useState(() => new Set([entries[0].value]))

  const change = (value: unknown) => {
    if (typeof value !== 'string') return
    setActive(value)
    setVisited((prev) => (prev.has(value) ? prev : new Set(prev).add(value)))
  }

  const panels = entries.map(({ value, Component }) => (
    <TabsContent key={value} value={value} keepMounted className="min-h-0 min-w-0 overflow-hidden">
      {visited.has(value) && <Component />}
    </TabsContent>
  ))

  return (
    <Tabs value={active} onValueChange={change} className="h-full min-h-0 gap-0">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b border-border px-4">
        <h2 className="text-sm font-semibold text-foreground">{title}</h2>
        {entries.length > 1 && (
          <Select
            items={entries.map((e) => ({ value: e.value, label: e.label }))}
            value={active}
            onValueChange={change}
          >
            <SelectTrigger className="w-48" aria-label="查询类型">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              {entries.map((e) => (
                <SelectItem key={e.value} value={e.value}>
                  {e.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        )}
      </div>
      <div className="flex min-h-0 flex-1 flex-col [&>[role=tabpanel]]:flex-1">{panels}</div>
    </Tabs>
  )
}
