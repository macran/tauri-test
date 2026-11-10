import React, { useEffect, useState } from 'react'
import { useLocation, useNavigate } from '@tanstack/react-router'
import DashboardContent from './components/DashboardContent'
import { TaxImportHub, VoucherImportHub } from './features/import-hub/ImportHubs'
import { CertifiedQuery, IndustryQuery, InvoiceQuery, OverviewQuery, VoucherQuery } from './features/query/QueryGroups'
import { InputTaxTransferManage, ReversalManage, StampTaxManage } from './features/tax-period/TaxManagePages'

import SystemSettings from './features/admin/SystemSettings'
import {
  ContextMenu,
  ContextMenuContent,
  ContextMenuItem,
  ContextMenuSeparator,
  ContextMenuTrigger,
} from './components/ui/context-menu'
import {
  SquaresFour as LayoutDashboard,
  Receipt as TaxData,
  ClipboardText as VoucherData,
  ArrowUUpLeft as Reversal,
  ArrowsLeftRight as TaxTransfer,
  Stamp,
  MagnifyingGlass as Filter,
  Table as QueryTable,
  Factory,
  SealCheck,
  Files,
  Gear as Settings,
  X,
} from '@phosphor-icons/react'
import { useAppStore } from './store/useAppStore'
import { Tooltip, TooltipContent, TooltipTrigger } from '@/components/ui/tooltip'

// 侧边栏在 xl 断点以下只显示图标（见 renderMenuItem 里的 `hidden xl:block`），
// 此时才需要用 tooltip 补充文字说明；xl 及以上文字本身可见，不需要重复提示。
function useIsIconOnlySidebar() {
  const [isIconOnly, setIsIconOnly] = useState(
    () => typeof window !== 'undefined' && !window.matchMedia('(min-width: 1280px)').matches
  )

  useEffect(() => {
    const mql = window.matchMedia('(min-width: 1280px)')
    const handler = (e: MediaQueryListEvent) => setIsIconOnly(!e.matches)
    mql.addEventListener('change', handler)
    return () => mql.removeEventListener('change', handler)
  }, [])

  return isIconOnly
}

const menuModules = [
  {
    title: '首页',
    items: [{ icon: LayoutDashboard, label: '进项发票概览', id: 'dashboard', path: '/dashboard' }],
  },
  {
    title: '数据导入',
    items: [
      { icon: TaxData, label: '税务数据', id: 'tax_data', path: '/tax_data' },
      { icon: VoucherData, label: '凭证数据', id: 'upload_business', path: '/upload_business' },
    ],
  },
  {
    title: '数据查询',
    items: [
      { icon: QueryTable, label: '发票数据', id: 'query_invoice', path: '/query_invoice' },
      { icon: Factory, label: '特定行业', id: 'query_industry', path: '/query_industry' },
      { icon: SealCheck, label: '认证清单', id: 'query_certified', path: '/query_certified' },
      { icon: Files, label: '凭证查询', id: 'query_voucher', path: '/query_voucher' },
    ],
  },
  {
    title: '进项管理',
    items: [
      { icon: Reversal, label: '红冲管理', id: 'billing', path: '/billing' },
      { icon: TaxTransfer, label: '进项转出管理', id: 'upload_manual', path: '/upload_manual' },
      { icon: Stamp, label: '印花税管理', id: 'stamp_tax', path: '/stamp_tax' },
    ],
  },
  {
    title: '综合查询',
    items: [{ icon: Filter, label: '综合查询', id: 'query', path: '/query' }],
  },
]

// 子路由（如 /tax_data/invoice_import）要归属到同一个顶层菜单项/Tab 上，
// 所以只取路径的第一段，而不是整段 pathname。
function topLevelSegment(pathname: string): string {
  return pathname.split('/').filter(Boolean)[0] ?? 'dashboard'
}

const settingsItem = { icon: Settings, label: '系统管理', id: 'settings', path: '/settings' }

const allMenuItems = [...menuModules.flatMap((m) => m.items), settingsItem]

