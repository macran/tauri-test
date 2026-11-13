// 导入页外壳：顶部操作栏（始终可用）+ 下方记录列表（累积，不被覆盖）。
//
//   ┌ 标题 / 说明                     [ 清除已完成 ]  [ 选择文件… ] ┐
//   │ 表单区（可选，如凭证页的接收日期、制表人）                      │
//   ├──────────────────────────────────────────────────────────────┤
//   │ ▾ 最新一次：文件名 · 时间 · 状态      （展开：统计 / 错误表）    │
//   │ ▸ 更早的：文件名 · 时间 · 状态 · 一行摘要                       │
//   └──────────────────────────────────────────────────────────────┘
//
// 选择文件按钮在导入进行中也能点：新文件排到队尾，不需要「重新导入」先回到选择页。
// 整个内容区都是拖放目标（拖入时出蒙层），不再画一个占掉大半页面的虚线框。

import { useCallback, useRef, useState, type ReactNode } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { CaretDown, CaretRight, CircleNotch, Clock, FolderOpen, Upload } from "@phosphor-icons/react";

import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { useFileDrop } from "@/lib/useFileDrop";

import { StatusPill } from "./ImportFeedback";
import type { ImportEntry } from "./useImportQueue";

export interface ImportShellProps<S> {
  title: string;
  description?: ReactNode;
  icon?: ReactNode;
  /** 标题下方的表单（如接收日期 / 制表人）；始终显示，不随导入状态消失 */
  form?: ReactNode;
  /** 表单没填完整等原因导致不能选文件时为 true；按钮和拖放都禁用 */
  disabled?: boolean;
  /** 禁用时按钮上的提示（title） */
  disabledReason?: string;
  entries: ImportEntry<S>[];
  /** 一次选了（或拖入了）若干文件 */
  onFiles: (files: { filePath: string; fileName: string }[]) => void;
  onClear: () => void;
  onRetry: (id: number) => void;
  /** 一条记录折叠时显示的一行摘要（成功 / 失败时才会调用） */
  summaryLine: (summary: S) => ReactNode;
  /** 该记录的状态徽标；不传则成功显示「导入成功」 */
  statusOf?: (summary: S) => { tone: "success" | "warning" | "error"; text: string };
  /** 展开后的详情（统计卡片、错误表） */
  renderResult: (summary: S, entry: ImportEntry<S>) => ReactNode;
  /** 选择对话框里的文件类型 */
  extensions?: string[];
  filterName?: string;
}

const fileNameOf = (p: string) => p.split(/[\\/]/).pop() ?? p;

const timeText = (d: Date) =>
  `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}:${String(d.getSeconds()).padStart(2, "0")}`;

