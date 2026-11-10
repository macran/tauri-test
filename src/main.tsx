// src/main.tsx
import { lazy, StrictMode, Suspense } from "react"
import { createRoot } from "react-dom/client"
import { invoke } from "@tauri-apps/api/core"
import { getCurrentWindow } from "@tauri-apps/api/window"

import Splash from "./features/startup/Splash"
import "./index.css"

const rootElement = document.getElementById("root")

if (!rootElement) {
  // 正常情况不会发生；真发生了说明 index.html 被改坏了。
  throw new Error("找不到 #root 挂载点，index.html 可能已损坏。")
}

// main 窗口把自己的加载里程碑报给 splash（进度轴 60..=99，100 由 notify_frontend_ready 隐含），
// 这样 splash 的 100% 就等于"主窗口可以显示"。splash 窗口自己不上报。
const isMain = getCurrentWindow().label === "main"
const reportProgress = (percent: number, message: string) => {
  if (isMain) void invoke("report_main_progress", { percent, message }).catch(() => {})
}
reportProgress(70, "正在加载界面脚本…")

// splash 窗口只加载自身；业务代码（router、所有页面）懒加载，只在 main 窗口才会下载/编译。
const MainApp = lazy(() =>
  import("./MainApp")
    .then((m) => {
      reportProgress(85, "正在初始化界面…")
      return m
    })
    .catch((e) => {
      // 业务模块加载失败（语法/求值错误）：把原因写进进度文案，启动页就不会静默停在 70% 了
      const msg = e instanceof Error ? e.message : String(e)
      reportProgress(79, `界面脚本加载失败：${msg}`.slice(0, 120))
      console.error("MainApp 加载失败", e)
      throw e
    }),
)


createRoot(rootElement).render(
  <StrictMode>
    {getCurrentWindow().label === "splash" ? (
      <Splash />
    ) : (
      <Suspense fallback={null}>
        <MainApp />
      </Suspense>
    )}
  </StrictMode>,
)
