// 备份页用的存储状态工具：读取后端状态（每分钟刷新）和体积格式化。

import { useCallback, useEffect, useState } from "react";

import { commands, type StorageStatus } from "@/bindings";

const POLL_MS = 60_000;

/** 容量条颜色：已用 ≤60% 绿，60%~90% 橙，>90% 红。边界整数值归前一档 */
export function usageTone(usedPercent: number): "green" | "orange" | "red" {
  if (usedPercent <= 60) return "green";
  if (usedPercent <= 90) return "orange";
  return "red";
}

export function formatBytes(n: number): string {
  if (n >= 1024 ** 3) return `${(n / 1024 ** 3).toFixed(2)} GB`;
  if (n >= 1024 ** 2) return `${(n / 1024 ** 2).toFixed(1)} MB`;
  return `${Math.max(1, Math.round(n / 1024))} KB`;
}

/** 状态每分钟刷新一次；备份完成后可用 setStatus 直接覆盖 */
export function useStorageStatus() {
  const [status, setStatus] = useState<StorageStatus | null>(null);

  const refresh = useCallback(async () => {
    const r = await commands.getStorageStatus();
    if (r.status === "ok") setStatus(r.data);
  }, []);

  useEffect(() => {
    void refresh();
    const timer = window.setInterval(() => void refresh(), POLL_MS);
    return () => window.clearInterval(timer);
  }, [refresh]);

  return { status, setStatus, refresh };
}