export function ImportShell<S>({
  title,
  description,
  icon,
  form,
  disabled = false,
  disabledReason,
  entries,
  onFiles,
  onClear,
  onRetry,
  summaryLine,
  statusOf,
  renderResult,
  extensions = ["xlsx", "xls"],
  filterName = "Excel 文件",
}: ImportShellProps<S>) {
  const areaRef = useRef<HTMLDivElement>(null);
  const [pickError, setPickError] = useState<string | null>(null);
  // 手动展开 / 折叠的覆盖：key = 记录 id。没覆盖过的，最新一条默认展开
  const [openOverride, setOpenOverride] = useState<Record<number, boolean>>({});

  const handleFiles = useCallback(
    (paths: string[]) => {
      if (disabled || paths.length === 0) return;
      setPickError(null);
      onFiles(paths.map((p) => ({ filePath: p, fileName: fileNameOf(p) })));
    },
    [disabled, onFiles],
  );

  const pick = useCallback(async () => {
    if (disabled) return;
    try {
      const selected = await open({ multiple: true, filters: [{ name: filterName, extensions }] });
      if (!selected) return;
      handleFiles(Array.isArray(selected) ? selected : [selected]);
    } catch (err) {
      setPickError(typeof err === "string" ? err : ((err as Error)?.message ?? "无法打开文件选择对话框"));
    }
  }, [disabled, extensions, filterName, handleFiles]);

  // 拖入多个文件：useFileDrop 的 onFiles 一次给出全部符合扩展名的路径（单文件回调只会取第一个）
  const dragging = useFileDrop(areaRef, () => {}, { disabled, extensions, onFiles: handleFiles });

  const hasFinished = entries.some((e) => e.status === "success" || e.status === "error");
  const isOpen = (e: ImportEntry<S>, index: number) => openOverride[e.id] ?? index === 0;

  return (
    <div ref={areaRef} className="relative flex h-full min-h-0 flex-col">
      {/* 顶部操作栏：始终在，不随导入状态变化 */}
      <header className="shrink-0 border-b border-border px-6 py-4">
        {/* min-h：有表单（标签 + 输入框两层）和没表单的页面，这一行要一样高，
            否则标题、按钮到页签的距离在不同导入页之间不一致 */}
        <div className="flex min-h-14 flex-wrap items-center gap-x-6 gap-y-3">
          <div className="flex min-w-0 items-start gap-3">
            {icon}
            <div className="min-w-0">
              <h1 className="text-base font-semibold text-foreground">{title}</h1>
              {description && <p className="mt-0.5 text-xs text-muted-foreground">{description}</p>}
            </div>
          </div>

          {form && <div className="flex flex-wrap items-end gap-3">{form}</div>}

          <div className="ml-auto flex items-center gap-2">
            {hasFinished && (
              <Button type="button" variant="ghost" size="sm" onClick={onClear}>
                清除已完成
              </Button>
            )}
            <Button type="button" disabled={disabled} title={disabled ? disabledReason : undefined} onClick={() => void pick()}>
              <FolderOpen data-icon="inline-start" />
              选择文件…
            </Button>
          </div>
        </div>
        <p className="mt-2 text-xs text-muted-foreground">
          可一次选择多个文件，按顺序依次导入；也可以直接把文件拖到本页任意位置。
        </p>
        {pickError && <p className="mt-2 text-xs text-destructive">{pickError}</p>}
      </header>

      {/* 记录列表：新的在上，不覆盖 */}
      <div className="thin-scrollbar min-h-0 flex-1 overflow-y-auto px-6 py-4">
        {entries.length === 0 ? (
          <div className="flex h-full flex-col items-center justify-center gap-2 text-muted-foreground">
            <Upload className="size-8 opacity-40" aria-hidden="true" />
            <p className="text-sm">{disabled ? (disabledReason ?? "请先完成上方信息") : "还没有导入记录，点右上角「选择文件…」开始，或把文件拖到这里"}</p>
          </div>
        ) : (
          <ul className="mx-auto flex max-w-5xl flex-col gap-3">
            {entries.map((e, index) => {
              const expanded = isOpen(e, index) && (e.status === "success" || e.status === "error");
              const pill =
                e.status === "queued" ? (
                  <StatusPill tone="info">排队中</StatusPill>
                ) : e.status === "loading" ? (
                  <StatusPill tone="info">正在导入</StatusPill>
                ) : e.status === "error" ? (
                  <StatusPill tone="error">导入失败</StatusPill>
                ) : (
                  (() => {
                    const s = statusOf?.(e.summary) ?? { tone: "success" as const, text: "导入成功" };
                    return <StatusPill tone={s.tone}>{s.text}</StatusPill>;
                  })()
                );
              const canToggle = e.status === "success" || e.status === "error";
              return (
                <li key={e.id} className="rounded-xl border border-border bg-card">
                  <div className="flex items-center gap-3 px-4 py-3">
                    <button
                      type="button"
                      disabled={!canToggle}
                      aria-expanded={expanded}
                      aria-label={expanded ? "折叠" : "展开"}
                      onClick={() => setOpenOverride((o) => ({ ...o, [e.id]: !isOpen(e, index) }))}
                      className="flex min-w-0 flex-1 items-center gap-3 text-left disabled:cursor-default"
                    >
                      {canToggle ? (
                        expanded ? <CaretDown className="size-4 shrink-0" /> : <CaretRight className="size-4 shrink-0" />
                      ) : (
                        <CircleNotch className={cn("size-4 shrink-0", e.status === "loading" && "animate-spin")} />
                      )}
                      <span className="min-w-0 truncate text-sm font-medium text-foreground" title={e.filePath}>
                        {e.fileName}
                      </span>
                      {pill}
                      <span className="hidden shrink-0 items-center gap-1 text-xs text-muted-foreground sm:flex">
                        <Clock className="size-3" aria-hidden="true" />
                        {timeText(e.at)}
                      </span>
                      {!expanded && e.status === "success" && (
                        <span className="min-w-0 truncate text-xs text-muted-foreground">{summaryLine(e.summary)}</span>
                      )}
                      {!expanded && e.status === "error" && (
                        <span className="min-w-0 truncate text-xs text-destructive">{e.message}</span>
                      )}
                    </button>
                    {e.status === "error" && (
                      <Button type="button" variant="outline" size="sm" onClick={() => onRetry(e.id)}>
                        重试
                      </Button>
                    )}
                  </div>

                  {expanded && e.status === "success" && (
                    <div className="border-t border-border px-4 py-4">{renderResult(e.summary, e)}</div>
                  )}
                  {expanded && e.status === "error" && (
                    <div className="border-t border-border px-4 py-4 text-sm text-destructive">{e.message}</div>
                  )}
                </li>
              );
            })}
          </ul>
        )}
      </div>

      {/* 拖入时的整页蒙层 */}
      {dragging && (
        <div className="pointer-events-none absolute inset-0 z-20 flex items-center justify-center border-2 border-dashed border-primary bg-primary/5 backdrop-blur-[1px]">
          <p className="rounded-lg bg-background/90 px-5 py-3 text-base font-medium text-primary shadow-sm">松开以开始导入</p>
        </div>
      )}
    </div>
  );
}
