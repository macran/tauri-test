// 标题栏的 LE（法人实体）切换器。
// 切换会关闭所有标签页并重新加载页面数据，所以选中后先确认；确认后后端像“重新打开软件”那样
// 备份当前库、同步/创建目标库、替换连接，并按阶段上报进度（le-switch-progress）。
// 任何用户都可以切换到任何有效 LE（互相备份工作需要）。

import { ArrowRight, Buildings } from "@phosphor-icons/react";
import { useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState } from "react";

import { commands, type StartupProgress, type SwitchableLes } from "@/bindings";
import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { AppButton } from "@/components/app-button";
import { Progress, ProgressLabel, ProgressValue } from "@/components/ui/progress";
import { toast } from "@/components/ui/toast";
import { useAppStore } from "@/store/useAppStore";

/** 完成后在 100% 停一下再关弹窗，让用户看到“做完了”，而不是进度条跳到一半就消失 */
const DONE_HOLD_MS = 400;

type Phase = "confirm" | "switching";

export default function LeSwitcher() {
  const queryClient = useQueryClient();
  const resetForLeSwitch = useAppStore((s) => s.resetForLeSwitch);
  const leSwitchRequest = useAppStore((s) => s.leSwitchRequest);
  const clearLeSwitchRequest = useAppStore((s) => s.clearLeSwitchRequest);
  const setLeOptions = useAppStore((s) => s.setLeOptions);

  const [data, setData] = useState<SwitchableLes | null>(null);
  const [target, setTarget] = useState<string | null>(null);
  const [phase, setPhase] = useState<Phase>("confirm");
  const [progress, setProgress] = useState<StartupProgress>({ percent: 0, message: "正在准备…" });
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    void commands.listSwitchableLes().then((r) => {
      if (r.status === "ok") setData(r.data);
    });
  }, []);

  // 把当前 LE 和可选列表同步给用户菜单（菜单里的「切换 LE」读它）
  useEffect(() => {
    setLeOptions(data ? { current: data.current, les: data.les } : null);
  }, [data, setLeOptions]);

  // 用户菜单选了某个 LE：打开确认框（和原下拉选择之后的流程完全一致）
  useEffect(() => {
    if (leSwitchRequest && data && leSwitchRequest !== data.current) setTarget(leSwitchRequest);
    if (leSwitchRequest) clearLeSwitchRequest();
  }, [leSwitchRequest, data, clearLeSwitchRequest]);

  // 只在切换期间订阅进度事件；进度只增不减，晚到的旧事件不能让进度条倒退
  useEffect(() => {
    if (phase !== "switching") return;
    const unlisten = listen<StartupProgress>("le-switch-progress", (e) =>
      setProgress((p) => (e.payload.percent >= p.percent ? e.payload : p)),
    );
    return () => void unlisten.then((fn) => fn());
  }, [phase]);

  if (!data) return null;
  const current = data.current;
  const switching = phase === "switching";

  function close() {
    setTarget(null);
    setPhase("confirm");
    setError(null);
  }

  async function run() {
    if (!target) return;
    setError(null);
    setProgress({ percent: 0, message: "正在准备…" });
    setPhase("switching");

    const r = await commands.switchLe(target);
    if (r.status !== "ok") {
      // 回到确认视图并显示原因，用户可以重试或取消，不必关掉再重新打开
      setError(r.error);
      setPhase("confirm");
      return;
    }

    setProgress({ percent: 100, message: "切换完成" });
    await new Promise((resolve) => window.setTimeout(resolve, DONE_HOLD_MS));

    setData((d) => (d ? { ...d, current: r.data.le } : d));
    // 先清缓存再重置页面：重新挂载的页面一上来就会查询，不能读到上一个 LE 的缓存
    queryClient.clear();
    resetForLeSwitch();
    close();
    toast.add({
      type: r.data.warnings.length ? "warning" : "success",
      title: `已切换到 LE ${r.data.le}`,
      description: r.data.warnings.join("\n") || undefined,
    });
  }

  return (
    <>
      {/* 只读：标题栏只告诉用户当前在哪个 LE；切换入口在用户菜单里（切换会关闭所有标签，不适合放在一点就弹的位置） */}
      <div
        className="flex items-center gap-1.5 rounded-lg py-1 pr-2 pl-2 text-[12px] font-medium"
        style={{
          background: "var(--input-bg)",
          border: "1px solid var(--input-border)",
          color: "var(--text-secondary)",
        }}
        title="当前法人实体；在用户菜单中切换"
      >
        <Buildings weight="duotone" className="h-3.5 w-3.5 text-primary" aria-hidden="true" />
        <span>LE {current}</span>
      </div>

      <AlertDialog
        open={target !== null}
        // 切换中不允许关闭：Esc 和点遮罩都走这里，否则用户以为取消了，后台其实还在跑
        onOpenChange={(open) => {
          if (!open && !switching) close();
        }}
      >
        <AlertDialogContent className="sm:max-w-sm">
          {switching ? (
            <div role="status" aria-live="polite" className="flex flex-col gap-4">
              <AlertDialogHeader>
                <AlertDialogTitle>正在切换到 LE {target}</AlertDialogTitle>
                <AlertDialogDescription>请勿关闭软件，完成后会自动回到首页。</AlertDialogDescription>
              </AlertDialogHeader>
              <Progress value={progress.percent} className="gap-2">
                <ProgressLabel className="min-w-0 flex-1 truncate text-xs font-normal text-muted-foreground">
                  {progress.message}
                </ProgressLabel>
                <ProgressValue className="text-xs">{() => `${Math.round(progress.percent)}%`}</ProgressValue>
              </Progress>
            </div>
          ) : (
            <>
              <AlertDialogHeader>
                <AlertDialogTitle className="flex items-center gap-2">
                  <span className="text-muted-foreground">LE {current}</span>
                  <ArrowRight className="size-4 text-muted-foreground" />
                  <span>LE {target}</span>
                </AlertDialogTitle>
                <AlertDialogDescription>切换将依次完成：</AlertDialogDescription>
              </AlertDialogHeader>

              {/* 列表不能放进 Description：它渲染成 <p>，里面嵌 <ul> 不合法 */}
              <ul className="flex list-disc flex-col gap-1 pl-5 text-xs text-muted-foreground">
                <li>备份 LE {current} 的数据到共享盘</li>
                <li>打开 LE {target}（本机没有会新建）</li>
                <li>关闭所有已打开的标签页，未保存的内容会丢失</li>
              </ul>

              {error && <p className="text-xs text-destructive">{error}</p>}

              <AlertDialogFooter>
                <AppButton type="button" variant="glass" onClick={close}>
                  取消
                </AppButton>
                <AppButton type="button" variant="brand" onClick={() => void run()}>
                  {error ? "重试" : "切换"}
                </AppButton>
              </AlertDialogFooter>
            </>
          )}
        </AlertDialogContent>
      </AlertDialog>
    </>
  );
}
