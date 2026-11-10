// 备份与同步：显示当前库的容量、共享盘状态、待备份文件，并提供“立即备份”。
// 退出时的自动备份、运行期间的后台重试、下次启动补备份都在后端，不受这个页面影响——
// 用户强制结束进程时这些兜底仍然有效。这里只是把状态露出来，并给一个手动入口。

import { ArrowsClockwise, CircleNotch, CloudCheck, CloudSlash } from "@phosphor-icons/react";
import { useState } from "react";

import { commands } from "@/bindings";
import { AppButton } from "@/components/app-button";
import { Card, CardContent } from "@/components/ui/card";
import { toast } from "@/components/ui/toast";
import { cn } from "@/lib/utils";

import { formatBytes, usageTone, useStorageStatus } from "./storage-status";

const BAR: Record<ReturnType<typeof usageTone>, string> = {
  green: "bg-emerald-500",
  orange: "bg-orange-500",
  red: "bg-red-500",
};

function formatTime(secs: number): string {
  return new Date(secs * 1000).toLocaleString("zh-CN", { hour12: false });
}

function Row({ label, children }: { label: string; children: React.ReactNode }) {
  return (
    <div className="flex items-start gap-4 border-b py-2 last:border-b-0">
      <div className="w-28 shrink-0 text-xs font-semibold">{label}</div>
      <div className="min-w-0 flex-1 text-xs text-muted-foreground">{children}</div>
    </div>
  );
}

export default function BackupSync() {
  const { status, setStatus, refresh } = useStorageStatus();
  const [running, setRunning] = useState(false);

  async function backup() {
    setRunning(true);
    try {
      const r = await commands.backupNow();
      if (r.status === "ok") {
        setStatus(r.data);
        toast.add({ type: "success", title: "已备份到共享盘" });
      } else {
        // 失败时数据仍在本机、待备份台账已更新；刷新让页面显示最新的待备份列表
        toast.add({ type: "error", title: "备份未完成", description: r.error });
        await refresh();
      }
    } finally {
      setRunning(false);
    }
  }

  const percent = status && status.limitBytes > 0 ? (status.usedBytes / status.limitBytes) * 100 : 0;

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col gap-3 p-3">
      <Card>
        <CardContent className="flex flex-col">
          {!status ? (
            <div className="py-6 text-center text-xs text-muted-foreground">正在读取状态…</div>
          ) : (
            <>
              <Row label="共享盘">
                {status.shareAvailable ? (
                  <span className="inline-flex items-center gap-1.5 text-foreground">
                    <CloudCheck weight="duotone" className="size-4 text-emerald-500" />
                    已连接
                  </span>
                ) : (
                  <span className="inline-flex items-center gap-1.5 text-foreground">
                    <CloudSlash weight="duotone" className="size-4 text-orange-500" />
                    未连接，数据只保存在本机；恢复后会自动补备份
                  </span>
                )}
              </Row>

              <Row label="当前数据库">
                <div className="flex flex-col gap-1.5">
                  <span className="text-foreground">{status.dbFile}</span>
                  <div className="flex items-center gap-3">
                    <div className="h-2 w-64 overflow-hidden bg-muted">
                      <div
                        className={cn("h-full", BAR[usageTone(percent)])}
                        style={{ width: `${Math.min(100, Math.max(1, percent))}%` }}
                      />
                    </div>
                    <span className="tabular-nums">
                      已用 {formatBytes(status.usedBytes)} / {formatBytes(status.limitBytes)}（剩余{" "}
                      {Math.max(0, 100 - percent).toFixed(1)}%）
                    </span>
                  </div>
                </div>
              </Row>

              <Row label="上次备份">
                {status.lastBackupAt ? formatTime(status.lastBackupAt) : "本机尚无成功备份记录"}
              </Row>

              <Row label="待备份">
                {status.pendingFiles.length === 0 ? (
                  "无"
                ) : (
                  <ul className="flex flex-col gap-0.5 text-orange-500">
                    {status.pendingFiles.map((f) => (
                      <li key={f}>{f}</li>
                    ))}
                  </ul>
                )}
              </Row>

              <div className="flex items-center gap-3 pt-3">
                <AppButton type="button" variant="brand" disabled={running} onClick={() => void backup()}>
                  {running ? (
                    <CircleNotch data-icon="inline-start" className="animate-spin" />
                  ) : (
                    <ArrowsClockwise data-icon="inline-start" weight="bold" />
                  )}
                  立即备份
                </AppButton>
                <span className="text-xs text-muted-foreground">
                  退出软件时会自动备份；备份期间新写入的数据可能要到下次备份才会包含。
                </span>
              </div>
            </>
          )}
        </CardContent>
      </Card>
    </div>
  );
}
