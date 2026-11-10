// src/AppRoot.tsx
//
// 窗口级别的外壳：只负责标题栏（拖拽区 + 最小化/最大化/关闭）。
// decorations:false 意味着 Windows 不提供任何原生窗口控件，所以这一层
// 必须在 starting / failed / ready 三个阶段都常驻——否则启动失败时
// 用户会困在一个连关闭按钮都没有的窗口里。
//
// 真正随阶段变化的内容（骨架屏 / 错误页 / 业务界面）交给 StartupGate 处理，
// 这里不关心。

import { useEffect } from "react"
import { invoke } from "@tauri-apps/api/core"
import { RouterProvider, type Router } from "@tanstack/react-router"

import CustomTitleBar from "./components/CustomTitleBar"
import { StartupGate } from "./features/startup"
import { StartupRuntimeToast } from "./features/startup/StartupRuntimeToast"
import { useAppStore } from "./store/useAppStore"

export function AppRoot({ router }: { router: Router<any, any> }) {
  const { isDark, toggleTheme } = useAppStore()

  useEffect(() => {
    document.documentElement.classList.toggle("dark", isDark)
  }, [isDark])

  useEffect(() => {
    // 窗口在 tauri.conf.json 里以 visible:false 创建，等这里通知 Rust 才会 show()。
    // 不能用 requestAnimationFrame：隐藏窗口里 WebView2 会暂停 rAF，回调永远不触发，
    // 主窗口就不会显示。React 首次提交已完成（effect 已执行），直接延后一个宏任务即可。
    const timer = window.setTimeout(() => {
      void invoke("notify_frontend_ready")
    }, 0)
    return () => window.clearTimeout(timer)
  }, [])

  return (
    <div
      className="flex h-screen w-full flex-col overflow-hidden"
      style={{ backgroundColor: "var(--app-bg)" }}
    >
      <CustomTitleBar isDark={isDark} toggleTheme={toggleTheme} onNavigate={(to) => void router.navigate({ to })} />

      <div className="min-h-0 flex-1 overflow-hidden">
        <StartupGate>
          <RouterProvider router={router} />
        </StartupGate>
      </div>

      {/* 全局挂一次即可：StartupGate 内部/AppShell 都不再重复挂载 */}
      <StartupRuntimeToast />
    </div>
  )
}
