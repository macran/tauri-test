// 独立 splash 窗口：后端每个初始化阶段 emit startup-progress，这里用 shadcn Progress 展示；
// 成功后由后端关闭本窗口并显示主窗口。事件之外再轮询 get_startup_status，
// 避免监听晚于事件而丢进度，两者取较大的 percent。
import { useEffect, useMemo, useState } from "react"
import { invoke } from "@tauri-apps/api/core"
import { listen } from "@tauri-apps/api/event"
import { getCurrentWindow } from "@tauri-apps/api/window"
import { Check, Clipboard, Receipt, RefreshCw, TriangleAlert, Upload, X } from "lucide-react"

import { commands } from "@/bindings"
import { Button } from "@/components/ui/button"
import { Progress, ProgressLabel, ProgressValue } from "@/components/ui/progress"

import type { AppError, StartupStatus } from "./startup.types"

type ProgressPayload = { percent: number; message: string }

export default function Splash() {
  const [progress, setProgress] = useState<ProgressPayload>({ percent: 0, message: "正在启动…" })
  const [error, setError] = useState<AppError | null>(null)

  useEffect(() => {
    // 窗口 visible:false 创建，内容渲染好再显示，避免先看到空白窗口
    void getCurrentWindow().show()

    // percent 单调：晚到的旧值不会让进度条倒退
    // 进度轴由后端初始化（0..60）与 main 窗口加载（60..100）共用，100 = 主窗口可以显示
    const advance = (next: Partial<ProgressPayload>) =>
      setProgress((prev) =>
        (next.percent ?? 0) >= prev.percent
          ? { percent: next.percent ?? prev.percent, message: next.message || prev.message }
          : prev,
      )

    const unlisten = listen<ProgressPayload>("startup-progress", (e) => advance(e.payload))

    const poll = async () => {
      try {
        const status = await invoke<StartupStatus>("get_startup_status")
        if (status.phase === "failed") {
          setError(status.error)
          return
        }
        // 初始化完成后状态是 ready，但 main 窗口还在加载并上报进度，所以统一读最近一次进度
        const last = await invoke<ProgressPayload | null>("get_startup_progress")
        if (last) advance(last)
      } catch (e) {
        setError({ code: "internal", severity: "fatal", message: "无法读取启动状态。", detail: String(e) })
      }
    }
    void poll()
    const timer = window.setInterval(() => void poll(), 250)

    return () => {
      window.clearInterval(timer)
      void unlisten.then((fn) => fn())
    }
  }, [])

  // 失败后停止轮询已无意义，但 effect 清理不依赖 error；失败态下轮询只会重复 setError，开销可忽略

  return (
    <div className="relative flex h-screen w-screen overflow-hidden bg-background text-foreground">
      <div data-tauri-drag-region className="absolute inset-0" />
      <button
        type="button"
        aria-label="关闭"
        onClick={() => void getCurrentWindow().close()}
        className="absolute top-2 right-2 z-10 rounded-md p-1.5 text-muted-foreground hover:bg-muted"
      >
        <X className="size-4" />
      </button>

      <div className="pointer-events-none flex w-[168px] shrink-0 flex-col justify-between bg-primary p-6 text-primary-foreground">
        <Receipt className="size-10" aria-hidden="true" />
        <div>
          <div className="text-lg font-semibold tracking-wide">进项发票管理</div>
          <div className="mt-1 text-xs opacity-80">Input Tax</div>
        </div>
      </div>

      <div className="relative flex min-w-0 flex-1 flex-col p-8">
        {error ? (
          <SplashError error={error} />
        ) : (
          <div className="flex flex-1 flex-col justify-center" role="status" aria-live="polite">
            <h1 className="mb-5 text-xl font-semibold">正在启动</h1>
            <Progress value={progress.percent} className="gap-2">
              <ProgressLabel className="max-w-[18rem] truncate text-xs font-normal text-muted-foreground">
                {progress.message}
              </ProgressLabel>
              <ProgressValue>{() => `${Math.round(progress.percent)}%`}</ProgressValue>
            </Progress>
          </div>
        )}
      </div>
    </div>
  )
}

function SplashError({ error }: { error: AppError }) {
  const [copied, setCopied] = useState(false)
  const [busy, setBusy] = useState(false)
  const [uploadError, setUploadError] = useState<string | null>(null)
  // 上传成功后不自动重启：先让用户看到结果，再由用户点「重新登录」
  const [uploaded, setUploaded] = useState(false)
  const [relaunchError, setRelaunchError] = useState<string | null>(null)

  const diagnostic = useMemo(
    () =>
      [`code: ${error.code}`, `message: ${error.message}`, error.detail ? `detail: ${error.detail}` : null]
        .filter(Boolean)
        .join("\n"),
    [error],
  )

  async function copy() {
    try {
      await navigator.clipboard.writeText(diagnostic)
      setCopied(true)
      window.setTimeout(() => setCopied(false), 2_500)
    } catch {
      setCopied(false)
    }
  }

  /** 重启失败不能静默吞掉（void 会丢掉 Promise 拒绝），否则用户只看到「点了没反应」 */
  async function restart() {
    setRelaunchError(null)
    try {
      await commands.restartApp()
    } catch (e) {
      setRelaunchError(e instanceof Error ? e.message : String(e))
    }
  }

  async function upload() {
    setBusy(true)
    setUploadError(null)
    try {
      const result = await commands.uploadLocalToShared()
      if (result.status === "error") throw new Error(result.error.message)
      setUploaded(true)
    } catch (e) {
      setUploadError(e instanceof Error ? e.message : String(e))
    } finally {
      setBusy(false)
    }
  }

  return (
    <>
      <div className="flex items-center gap-2 text-destructive">
        <TriangleAlert className="size-5" aria-hidden="true" />
        <h1 className="text-base font-semibold">启动未完成</h1>
      </div>
      <div role="alert" className="thin-scrollbar mt-3 flex-1 overflow-auto whitespace-pre-wrap break-words pr-1 text-sm text-muted-foreground">
        {error.message}
        {error.detail ? <div className="mt-2 font-mono text-xs opacity-70">{error.detail}</div> : null}
        {uploadError ? <div className="mt-2 text-destructive">上传失败：{uploadError}</div> : null}
        {relaunchError ? <div className="mt-2 text-destructive">重新登录失败：{relaunchError}</div> : null}
        {uploaded ? (
          <div className="mt-2 flex items-center gap-1.5 text-foreground">
            <Check className="size-4 text-emerald-600" aria-hidden="true" />
            已上传到共享盘，请点「重新登录」继续。
          </div>
        ) : null}
      </div>
      <div className="mt-4 flex flex-wrap justify-end gap-2">
        {error.code === "local_newer" && !uploaded && (
          <Button size="sm" disabled={busy} onClick={() => void upload()}>
            <Upload aria-hidden="true" />
            {busy ? "正在上传…" : "立即上传到共享盘"}
          </Button>
        )}
        <Button size="sm" variant="outline" onClick={() => void copy()}>
          {copied ? <Check aria-hidden="true" /> : <Clipboard aria-hidden="true" />}
          {copied ? "已复制" : "复制诊断信息"}
        </Button>
        {/* 上传成功后「重新登录」是下一步，突出显示 */}
        <Button size="sm" variant={uploaded ? "default" : "outline"} disabled={busy} onClick={() => void restart()}>
          <RefreshCw aria-hidden="true" />
          重新登录
        </Button>
        <Button size="sm" variant="ghost" onClick={() => void getCurrentWindow().close()}>
          退出
        </Button>
      </div>
    </>
  )
}
