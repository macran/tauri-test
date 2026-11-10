import { ReceiptIcon, WarningCircleIcon, XIcon } from "@phosphor-icons/react";
import { listen } from "@tauri-apps/api/event";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { useEffect, useRef, useState } from "react";
import { Button } from "@/components/ui/button";
import {
  retryStartup,
  type StartupStatus,
  uploadLocalToShared,
} from "@/lib/tauri";

const TITLES: Record<NonNullable<StartupStatus["kind"]>, string> = {
  localLocked: "本地数据库被占用",
  sharedUnreachable: "无法连接共享网盘",
  localNewer: "本地有未上传的改动",
  other: "启动失败",
};

const failed = (e: unknown): StartupStatus => ({
  ready: false,
  kind: "other",
  message: String(e),
  files: [],
});

/** splash 窗口：显示启动步骤；成功后由后端创建主窗口并关闭本窗口。 */
export default function Splash() {
  const [status, setStatus] = useState<StartupStatus | null>(null);
  const [busy, setBusy] = useState(true);
  const [step, setStep] = useState("正在准备…");
  const started = useRef(false);

  const run = async (action: () => Promise<StartupStatus>) => {
    setBusy(true);
    setStatus(null);
    try {
      setStatus(await action());
    } catch (e) {
      setStatus(failed(e));
    } finally {
      setBusy(false);
    }
  };

  useEffect(() => {
    // 窗口配置为 visible=false，内容渲染好再显示，避免先看到空白窗口。
    void getCurrentWindow().show();
    const unlisten = listen<string>("startup-step", (e) => setStep(e.payload));
    if (!started.current) {
      // StrictMode 下 effect 会执行两次，只触发一次启动
      started.current = true;
      void run(retryStartup);
    }
    return () => {
      void unlisten.then((fn) => fn());
    };
  }, []);

  const error = !busy && status?.kind ? status : null;

  return (
    <div className="relative flex h-screen w-screen overflow-hidden bg-background">
      {/* 无边框窗口：整块可拖动，右上角自带关闭 */}
      <div data-tauri-drag-region className="absolute inset-0" />
      <button
        type="button"
        aria-label="关闭"
        onClick={() => getCurrentWindow().close()}
        className="absolute top-2 right-2 z-10 rounded-md p-1.5 text-muted-foreground hover:bg-muted"
      >
        <XIcon className="size-4" />
      </button>

      <div className="pointer-events-none flex w-[200px] shrink-0 flex-col justify-between bg-gradient-to-br from-primary to-primary/70 p-6 text-primary-foreground">
        <ReceiptIcon className="size-12" weight="duotone" />
        <div>
          <div className="text-lg font-semibold tracking-wide">发票管理</div>
          <div className="mt-1 text-xs opacity-80">导入 · 对账 · 红冲匹配</div>
        </div>
      </div>

      <div className="relative flex flex-1 flex-col p-8">
        {error?.kind ? (
          <>
            <div className="flex items-center gap-2 text-destructive">
              <WarningCircleIcon className="size-6" weight="fill" />
              <h1 className="text-lg font-semibold">{TITLES[error.kind]}</h1>
            </div>
            <div className="mt-3 flex-1 overflow-auto whitespace-pre-wrap break-words pr-1 text-sm text-muted-foreground">
              {error.message}
            </div>
            <div className="mt-4 flex justify-end gap-2">
              {error.kind === "localNewer" && (
                <Button size="sm" onClick={() => run(uploadLocalToShared)}>
                  立即上传到共享网盘
                </Button>
              )}
              <Button
                size="sm"
                variant="outline"
                onClick={() => run(retryStartup)}
              >
                重试
              </Button>
              <Button
                size="sm"
                variant="ghost"
                onClick={() => getCurrentWindow().close()}
              >
                退出
              </Button>
            </div>
          </>
        ) : (
          <>
            <div className="flex flex-1 flex-col justify-center gap-4">
              <h1 className="text-2xl font-semibold">正在连接数据库</h1>
              <div className="h-1 w-full overflow-hidden rounded-full bg-muted">
                <div className="h-full w-1/3 animate-[splash-slide_1.4s_ease-in-out_infinite] rounded-full bg-primary" />
              </div>
            </div>
            <p className="text-xs text-muted-foreground">{step}</p>
          </>
        )}
      </div>
    </div>
  );
}
