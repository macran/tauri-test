// 凭证导入页共用的"页头 + 接收日期/制表人 + 文件选择区"。
// 接收日期或制表人任一为空时，整块上传区禁用：卡片、图标、按钮、拖放都不响应，也不提示错误。
// 结果区（统计、错误表）仍由各页自己渲染，传入 children。

import { useCallback, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import { Win11DatePicker } from "@/components/ui/win11-date-picker";
import { cn } from "@/lib/utils";
import { useFileDrop } from "@/lib/useFileDrop";
import { parseLocalDate, toIsoDate } from "@/features/query/fields";

export interface VoucherUploadPanelProps {
  title: string;
  /** 拖拽区文案，如"应付凭证" */
  kindLabel: string;
  /** 选择了文件（或拖入文件）后触发，已保证接收日期与制表人非空 */
  onFile: (args: {
    filePath: string;
    fileName: string;
    receivedDate: string;
    preparer: string;
  }) => void;
  /** 为 true 时（导入中/出结果）隐藏选择区，只保留页头与表单 */
  hideDropZone: boolean;
  /** 卡片内、表单下方（导入中 / 导入失败） */
  status?: React.ReactNode;
  /** 卡片外、下方整行（导入结果统计与错误表） */
  children?: React.ReactNode;
}

export function VoucherUploadPanel({
  title,
  kindLabel,
  onFile,
  hideDropZone,
  status,
  children,
}: VoucherUploadPanelProps) {
  const [receivedDate, setReceivedDate] = useState("");
  const [preparer, setPreparer] = useState("");
  const dropRef = useRef<HTMLDivElement>(null);

  const canImport = receivedDate !== "" && preparer.trim() !== "";

  const submit = useCallback(
    (filePath: string, fileName: string) =>
      onFile({ filePath, fileName, receivedDate, preparer: preparer.trim() }),
    [onFile, receivedDate, preparer],
  );

  const pickFile = useCallback(async () => {
    if (!canImport) return;
    const selected = await open({
      multiple: false,
      filters: [{ name: "Excel 文件", extensions: ["xlsx", "xls"] }],
    });
    if (!selected) return;
    submit(selected, selected.split(/[\\/]/).pop() ?? selected);
  }, [canImport, submit]);

  // 未填完整时 disabled：拖入既不高亮也不触发
  const dragging = useFileDrop(dropRef, submit, { disabled: !canImport });

  return (
    // 四个凭证导入页统一用 pt-12，保证内容区与顶部间距一致
    <div className="flex flex-col items-center px-4 pt-12">
      <div className="mb-4 text-center">
        <h1 className="text-xl font-semibold tracking-tight">{title}</h1>
      </div>

      <div className="w-full max-w-3xl rounded-2xl bg-card p-6 surface-card">
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
          <div className="flex flex-col gap-1.5">
            <Label className="text-xs text-muted-foreground">接收日期</Label>
            <Win11DatePicker
              value={parseLocalDate(receivedDate)}
              onChange={(d) => setReceivedDate(toIsoDate(d))}
            />
          </div>
          <div className="flex flex-col gap-1.5">
            <Label className="text-xs text-muted-foreground">制表人</Label>
            <Input
              value={preparer}
              onChange={(e) => setPreparer(e.target.value)}
              placeholder="请输入制表人"
            />
          </div>
        </div>

        {!hideDropZone && (
          <div
            ref={dropRef}
            aria-disabled={!canImport}
            // 未填完整时去掉所有指针事件：卡片、图标、按钮都不会响应点击
            onClick={canImport ? pickFile : undefined}
            className={cn(
              "mt-6 flex flex-col items-center justify-center gap-4",
              "rounded-2xl border-2 border-dashed px-8 py-14 transition-all duration-200",
              canImport
                ? cn(
                    "cursor-pointer",
                    dragging
                      ? "scale-[1.01] border-blue-500 bg-blue-50"
                      : "border-zinc-300 bg-white hover:border-zinc-400 hover:bg-zinc-50",
                  )
                : "pointer-events-none cursor-not-allowed border-zinc-200 bg-zinc-50 opacity-50",
            )}
          >
            <div
              className={cn(
                "rounded-full p-4 transition-colors",
                dragging ? "bg-blue-100" : "bg-zinc-100",
              )}
            >
              <svg
                className={cn(
                  "h-8 w-8",
                  dragging ? "text-blue-500" : "text-zinc-400",
                )}
                fill="none"
                viewBox="0 0 24 24"
                stroke="currentColor"
                strokeWidth={1.5}
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  d="M3 16.5v2.25A2.25 2.25 0 005.25 21h13.5A2.25 2.25 0 0021 18.75V16.5m-13.5-9L12 3m0 0l4.5 4.5M12 3v13.5"
                />
              </svg>
            </div>

            <div className="text-center">
              <p
                className={cn(
                  "text-base font-medium",
                  dragging ? "text-blue-600" : "text-zinc-700",
                )}
              >
                {dragging ? "松开以开始导入" : `拖拽${kindLabel} Excel 到此处`}
              </p>
              <p className="mt-1 text-sm text-zinc-400">或点击选择文件</p>
            </div>

            <button
              type="button"
              disabled={!canImport}
              onClick={(e) => {
                e.stopPropagation();
                pickFile();
              }}
              className={cn(
                "inline-flex items-center rounded-lg px-5 py-2.5",
                "text-sm font-medium transition-colors",
                "bg-zinc-900 text-white hover:bg-zinc-700 active:bg-zinc-800",
                "disabled:cursor-not-allowed disabled:opacity-50",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-zinc-900",
              )}
            >
              选择{kindLabel}文件
            </button>

            <p className="text-xs text-zinc-400">.xlsx · .xls</p>
          </div>
        )}

        {status}
      </div>

      {children && <div className="mt-6 flex w-full max-w-3xl flex-col gap-6">{children}</div>}
    </div>
  );
}
