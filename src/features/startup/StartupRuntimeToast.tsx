// src/features/startup/StartupRuntimeToast.tsx

import { useEffect, useRef } from "react"

import { toast } from "@/components/ui/toast"

import {
  usePendingBackups,
  useRetryBackup,
  useStartupStatus,
} from "./useStartupStatus"

/**
 * 不渲染任何节点，只负责把启动状态映射到 Toast。
 * 挂载点必须在 <Toaster /> 已经渲染之后（main.tsx 或 AppShell 内）。
 */
export function StartupRuntimeToast() {
  const startupQuery = useStartupStatus()

  const data = startupQuery.data
  const isReady = data?.phase === "ready"
  const runtime = data?.phase === "ready" ? data.runtime : undefined
  const warnings = data?.phase === "ready" ? data.warnings : []

  const pendingQuery = usePendingBackups(isReady)
  const retryBackup = useRetryBackup()

  const pendingFiles = pendingQuery.data ?? []

  const shareUnavailable = runtime?.share_available === false
  const hasPendingBackup =
    pendingFiles.length > 0 ||
    warnings.some((warning) => warning.code === "backup_pending")
  const hasSyncWarning = warnings.some(
    (warning) =>
      warning.code === "sync_failed" || warning.code === "backup_failed",
  )

  // Base UI 的 toast 是命令式 add/close，需要自己记住当前显示的 toast id，
  // 这样状态变化时可以先关掉旧的再开新的，而不是无限叠加。
  const toastIdRef = useRef<string | null>(null);

  useEffect(() => {
    if (!isReady || !runtime) {
      return
    }

    const shouldShow = shareUnavailable || hasPendingBackup || hasSyncWarning

    if (!shouldShow) {
      if (toastIdRef.current) {
        toast.close(toastIdRef.current)
        toastIdRef.current = null
      }
      return
    }

    const title = hasPendingBackup
      ? "本地修改尚未完成备份"
      : "当前使用本机数据"

    const description = hasPendingBackup
      ? `${pendingFiles.length || "部分"} 个数据库将在共享盘恢复后自动备份。`
      : shareUnavailable
        ? "共享盘暂时无法访问。数据会保存在本机，恢复网络后将自动同步。"
        : "部分数据同步未完成，本次将继续使用已有的本机数据。"

    // 已经显示过：更新内容而不是重新弹一条。
    if (toastIdRef.current) {
      toast.update(toastIdRef.current, {
        title,
        description,
        type: "warning",
      })
      return
    }

    const id = toast.add({
      title,
      description,
      type: "warning",
      timeout: 0, // 持续显示，直到状态恢复正常才关闭
      actionProps: hasPendingBackup
        ? {
            children: retryBackup.isPending ? "正在备份…" : "立即备份",
            onClick: () => {
              if (!retryBackup.isPending) {
                retryBackup.mutate()
              }
            },
          }
        : undefined,
    })

    toastIdRef.current = id
  }, [
    isReady,
    runtime,
    shareUnavailable,
    hasPendingBackup,
    hasSyncWarning,
    pendingFiles.length,
    retryBackup.isPending,
    retryBackup.mutate,
  ])

  // 组件卸载时把自己开的 toast 收掉，避免残留。
  useEffect(() => {
    return () => {
      if (toastIdRef.current) {
        toast.close(toastIdRef.current)
      }
    }
  }, [])

  return null
}