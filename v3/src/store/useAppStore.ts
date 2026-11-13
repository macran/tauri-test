import { create } from 'zustand'
import { persist } from 'zustand/middleware'

export interface MenuItemDef {
  id: string
  label: string
}

interface AppState {
  isDark: boolean
  openTabs: MenuItemDef[]
  activeTabId: string
  /** 每切换一次 LE 加一。内容区的页面 key 里带上它，切换后所有页面重新挂载，不会残留上一个 LE 的数据 */
  leEpoch: number
  /** 想让「系统管理」打开的标签（如 'rules'）。一次性请求：页面读取并切换后用 clearSettingsTab 清掉 */
  settingsTab: string | null
  /** 用户菜单点了某个 LE：LeSwitcher 收到后弹出确认框（对话框和切换逻辑都在 LeSwitcher 里） */
  leSwitchRequest: string | null
  /** 可切换的 LE 列表与当前 LE，由 LeSwitcher 读取后级联给用户菜单显示 */
  leOptions: { current: string; les: string[] } | null

  /** 切换 LE 后调用：关闭所有标签页，回到首页，并让所有页面重新挂载 */
  resetForLeSwitch: () => void
  toggleTheme: () => void
  openTab: (item: MenuItemDef) => void
  closeTab: (id: string) => void
  setActiveTab: (id: string) => void
  /** 打开「系统管理」并定位到某个标签 */
  openSettingsTab: (tab: string) => void
  clearSettingsTab: () => void
  requestLeSwitch: (le: string) => void
  clearLeSwitchRequest: () => void
  setLeOptions: (o: { current: string; les: string[] } | null) => void
}

export const useAppStore = create<AppState>()(
  persist(
    (set, get) => ({
      isDark: false,
      openTabs: [{ id: 'dashboard', label: '进项发票概览' }],
      activeTabId: 'dashboard',
      leEpoch: 0,
      settingsTab: null,
      leSwitchRequest: null,
      leOptions: null,

      resetForLeSwitch: () =>
        set((s) => ({
          openTabs: [{ id: 'dashboard', label: '进项发票概览' }],
          activeTabId: 'dashboard',
          leEpoch: s.leEpoch + 1,
        })),

      toggleTheme: () => {
        const next = !get().isDark
        if (next) {
          document.documentElement.classList.add('dark')
        } else {
          document.documentElement.classList.remove('dark')
        }
        set({ isDark: next })
      },

      openTab: (item) => {
        const { openTabs } = get()
        if (!openTabs.find(t => t.id === item.id)) {
          set({ openTabs: [...openTabs, item] })
        }
        set({ activeTabId: item.id })
      },

      closeTab: (id) => {
        const { openTabs, activeTabId } = get()
        const next = openTabs.filter(t => t.id !== id)
        let nextActive = activeTabId
        if (activeTabId === id) {
          nextActive = next.length > 0 ? next[next.length - 1].id : ''
        }
        set({ openTabs: next, activeTabId: nextActive })
      },

      setActiveTab: (id) => set({ activeTabId: id }),

      openSettingsTab: (tab) => {
        const { openTabs } = get()
        if (!openTabs.find((t) => t.id === 'settings')) {
          set({ openTabs: [...openTabs, { id: 'settings', label: '系统管理' }] })
        }
        set({ activeTabId: 'settings', settingsTab: tab })
      },
      clearSettingsTab: () => set({ settingsTab: null }),
      requestLeSwitch: (le) => set({ leSwitchRequest: le }),
      clearLeSwitchRequest: () => set({ leSwitchRequest: null }),
      setLeOptions: (o) => set({ leOptions: o }),
    }),
    {
      name: 'app-ui',
      partialize: (s) => ({ isDark: s.isDark }),
      onRehydrateStorage: () => (state) => {
        if (state?.isDark) {
          document.documentElement.classList.add('dark')
        }
      },
    }
  )
)