const TAB_COMPONENTS: Record<string, React.ComponentType> = {
  dashboard: DashboardContent,
  query: OverviewQuery,
  query_invoice: InvoiceQuery,
  query_industry: IndustryQuery,
  query_certified: CertifiedQuery,
  query_voucher: VoucherQuery,
  tax_data: TaxImportHub,
  upload_business: VoucherImportHub,
  upload_manual: InputTaxTransferManage,
  billing: ReversalManage,
  stamp_tax: StampTaxManage,
  settings: SystemSettings,
}

export default function AppShell() {
  const navigate = useNavigate()
  // useRouter().state 只是读一次 context 里的 router 实例，拿到的 .state 是否新鲜
  // 完全取决于组件“恰好”因为别的原因重渲染——点卡片 navigate() 后如果没有其他
  // state 变化，这里会读到过期的 pathname，页面看起来“点了没反应”。
  // useLocation() 是基于 store 的订阅式 hook，pathname 变化会正确触发重渲染。
  const location = useLocation()

  const { openTabs, activeTabId, openTab, closeTab, setActiveTab, leEpoch } = useAppStore()

  // 切换页面时关掉所有打开的弹层（下拉、日期面板等）。
  // 页面用 Activity 保持挂载，弹层又是 portal 到 body 的，隐藏页面并不会带走它们；
  // Base UI 的弹层统一在 document 上监听 Escape 关闭，所以派发一次即可。
  useEffect(() => {
    document.dispatchEvent(new KeyboardEvent('keydown', { key: 'Escape', bubbles: true }))
  }, [activeTabId])

  useEffect(() => {
    if (location.pathname === '/') {
      navigate({ to: '/dashboard', replace: true })
    }
  }, [location.pathname, navigate])

  useEffect(() => {
    const id = topLevelSegment(location.pathname)
    const item = allMenuItems.find((m) => m.id === id)
    if (!item) return
    // 刷新后标签列表被重置（只持久化了主题），但 URL 还在、侧边栏据此高亮：
    // 路径对应的页面没有标签时补开一个，内容区才和侧边栏一致
    if (openTabs.find((t) => t.id === id)) setActiveTab(id)
    else openTab({ id: item.id, label: item.label })
  }, [location.pathname])

  const handleMenuClick = (item: (typeof allMenuItems)[0]) => {
    openTab({ id: item.id, label: item.label })
    navigate({ to: item.path })
  }

  const handleTabClick = (tab: { id: string; label: string }) => {
    const item = allMenuItems.find((m) => m.id === tab.id)
    if (item) {
      setActiveTab(tab.id)
      navigate({ to: item.path })
    }
  }

  const handleCloseTab = (e: React.MouseEvent | undefined, tabId: string) => {
    e?.stopPropagation()
    const { openTabs: tabs } = useAppStore.getState()
    const remaining = tabs.filter((t) => t.id !== tabId)
    closeTab(tabId)
    if (activeTabId === tabId && remaining.length > 0) {
      const next = remaining[remaining.length - 1]
      const item = allMenuItems.find((m) => m.id === next.id)
      if (item) navigate({ to: item.path })
    } else if (remaining.length === 0) {
      navigate({ to: '/dashboard' })
    }
  }

  /** 右键菜单：keepIds 之外的标签全部关闭；激活的标签被关掉时，改为激活留下的最后一个（没有就回到空状态） */
  const closeTabsExcept = (keepIds: string[]) => {
    const { openTabs: tabs, activeTabId: active } = useAppStore.getState()
    const kept = tabs.filter((t) => keepIds.includes(t.id))
    tabs.filter((t) => !keepIds.includes(t.id)).forEach((t) => closeTab(t.id))
    if (kept.some((t) => t.id === active)) return
    const next = kept[kept.length - 1]
    const item = next && allMenuItems.find((m) => m.id === next.id)
    if (item) {
      setActiveTab(item.id)
      navigate({ to: item.path })
    }
  }

  const currentPath = topLevelSegment(location.pathname)
  const isIconOnlySidebar = useIsIconOnlySidebar()

  const renderMenuItem = (
    item: (typeof allMenuItems)[0],
    isActive: boolean,
    onClick: (item: (typeof allMenuItems)[0]) => void
  ) => {
    const trigger = (
      <div
        onClick={() => onClick(item)}
        className="relative flex items-center gap-3 px-3 h-10 rounded-xl cursor-pointer transition-all duration-200"
        style={
          isActive
            ? {
                background: 'var(--sidebar-active-bg)',
                border: '1px solid var(--sidebar-active-border)',
                color: 'var(--sidebar-active-text)',
                boxShadow: 'var(--sidebar-active-glow)',
              }
            : {
                background: 'transparent',
                border: '1px solid transparent',
                color: 'var(--text-secondary)',
              }
        }
        onMouseEnter={(e) => {
          if (!isActive) {
            const el = e.currentTarget as HTMLElement
            el.style.background = 'var(--blue-pale)'
            el.style.color = 'var(--text-primary)'
            el.style.borderColor = 'var(--glass-border-subtle)'
          }
        }}
        onMouseLeave={(e) => {
          if (!isActive) {
            const el = e.currentTarget as HTMLElement
            el.style.background = 'transparent'
            el.style.color = 'var(--text-secondary)'
            el.style.borderColor = 'transparent'
          }
        }}
      >
        <item.icon
          weight={isActive ? 'duotone' : 'regular'}
          className="w-[18px] h-[18px] shrink-0"
          style={{ color: isActive ? 'var(--sidebar-active-text)' : 'var(--text-muted)' }}
        />
        <span className={`text-sm hidden xl:block truncate ${isActive ? 'font-semibold' : 'font-medium'}`} style={{ color: 'inherit' }}>
          {item.label}
        </span>
      </div>
    )

    if (!isIconOnlySidebar) {
      return trigger
    }

    return (
      <Tooltip>
        <TooltipTrigger render={trigger} />
        <TooltipContent
          side="right"
          sideOffset={8}
          className="bg-black text-white dark:bg-white dark:text-black"
        >
          {item.label}
        </TooltipContent>
      </Tooltip>
    )
  }

  return (
    <div
      className="flex flex-col h-full w-full font-sans relative overflow-hidden"
      style={{ backgroundColor: 'var(--app-bg)' }}
    >
      {/* 多层光晕背景 */}
      <div className="absolute inset-0 pointer-events-none" style={{ background: 'var(--orb-1)' }} />
      <div className="absolute inset-0 pointer-events-none" style={{ background: 'var(--orb-2)' }} />
      <div className="absolute inset-0 pointer-events-none" style={{ background: 'var(--orb-3)' }} />

      {/* 细粒噪点质感层 */}
      <div
        className="absolute inset-0 pointer-events-none opacity-[0.025]"
        style={{
          backgroundImage: `url("data:image/svg+xml,%3Csvg viewBox='0 0 256 256' xmlns='http://www.w3.org/2000/svg'%3E%3Cfilter id='n'%3E%3CfeTurbulence type='fractalNoise' baseFrequency='0.9' numOctaves='4' stitchTiles='stitch'/%3E%3C/filter%3E%3Crect width='100%25' height='100%25' filter='url(%23n)'/%3E%3C/svg%3E")`,
          backgroundSize: '200px 200px',
        }}
      />

      <div className="flex flex-1 overflow-hidden relative z-10 p-3 gap-3">
        {/* ── 侧边栏 card ── */}
        <aside
          className="w-16 xl:w-56 flex-shrink-0 flex flex-col z-20 transition-all duration-300 relative rounded-2xl overflow-hidden select-none"
          style={{
            background: 'var(--card-bg)',
            backdropFilter: 'var(--glass-blur)',
            WebkitBackdropFilter: 'var(--glass-blur)',
            border: '1px solid var(--card-border)',
            boxShadow: 'var(--card-shadow)',
          }}
        >
          <nav className="flex-1 flex flex-col gap-0.5 px-2 pt-3 overflow-y-auto hide-scrollbar relative z-10">
            {menuModules.map((module, mIdx) => (
              <div key={mIdx} className="flex flex-col gap-0.5">
                <div
                  className="px-3 text-xs font-semibold tracking-wide mt-5 mb-1.5 hidden xl:block"
                  style={{ color: 'var(--text-muted)' }}
                >
                  {module.title}
                </div>
                {module.items.map((item, idx) => (
                  <React.Fragment key={idx}>
                    {renderMenuItem(item, currentPath === item.id, handleMenuClick)}
                  </React.Fragment>
                ))}
              </div>
            ))}
          </nav>

          {/* 偏好与设置 — 固定于底部，与下边距保持间隔 */}
          <div className="px-2 pt-2 pb-4 shrink-0 relative z-10">
            {renderMenuItem(settingsItem, currentPath === settingsItem.id, handleMenuClick)}
          </div>
        </aside>

        {/* ── 主内容区 ── */}
        <main className="flex-1 relative flex flex-col overflow-hidden z-10 rounded-2xl" style={{ background: 'var(--card-bg)', backdropFilter: 'var(--glass-blur)', WebkitBackdropFilter: 'var(--glass-blur)', border: '1px solid var(--card-border)', boxShadow: 'var(--card-shadow)' }}>
          {/* Tab 标签栏 */}
          <div
            className="flex px-3 pt-2 gap-1 overflow-x-auto shrink-0 z-20 hide-scrollbar select-none"
            style={{
              background: 'var(--tabbar-bg)',
              backdropFilter: 'blur(16px)',
              WebkitBackdropFilter: 'blur(16px)',
              borderBottom: '1px solid var(--tabbar-border)',
            }}
          >
            {openTabs.map((tab) => {
              const isActive = activeTabId === tab.id
              return (
                <ContextMenu key={tab.id}>
                <ContextMenuTrigger
                  onClick={() => handleTabClick(tab)}
                  className="group relative flex items-center gap-1.5 pl-3 pr-6 py-1.5 rounded-t-xl cursor-pointer transition-all duration-200 min-w-[100px] max-w-[160px]"
                  style={
                    isActive
                      ? {
                          background: 'var(--tab-active-bg)',
                          backdropFilter: 'blur(12px)',
                          WebkitBackdropFilter: 'blur(12px)',
                          // 不用 border 简写：和 borderBottomColor 混用时，激活 / 非激活切换会让 React 警告并可能样式错乱
                          borderWidth: '1px',
                          borderStyle: 'solid',
                          borderColor: 'var(--glass-border)',
                          borderBottomColor: 'transparent',
                          color: 'var(--tab-active-text)',
                          boxShadow: '0 -2px 8px rgba(100,130,255,0.08)',
                        }
                      : {
                          background: 'transparent',
                          borderWidth: '1px',
                          borderStyle: 'solid',
                          borderColor: 'transparent',
                          borderBottomColor: 'transparent',
                          color: 'var(--tab-inactive-text)',
                        }
                  }
                >
                  {isActive && (
                    <span
                      className="w-1.5 h-1.5 rounded-full shrink-0"
                      style={{ backgroundColor: '#22C55e', boxShadow: '0 0 4px var(--blue-primary)' }}
                    />
                  )}
                  {/* 外层 flex-1 只用来量出 tab 内剩余宽度；内层 w-fit 才是文字宽度。
                      下划线（参考 Teams）：激活 = 绿色、与文字等长；未激活悬停 = 灰色预告线。
                      悬停激活 tab 时，从文字两端向外"长出"两段绿线直到 tab 边缘，用缩放做过渡。
                      关闭按钮是绝对定位，不占文字宽度。 */}
                  <span className="relative min-w-0 flex-1">
                    <span className="relative block w-fit max-w-full">
                      <span className="block truncate text-xs font-medium">{tab.label}</span>
                      <span
                        aria-hidden="true"
                        className={`pointer-events-none absolute inset-x-0 -bottom-1.5 rounded-full transition-opacity duration-200 ${
                          isActive
                            ? 'h-[3px] bg-[#22C55e]'
                            : 'h-[2px] bg-[var(--tab-inactive-text)] opacity-0 group-hover:opacity-35'
                        }`}
                      />
                    </span>
                    {isActive && (
                      <>
                        {/* 向右：从文字起点铺到 tab 右边缘（抵消 pr-6），起点被文字下的主线盖住，只看到超出文字的部分在生长 */}
                        <span
                          aria-hidden="true"
                          className="pointer-events-none absolute -bottom-1.5 left-0 -right-6 h-[3px] origin-left scale-x-0 rounded-full bg-[#22C55e] transition-transform duration-300 ease-out group-hover:scale-x-100"
                        />
                        {/* 向左：从文字起点向左铺到 tab 左边缘（pl-3 + 圆点 6 + gap 6 = 24px），以文字起点为原点生长 */}
                        <span
                          aria-hidden="true"
                          className="pointer-events-none absolute -bottom-1.5 -left-6 h-[3px] w-6 origin-right scale-x-0 rounded-full bg-[#22C55e] transition-transform duration-300 ease-out group-hover:scale-x-100"
                        />
                      </>
                    )}
                  </span>
                  <button
                    onClick={(e) => handleCloseTab(e, tab.id)}
                    className="absolute right-[3px] top-1/2 -translate-y-1/2 w-[16px] h-[16px] flex items-center justify-center rounded-sm transition-colors shrink-0 opacity-0 group-hover:opacity-100 text-[var(--tab-inactive-text)] hover:bg-black/10 hover:text-[var(--tab-active-text)] dark:hover:bg-white/15"
                  >
                    <X className="w-[10px] h-[10px]" weight="bold" />
                  </button>
                </ContextMenuTrigger>
                <ContextMenuContent>
                  <ContextMenuItem onClick={() => handleCloseTab(undefined, tab.id)}>关闭当前标签</ContextMenuItem>
                  <ContextMenuItem disabled={openTabs.length < 2} onClick={() => closeTabsExcept([tab.id])}>
                    关闭其他所有标签
                  </ContextMenuItem>
                  <ContextMenuSeparator />
                  <ContextMenuItem onClick={() => closeTabsExcept([])}>关闭所有标签</ContextMenuItem>
                </ContextMenuContent>
                </ContextMenu>
              )
            })}
          </div>

          {/* 渲染区 — keepalive */}
          <div className="flex-1 flex flex-col min-h-0 relative z-10 p-4 overflow-hidden">
            {openTabs.length === 0 ? (
              <div
                className="flex-1 flex flex-col items-center justify-center gap-4 select-none"
                style={{ color: 'var(--text-muted)' }}
              >
                <div
                  className="w-16 h-16 rounded-2xl flex items-center justify-center"
                  style={{
                    background: 'var(--glass-bg)',
                    border: '1px solid var(--glass-border)',
                    backdropFilter: 'var(--glass-blur)',
                  }}
                >
                  <LayoutDashboard className="w-8 h-8 opacity-40" />
                </div>
                <p className="text-sm font-medium" style={{ color: 'var(--text-muted)' }}>请从左侧菜单选择功能模块</p>
              </div>
            ) : (
              openTabs.map((tab) => {
                const Comp = TAB_COMPONENTS[tab.id]
                if (!Comp) return null
                // 用 hidden 而不是 <Activity>：Activity 隐藏时会停掉子树的所有 effect，
                // Base UI 的下拉/日期面板靠 effect 监听 Escape 和完成关闭动画，
                // 弹层又是 portal 到 body 的——页面一隐藏，弹层就残留在新页面上。
                return (
                  <div
                    key={`${leEpoch}-${tab.id}`}
                    hidden={activeTabId !== tab.id}
                    className="flex-1 flex flex-col min-h-0 h-full [&[hidden]]:hidden"
                  >
                    <Comp />
                  </div>
                )
              })
            )}
          </div>
        </main>
      </div>
    </div>
  )
}
