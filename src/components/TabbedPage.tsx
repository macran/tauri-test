// 管理页外壳：标题栏（标题 + 动画标签组，下方分割线，与 SubTabs 一致）（样式与首页 DashboardContent 的标签组一致）。
// 访问过的标签保持挂载，切走再切回不丢表单和结果。
import { motion } from 'motion/react'
import { useEffect, useId, useState, type ComponentType } from 'react'

export interface TabEntry {
  value: string
  label: string
  Component: ComponentType
}

export default function TabbedPage({
  title,
  tabs,
  requestedTab,
  onRequestedTabHandled,
}: {
  title: string
  tabs: TabEntry[]
  /** 外部要求切到的标签（例如用户菜单里的「规则管理」）；切换后调用 onRequestedTabHandled 清掉请求 */
  requestedTab?: string | null
  onRequestedTabHandled?: () => void
}) {
  const [active, setActive] = useState(tabs[0].value)
  const [visited, setVisited] = useState(() => new Set([tabs[0].value]))
  // 同时打开多个管理页时，各自的滑块 layoutId 不能相同
  const indicatorId = useId()

  const change = (value: string) => {
    setActive(value)
    setVisited((prev) => (prev.has(value) ? prev : new Set(prev).add(value)))
  }

  useEffect(() => {
    if (!requestedTab) return
    if (tabs.some((t) => t.value === requestedTab)) change(requestedTab)
    onRequestedTabHandled?.()
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [requestedTab])

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col">
      <div className="flex h-12 shrink-0 items-center gap-3 border-b border-border px-4">
      <h2 className="text-sm font-semibold text-foreground">{title}</h2>

      <div role="tablist" className="flex w-fit shrink-0 gap-0.5 rounded-xl bg-slate-100 p-1 dark:bg-slate-900/60">
        {tabs.map((tab) => (
          <button
            key={tab.value}
            role="tab"
            type="button"
            aria-selected={active === tab.value}
            onClick={() => change(tab.value)}
            className="relative z-10 rounded-lg px-4 py-1 text-[13px] font-medium transition-colors"
            style={{ color: active === tab.value ? 'var(--tab-active-text)' : 'var(--text-muted)' }}
          >
            {active === tab.value && (
              <motion.div
                layoutId={indicatorId}
                className="absolute inset-0 -z-10 rounded-lg"
                style={{
                  background: 'var(--segment-active-bg)',
                  backdropFilter: 'blur(12px)',
                  WebkitBackdropFilter: 'blur(12px)',
                  border: '1px solid var(--glass-border)',
                  boxShadow: '0 2px 8px rgba(100,130,255,0.10)',
                }}
                transition={{ type: 'spring', bounce: 0.18, duration: 0.45 }}
              />
            )}
            {tab.label}
          </button>
        ))}
      </div>
      </div>

      <div className="min-h-0 min-w-0 flex-1 overflow-hidden">
        {tabs.map(({ value, Component }) => (
          <div key={value} role="tabpanel" hidden={active !== value} className="h-full min-h-0 min-w-0">
            {visited.has(value) && <Component />}
          </div>
        ))}
      </div>
    </div>
  )
}
