import React, { useEffect, useRef, useState } from "react"
import { getCurrentWindow } from "@tauri-apps/api/window"
import {
  MagnifyingGlass,
  User,
  Moon,
  Sun,
  Translate,
  ListChecks,
  Buildings,
  Power
} from "@phosphor-icons/react"
import { CloseIcon, MaximizeIcon, MinimizeIcon, RestoreIcon } from "./icons/icons"
import { cn } from "../lib/utils"
import LeSwitcher from "./LeSwitcher"
import { commands, type CurrentUser } from "../bindings"
import { useAppStore } from "../store/useAppStore"

interface TitleBarProps {
  isDark: boolean
  toggleTheme: () => void
  /** 跳转到某个路径。标题栏渲染在 RouterProvider 之外，拿不到路由上下文，所以由外层把 router.navigate 传进来 */
  onNavigate: (to: string) => void
}

export default function CustomTitleBar({ isDark, toggleTheme, onNavigate }: TitleBarProps) {
  const [isMaximized, setIsMaximized] = useState(false)
  const [isFocused, setIsFocused] = useState(true)
  const [searchActive, setSearchActive] = useState(false)
  const searchInputRef = useRef<HTMLInputElement>(null)
  const appWindow = getCurrentWindow()
  // 用户菜单：姓名 / 用户名取自后端；读取失败时不显示占位的假名字
  const [user, setUser] = useState<CurrentUser | null>(null)
  const openSettingsTab = useAppStore((s) => s.openSettingsTab)
  const leOptions = useAppStore((s) => s.leOptions)
  const requestLeSwitch = useAppStore((s) => s.requestLeSwitch)
  useEffect(() => {
    void commands.currentUserInfo().then((r) => {
      if (r.status === "ok") setUser(r.data)
    })
  }, [])

  useEffect(() => {
    appWindow.isMaximized().then(setIsMaximized)
    const unlisten = appWindow.onResized(async () => {
      setIsMaximized(await appWindow.isMaximized())
    })
    const unlistenFocus = appWindow.onFocusChanged(({ payload: focused }) => {
      setIsFocused(focused)
    })
    return () => {
      unlisten.then((f) => f())
      unlistenFocus.then((f) => f())
    }
  }, [])

  const hdlDoubleClic = async (e: React.MouseEvent) => {
    const target = e.target as HTMLElement
    if (target.closest("button") || target.closest(".search-box")) {
      return
    }
    await appWindow.toggleMaximize()
  }

  const hdlTitleBarMouseDown = (e: React.MouseEvent) => {
    const target = e.target as HTMLElement
    if (!target.closest(".search-box")) {
      searchInputRef.current?.blur()
      setSearchActive(false)
    }
  }

  // 手动实现拖拽：只有在检测到真实位移后才调用 startDragging，
  // 避免 data-tauri-drag-region 在单纯点击（mousedown 后立即 mouseup）时
  // 也会触发一次原生拖拽/释放捕获（ReleaseCapture + WM_NCLBUTTONDOWN），
  // 该原生调用会在 Win11 上造成鼠标箭头光标出现一帧闪烁。
  const hdlTitleBarDragStart = (e: React.MouseEvent) => {
    if (e.button !== 0) return
    const target = e.target as HTMLElement
    if (target.closest("button") || target.closest(".search-box")) {
      return
    }
    const { clientX, clientY } = e
    let dragging = false
    const handleMove = (moveEvent: MouseEvent) => {
      const moved = Math.abs(moveEvent.clientX - clientX) > 3 || Math.abs(moveEvent.clientY - clientY) > 3
      if (moved && !dragging) {
        dragging = true
        window.removeEventListener("mousemove", handleMove)
        window.removeEventListener("mouseup", handleUp)
        appWindow.startDragging()
      }
    }
    const handleUp = () => {
      window.removeEventListener("mousemove", handleMove)
      window.removeEventListener("mouseup", handleUp)
    }
    window.addEventListener("mousemove", handleMove)
    window.addEventListener("mouseup", handleUp)
  }

  const controlBtnClass = cn(
    "w-11 h-full flex items-center justify-center transition-all duration-150",
    !isFocused && "opacity-50"
  )

  return (
    <div
      onDoubleClick={hdlDoubleClic}
      onMouseDownCapture={hdlTitleBarMouseDown}
      onMouseDown={hdlTitleBarDragStart}
      className={cn(
        "w-full flex items-center justify-between select-none z-50 shrink-0 relative transition-all",
        isMaximized ? "h-10" : "h-11"
      )}
      style={{
        background: 'var(--titlebar-bg)',
        backdropFilter: 'var(--glass-blur)',
        WebkitBackdropFilter: 'var(--glass-blur)',
        borderBottom: '1px solid var(--titlebar-border)',
      }}
    >
      {/* 顶部高光线 */}
      <div
        className="absolute top-0 left-0 right-0 h-px pointer-events-none"
        style={{
          background: 'linear-gradient(90deg, transparent 0%, rgba(255,255,255,0.35) 30%, rgba(255,255,255,0.55) 50%, rgba(255,255,255,0.35) 70%, transparent 100%)',
        }}
      />

      {/* 左：Logo & LE 选择 */}
	  <div className="flex items-center gap-3 h-full" data-tauri-drag-region>
      <div className="pointer-events-none flex h-full items-center gap-2 pl-5">
		  <svg
			width="36"
			height="17"
			viewBox="0 0 36 17"
			fill="#7030a0"
			xmlns="http://www.w3.org/2000/svg"
			className="block h-3.5 w-auto shrink-0"
			aria-hidden="true"
		  >
			<path
			  fillRule="evenodd"
			  clipRule="evenodd"
			  d="M33.4657 0H30.78C30.6462 0.00119946 30.5246 0.0533761 30.4333 0.137339L25.0522 5.11991C23.2088 6.82795 20.728 7.78692 18.0003 7.78692C15.1571 7.78692 12.5845 6.73979 10.7204 4.90341C10.7204 4.90341 6.76235 1.23785 6.7283 1.20486C5.96868 0.507973 4.92322 0.0263882 3.71963 0H0.506614C0.226851 0 0 0.2237 0 0.499577V14.4997C0 15.8815 1.13426 17 2.53489 17H3.51589H4.17881C4.46039 17 4.68846 16.7745 4.68846 16.4974L4.68968 11.5155C4.68968 10.2134 5.76737 9.13931 7.06644 9.13931C9.48457 9.13931 11.2185 11.182 12.7852 12.5464C14.704 14.2178 16.1818 15.8281 18.0003 15.8281C19.8169 15.8281 21.2954 14.2178 23.2142 12.5464C24.7815 11.182 26.5154 9.13931 28.9323 9.13931C30.2247 9.13931 31.2969 10.2026 31.3097 11.4945V14.4997C31.3097 15.8815 32.4446 16.9976 33.8458 16.9976H34.9399C35.2258 16.9976 35.491 17 35.491 17C35.7719 17 36 16.7745 36 16.498C36 16.4908 36 2.61723 36 2.50028C36 1.1191 34.8645 0 33.4657 0Z"
			/>
		  </svg>

		  <span
			className="whitespace-nowrap text-[13px] font-bold leading-4 tracking-wide"
			style={{ color: "var(--text-primary)" }}
		  >
			进项发票管理系统
		  </span>
	  </div>

        <LeSwitcher />
      </div>

      {/* 中：快捷搜索 */}
      <div className="search-box flex-1 max-w-xs mx-6">
        <div className="relative flex items-center">
          <MagnifyingGlass className="w-3.5 h-3.5 absolute left-2.5 top-1/2 -translate-y-1/2 z-10 pointer-events-none" style={{ color: 'var(--text-faint)' }} />
          <input
            ref={searchInputRef}
            type="text"
            placeholder="快捷搜索功能或单据 (Ctrl+K)"
            className="w-full rounded-lg text-[12px] pl-8 pr-3 py-1 outline-none transition-all"
            style={{
              background: 'var(--input-bg)',
              backdropFilter: 'blur(8px)',
              WebkitBackdropFilter: 'blur(8px)',
              border: '1px solid var(--input-border)',
              color: 'var(--input-text)',
              cursor: searchActive ? 'text' : 'default',
            }}
            onMouseDown={(e) => {
              if (!searchActive) {
                // Not yet active: suppress the browser's default mousedown-focus so activation
                // only happens on mouseup, once we know it wasn't a drag.
                e.preventDefault()
                const { clientX, clientY } = e
                let dragging = false
                const handleMove = (moveEvent: MouseEvent) => {
                  const moved = Math.abs(moveEvent.clientX - clientX) > 3 || Math.abs(moveEvent.clientY - clientY) > 3
                  if (moved && !dragging) {
                    dragging = true
                    window.removeEventListener('mousemove', handleMove)
                    window.removeEventListener('mouseup', handleUp)
                   appWindow.startDragging()
                  }
                }
                const handleUp = () => {
                  window.removeEventListener('mousemove', handleMove)
                  window.removeEventListener('mouseup', handleUp)
                  if (!dragging) {
                    setSearchActive(true)
                    requestAnimationFrame(() => searchInputRef.current?.focus())
                  }
                }
                window.addEventListener('mousemove', handleMove)
                window.addEventListener('mouseup', handleUp)
              }
            }}
            onFocus={(e) => {
              setSearchActive(true)
              e.currentTarget.style.borderColor = 'var(--input-focus-ring)'
              e.currentTarget.style.boxShadow = '0 0 0 3px color-mix(in oklch, var(--input-focus-ring) 20%, transparent)'
            }}
            onBlur={(e) => {
              setSearchActive(false)
              e.currentTarget.style.borderColor = 'var(--input-border)'
              e.currentTarget.style.boxShadow = 'none'
            }}
          />
        </div>
      </div>

      {/* 右：功能 & 窗口控制 */}
      <div className="flex items-center h-full">
      <div className="flex items-center h-full gap-0.5 pr-2">
        {/* 主题切换 */}
        <button
          onClick={toggleTheme}
          className="w-8 h-8 flex items-center justify-center rounded-lg transition-all duration-200"
          style={{ color: 'var(--text-muted)' }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = 'var(--blue-pale)'
            e.currentTarget.style.color = 'var(--blue-light)'
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'transparent'
            e.currentTarget.style.color = 'var(--text-muted)'
          }}
          title="切换主题"
        >
          {isDark
            ? <Sun className="w-4 h-4" weight="duotone" />
            : <Moon className="w-4 h-4" weight="duotone" />
          }
        </button>

        {/* 用户 */}
        <div className="relative group">
          <button
            className="flex items-center justify-center ml-1 mr-2 w-8 h-8 rounded-lg transition-all duration-200"
            style={{ color: 'var(--text-muted)' }}
            onMouseEnter={(e) => {
              e.currentTarget.style.background = 'var(--blue-pale)'
              e.currentTarget.style.color = 'var(--blue-light)'
            }}
            onMouseLeave={(e) => {
              e.currentTarget.style.background = 'transparent'
              e.currentTarget.style.color = 'var(--text-muted)'
            }}
          >
            <User weight="duotone" className="w-4 h-4" />
          </button>
          <div
            className="absolute top-full mt-1.5 right-2 w-48 rounded-xl opacity-0 invisible group-hover:opacity-100 group-hover:visible transition-all z-50 flex flex-col py-1.5 overflow-hidden"
            style={{
              background: 'var(--glass-bg-strong)',
              backdropFilter: 'var(--glass-blur-strong)',
              WebkitBackdropFilter: 'var(--glass-blur-strong)',
              border: '1px solid var(--glass-border)',
              boxShadow: 'var(--glass-shadow-lg)',
            }}
          >
            <div
              className="px-3 py-2.5 border-b"
              style={{ borderColor: 'var(--glass-border-subtle)' }}
            >
              <div className="text-[12px] font-bold" style={{ color: 'var(--text-primary)' }}>{user?.fullName ?? '—'}</div>
              <div className="text-[10px] mt-0.5" style={{ color: 'var(--text-muted)' }}>{user?.username ?? ''}</div>
            </div>
            <div className="flex flex-col py-1">
              <button
                className="flex items-center justify-between px-3 py-1.5 text-[11px] transition-all w-full"
                style={{ color: 'var(--text-secondary)' }}
                onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--blue-pale)')}
                onMouseLeave={(e) => (e.currentTarget.style.background = 'transparent')}
              >
                <span className="flex items-center gap-2">
                  <Translate className="w-3.5 h-3.5" weight="duotone" /> 语言设置
                </span>
                <span style={{ color: 'var(--text-faint)' }}>中文</span>
              </button>
              {/* 只要拿到了 LE 列表就显示，哪怕只有一个：当前那一项灰着并标「当前」，用户至少知道有这个功能 */}
              {leOptions && leOptions.les.length > 0 && (
                <div className="py-1">
                  <div className="flex items-center gap-2 px-3 py-1 text-[10px]" style={{ color: 'var(--text-faint)' }}>
                    <Buildings className="w-3.5 h-3.5" weight="duotone" /> 切换 LE
                  </div>
                  {leOptions.les.map((le) => (
                    <button
                      key={le}
                      disabled={le === leOptions.current}
                      className="flex items-center justify-between px-3 py-1.5 pl-8 text-[11px] transition-all w-full disabled:opacity-60"
                      style={{ color: 'var(--text-secondary)' }}
                      onMouseEnter={(e) => { if (!e.currentTarget.disabled) e.currentTarget.style.background = 'var(--blue-pale)' }}
                      onMouseLeave={(e) => (e.currentTarget.style.background = 'transparent')}
                      onClick={() => requestLeSwitch(le)}
                    >
                      <span>LE {le}</span>
                      {le === leOptions.current && <span style={{ color: 'var(--text-faint)' }}>当前</span>}
                    </button>
                  ))}
                </div>
              )}
              <button
                className="flex items-center px-3 py-1.5 text-[11px] transition-all w-full gap-2"
                style={{ color: 'var(--text-secondary)' }}
                onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--blue-pale)')}
                onMouseLeave={(e) => (e.currentTarget.style.background = 'transparent')}
                onClick={() => {
                  // 打开「系统管理」并定位到「规则管理」标签；路由与侧边栏高亮一起切过去
                  openSettingsTab('rules')
                  onNavigate('/settings')
                }}
              >
                <ListChecks className="w-3.5 h-3.5" weight="duotone" /> 规则管理
              </button>
            </div>
            <div className="border-t pt-1" style={{ borderColor: 'var(--glass-border-subtle)' }}>
              <button
                className="flex items-center gap-2 px-3 py-1.5 text-[11px] font-medium transition-all w-full"
                style={{ color: 'var(--accent-red)' }}
                onMouseEnter={(e) => (e.currentTarget.style.background = 'var(--accent-red-bg)')}
                onMouseLeave={(e) => (e.currentTarget.style.background = 'transparent')}
                onClick={() => void appWindow.close()}
              >
                <Power className="w-3.5 h-3.5" weight="duotone" /> 关闭软件
              </button>
            </div>
          </div>
        </div>
      </div>

      {/* 窗口控制 — 与标题栏等高，紧贴右侧边缘 */}
      <div className="flex items-stretch h-full shrink-0">
        <button
          className={controlBtnClass}
          style={{ color: 'var(--text-muted)' }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = 'var(--blue-pale)'
            e.currentTarget.style.color = 'var(--text-primary)'
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'transparent'
            e.currentTarget.style.color = 'var(--text-muted)'
          }}
          onClick={() => appWindow.minimize()}
        >
          <MinimizeIcon className="w-2.5 h-2.5" />
        </button>
        <button
          className={controlBtnClass}
          style={{ color: 'var(--text-muted)' }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = 'var(--blue-pale)'
            e.currentTarget.style.color = 'var(--text-primary)'
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'transparent'
            e.currentTarget.style.color = 'var(--text-muted)'
          }}
          onClick={() => appWindow.toggleMaximize()}
        >
          {isMaximized ? <RestoreIcon className="w-2.5 h-2.5" /> : <MaximizeIcon className="w-2.5 h-2.5" />}
        </button>
        <button
          className={controlBtnClass}
          style={{ color: 'var(--text-muted)' }}
          onMouseEnter={(e) => {
            e.currentTarget.style.background = '#e81123'
            e.currentTarget.style.color = '#fff'
          }}
          onMouseLeave={(e) => {
            e.currentTarget.style.background = 'transparent'
            e.currentTarget.style.color = 'var(--text-muted)'
          }}
          onClick={() => appWindow.close()}
        >
          <CloseIcon className="w-2.5 h-2.5" />
        </button>
      </div>
      </div>
    </div>
  )
}
