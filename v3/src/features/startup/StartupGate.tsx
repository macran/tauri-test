import type { PropsWithChildren } from "react";
import { useMemo, useState } from "react";
import {
  Check,
  CheckCircle2,
  Clipboard,
  CloudOff,
  RefreshCw,
  ShieldAlert,
  TriangleAlert,
} from "lucide-react";

import { commands } from "@/bindings";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Card, CardContent, CardHeader, CardTitle } from "@/components/ui/card";

import type { AppError } from "./startup.types";
import { useStartupStatus } from "./useStartupStatus";

export function StartupGate({ children }: PropsWithChildren) {
  const statusQuery = useStartupStatus();

  if (statusQuery.isError) {
    return (
      <StartupFailure
        error={{
          code: "internal",
          severity: "fatal",
          message: "无法读取启动状态。请重启软件；如果问题持续，请联系 IT 支持。",
          detail: statusQuery.error instanceof Error ? statusQuery.error.message : String(statusQuery.error),
        }}
      />
    );
  }

  if (statusQuery.isPending || !statusQuery.data || statusQuery.data.phase === "starting") {
    // 主窗口在初始化完成前不会显示（启动进度由 splash 窗口负责），这里无需占位 UI。
    return null;
  }

  if (statusQuery.data.phase === "failed") {
    return <StartupFailure error={statusQuery.data.error} />;
  }

  // ready：共享盘/备份提示改由 StartupRuntimeToast 以 Toast 形式呈现，
  // StartupGate 本身不再包裹任何横幅 UI。
  return <>{children}</>;
}

function StartupFailure({ error }: { error: AppError }) {
  const [copied, setCopied] = useState(false);

  const diagnosticText = useMemo(() => {
    return [
      `code: ${error.code}`,
      `severity: ${error.severity}`,
      `message: ${error.message}`,
      error.detail ? `detail: ${error.detail}` : null,
    ]
      .filter(Boolean)
      .join("\n");
  }, [error]);

  async function copyDiagnostic() {
    try {
      await navigator.clipboard.writeText(diagnosticText);
      setCopied(true);
      window.setTimeout(() => setCopied(false), 2_500);
    } catch {
      setCopied(false);
    }
  }

  return (
    <main className="h-full overflow-auto bg-[#071521] px-6 py-10 text-slate-100 sm:px-10">
      <div className="mx-auto flex min-h-full w-full max-w-5xl items-center">
        <div className="grid w-full gap-8 lg:grid-cols-[minmax(0,0.82fr)_minmax(0,1.18fr)] lg:items-center">
          <div>
            <div className="mb-8 flex items-center gap-3 text-sm font-medium text-cyan-300">
              <span className="grid size-9 place-items-center rounded-xl bg-cyan-300/10 ring-1 ring-cyan-200/20">
                <ShieldAlert className="size-5" aria-hidden="true" />
              </span>
              <span>Input Tax workspace</span>
            </div>
            <p className="mb-3 text-xs font-semibold uppercase tracking-[0.22em] text-amber-300">
              Startup needs attention
            </p>
            <h1 className="max-w-xl text-4xl font-semibold tracking-tight sm:text-5xl">
              软件没有进入业务界面
            </h1>
            <p className="mt-5 max-w-lg text-base leading-7 text-slate-300">
              数据没有被删除。请按照右侧提示处理；如果需要 IT 支持，可以复制诊断信息一起提交。
            </p>
          </div>

          <Card className="border-amber-200/20 bg-white/[0.07] text-slate-100 shadow-2xl shadow-black/20 backdrop-blur">
            <CardHeader className="border-b border-white/10 pb-5">
              <div className="flex items-center justify-between gap-4">
                <CardTitle className="flex items-center gap-3 text-xl">
                  <span className="grid size-10 place-items-center rounded-xl bg-amber-300/15 text-amber-200">
                    <TriangleAlert className="size-5" aria-hidden="true" />
                  </span>
                  {error.code === "admin_db_missing" || error.code === "admin_db_invalid"
                    ? "用户配置不可用"
                    : "启动检查未完成"}
                </CardTitle>
                <Badge variant="outline" className="border-amber-200/30 text-amber-200">
                  {error.code}
                </Badge>
              </div>
            </CardHeader>
            <CardContent className="space-y-6 p-6 sm:p-8">
              <div role="alert" className="whitespace-pre-line text-sm leading-7 text-slate-200">
                {error.message}
              </div>

              {error.detail ? (
                <details className="rounded-xl border border-white/10 bg-black/10 p-4 text-sm">
                  <summary className="cursor-pointer font-medium text-slate-300 focus:outline-none focus-visible:ring-2 focus-visible:ring-cyan-300">
                    显示技术详情
                  </summary>
                  <pre className="mt-3 max-h-40 overflow-auto whitespace-pre-wrap break-words font-mono text-xs leading-5 text-slate-400">
                    {error.detail}
                  </pre>
                </details>
              ) : null}

              <div className="flex flex-col gap-3 border-t border-white/10 pt-5 sm:flex-row">
                <Button
                  type="button"
                  onClick={() => {
                    // 不能直接用插件的 relaunch：单实例互斥量未释放时新进程会被当成第二个实例而退出
                    void commands.restartApp();
                  }}
                  className="min-h-11 flex-1 bg-cyan-300 text-slate-950 hover:bg-cyan-200"
                >
                  <RefreshCw className="size-4" aria-hidden="true" />
                  重启软件
                </Button>
                <Button
                  type="button"
                  variant="outline"
                  onClick={copyDiagnostic}
                  className="min-h-11 border-white/15 bg-transparent text-slate-200 hover:bg-white/10 hover:text-white"
                >
                  {copied ? <Check className="size-4" aria-hidden="true" /> : <Clipboard className="size-4" aria-hidden="true" />}
                  {copied ? "已复制" : "复制诊断信息"}
                </Button>
              </div>
            </CardContent>
          </Card>
        </div>
      </div>
    </main>
  );
}

export function WorkspaceStatus({ runtime }: { runtime: { username: string; active_le: string; share_available: boolean } }) {
  return (
    <div className="flex items-center gap-2 text-xs text-slate-600" aria-label="工作区状态">
      {runtime.share_available ? (
        <CheckCircle2 className="size-4 text-emerald-600" aria-hidden="true" />
      ) : (
        <CloudOff className="size-4 text-amber-600" aria-hidden="true" />
      )}
      <span>{runtime.share_available ? "已连接共享盘" : "本机模式"}</span>
      <span className="text-slate-400">·</span>
      <span>{runtime.active_le}</span>
      <span className="text-slate-400">·</span>
      <span>{runtime.username}</span>
    </div>
  );
}