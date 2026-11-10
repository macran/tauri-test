// src/TravelExpenseImport.tsx
// Travel Expense 凭证文件导入页面

import { useCallback, useRef, useState } from "react";
import { commands, type TravelExpenseImportSummary } from "@/bindings";
import { open } from "@tauri-apps/plugin-dialog";
import { cn } from "@/lib/utils";

type UploadState =
  | { status: "idle" }
  | { status: "dragging" }
  | { status: "loading"; fileName: string }
  | { status: "success"; fileName: string; summary: TravelExpenseImportSummary }
  | { status: "error"; fileName: string; message: string };

export default function TravelExpenseImport() {
  const [state, setState] = useState<UploadState>({ status: "idle" });
  const dropRef = useRef<HTMLDivElement>(null);

  const runImport = useCallback(async (filePath: string, fileName: string) => {
    setState({ status: "loading", fileName });

    try {
      const result = await commands.importTravelExpenseExcel({ filePath });

      if (result.status === "ok") {
        setState({
          status: "success",
          fileName,
          summary: result.data,
        });
      } else {
        setState({
          status: "error",
          fileName,
          message: result.error || "导入失败",
        });
      }
    } catch (err: unknown) {
      setState({
        status: "error",
        fileName,
        message:
          typeof err === "string"
            ? err
            : ((err as Error)?.message ?? "未知错误"),
      });
    }
  }, []);

  const handleDrop = useCallback(
    (e: React.DragEvent<HTMLDivElement>) => {
      e.preventDefault();
      e.stopPropagation();

      const file = e.dataTransfer.files?.[0];

      if (!file) {
        setState({ status: "idle" });
        return;
      }

      const filePath = (file as File & { path?: string }).path ?? file.name;
      runImport(filePath, file.name);
    },
    [runImport],
  );

  const handleDragOver = useCallback((e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();

    setState((prev) =>
      prev.status === "idle" ? { status: "dragging" } : prev,
    );
  }, []);

  const handleDragLeave = useCallback((e: React.DragEvent<HTMLDivElement>) => {
    e.preventDefault();

    if (dropRef.current && !dropRef.current.contains(e.relatedTarget as Node)) {
      setState((prev) =>
        prev.status === "dragging" ? { status: "idle" } : prev,
      );
    }
  }, []);

  const handlePickFile = useCallback(async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Excel 文件", extensions: ["xlsx", "xls"] }],
    });

    if (!selected) return;

    const filePath = typeof selected === "string" ? selected : selected;
    const fileName = filePath.split(/[\\/]/).pop() ?? filePath;

    runImport(filePath, fileName);
  }, [runImport]);

  const handleReset = () => setState({ status: "idle" });

  const isDragging = state.status === "dragging";

  return (
    <div className="min-h-screen bg-zinc-50 flex flex-col items-center pt-16 px-4">
      {/* 页头 */}
      <div className="mb-10 text-center">
        <h1 className="text-3xl font-semibold tracking-tight text-zinc-900">
          Travel Expense 凭证导入
        </h1>
        <p className="mt-2 text-sm text-zinc-500">
          请先导入认证清单，再导入 Travel Expense
          凭证文件，用于更新凭证号码和入账时间
        </p>
      </div>

      {/* Drop Zone */}
      {(state.status === "idle" || state.status === "dragging") && (
        <div
          ref={dropRef}
          onDrop={handleDrop}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          onClick={handlePickFile}
          className={cn(
            "w-full max-w-xl rounded-2xl border-2 border-dashed transition-all duration-200",
            "flex flex-col items-center justify-center gap-4 py-16 px-8 cursor-pointer",
            isDragging
              ? "border-blue-500 bg-blue-50 scale-[1.01]"
              : "border-zinc-300 bg-white hover:border-zinc-400 hover:bg-zinc-50",
          )}
        >
          <div
            className={cn(
              "rounded-full p-4 transition-colors",
              isDragging ? "bg-blue-100" : "bg-zinc-100",
            )}
          >
            <svg
              className={cn(
                "w-8 h-8",
                isDragging ? "text-blue-500" : "text-zinc-400",
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
                isDragging ? "text-blue-600" : "text-zinc-700",
              )}
            >
              {isDragging ? "松开以开始导入" : "拖拽 Travel Expense 文件到此处"}
            </p>
            <p className="mt-1 text-sm text-zinc-400">或</p>
          </div>

          <button
            type="button"
            onClick={(e) => {
              e.stopPropagation();
              handlePickFile();
            }}
            className={cn(
              "inline-flex items-center gap-2 rounded-lg px-5 py-2.5",
              "text-sm font-medium transition-colors",
              "bg-zinc-900 text-white hover:bg-zinc-700 active:bg-zinc-800",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-zinc-900",
            )}
          >
            选择 Travel Expense 文件
          </button>

          <p className="text-xs text-zinc-400">.xlsx · .xls</p>
        </div>
      )}

      {/* 加载中 */}
      {state.status === "loading" && (
        <div className="w-full max-w-xl rounded-2xl border border-zinc-200 bg-white px-8 py-14 flex flex-col items-center gap-5">
          <div className="relative w-12 h-12">
            <div className="absolute inset-0 rounded-full border-4 border-zinc-100" />
            <div className="absolute inset-0 rounded-full border-4 border-t-blue-500 animate-spin" />
          </div>
          <div className="text-center">
            <p className="text-sm font-medium text-zinc-700">正在导入</p>
            <p className="mt-1 text-xs text-zinc-400 max-w-xs truncate">
              {state.fileName}
            </p>
          </div>
        </div>
      )}

      {/* 命令级错误 */}
      {state.status === "error" && (
        <div className="w-full max-w-xl rounded-2xl border border-red-200 bg-red-50 px-8 py-10 flex flex-col items-center gap-4">
          <div className="rounded-full bg-red-100 p-3">
            <svg
              className="w-6 h-6 text-red-500"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z"
              />
            </svg>
          </div>

          <div className="text-center">
            <p className="text-sm font-semibold text-red-700">导入失败</p>
            <p className="mt-1 text-xs text-red-500">{state.message}</p>
          </div>

          <button
            onClick={handleReset}
            className="mt-2 text-xs text-red-600 underline underline-offset-2 hover:text-red-800"
          >
            重新选择文件
          </button>
        </div>
      )}

      {/* 成功结果 */}
      {state.status === "success" && (
        <TravelExpenseResultCard
          fileName={state.fileName}
          summary={state.summary}
          onReset={handleReset}
        />
      )}
    </div>
  );
}

function TravelExpenseResultCard({
  fileName,
  summary,
  onReset,
}: {
  fileName: string;
  summary: TravelExpenseImportSummary;
  onReset: () => void;
}) {
  const hasErrors = summary.errorCount > 0;
  const isRollback = !summary.committed;
  const isPartialSuccess = summary.committed && hasErrors;

  return (
    <div className="w-full max-w-3xl flex flex-col gap-6">
      <div className="rounded-2xl border border-zinc-200 bg-white p-6">
        <div className="flex items-start justify-between">
          <div>
            <div className="flex items-center gap-2">
              {isRollback ? (
                <StatusBadge color="red" label="已回滚" />
              ) : isPartialSuccess ? (
                <StatusBadge color="amber" label="部分成功" />
              ) : (
                <StatusBadge color="green" label="导入成功" />
              )}
            </div>

            <p className="mt-2 text-sm text-zinc-500 truncate max-w-sm">
              {fileName}
            </p>
          </div>

          <button
            onClick={onReset}
            className="text-xs text-zinc-400 hover:text-zinc-700 transition-colors"
          >
            重新导入
          </button>
        </div>

        <div className="mt-5 grid grid-cols-2 gap-4">
          <StatCard label="成功更新" value={summary.updated} unit="条" />
          <StatCard
            label="错误"
            value={summary.errorCount}
            unit="条"
            danger={summary.errorCount > 0}
          />
        </div>

        {summary.rollbackReason && (
          <div className="mt-4 rounded-lg bg-red-50 border border-red-100 px-4 py-3">
            <p className="text-xs text-red-600">
              <span className="font-medium">回滚原因：</span>
              {summary.rollbackReason}
            </p>
          </div>
        )}
      </div>

      {summary.errorCount > 0 && (
        <div className="rounded-2xl border border-amber-200 bg-amber-50 overflow-hidden">
          <div className="px-6 py-4 border-b border-amber-100">
            <h2 className="text-sm font-semibold text-amber-800">错误明细</h2>
          </div>

          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="bg-amber-50/80 border-b border-amber-100">
                  <Th center>Excel 行</Th>
                  <Th>inv_unq_key</Th>
                  <Th>凭证号码</Th>
                  <Th>错误信息</Th>
                </tr>
              </thead>

              <tbody className="divide-y divide-amber-100">
                {summary.errors.map((err, i) => (
                  <tr
                    key={`${err.row}-${err.invUnqKey}-${i}`}
                    className="hover:bg-amber-50/60 transition-colors"
                  >
                    <td className="px-6 py-3 text-center text-xs text-zinc-500">
                      {err.row}
                    </td>
                    <td className="px-6 py-3 text-xs font-mono text-zinc-700">
                      {err.invUnqKey || "—"}
                    </td>
                    <td className="px-6 py-3 text-xs font-mono text-zinc-700">
                      {err.voucherNo || "—"}
                    </td>
                    <td className="px-6 py-3 text-xs text-red-600">
                      {err.message}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}
    </div>
  );
}

function StatusBadge({
  color,
  label,
}: {
  color: "green" | "amber" | "red";
  label: string;
}) {
  const className =
    color === "green"
      ? "bg-emerald-50 text-emerald-700 ring-emerald-200"
      : color === "amber"
        ? "bg-amber-50 text-amber-700 ring-amber-200"
        : "bg-red-50 text-red-700 ring-red-200";

  const dot =
    color === "green"
      ? "bg-emerald-500"
      : color === "amber"
        ? "bg-amber-500"
        : "bg-red-500";

  return (
    <span
      className={cn(
        "inline-flex items-center gap-1.5 rounded-full px-2.5 py-0.5 text-xs font-medium ring-1",
        className,
      )}
    >
      <span className={cn("w-1.5 h-1.5 rounded-full", dot)} />
      {label}
    </span>
  );
}

function StatCard({
  label,
  value,
  unit,
  danger,
}: {
  label: string;
  value: number;
  unit: string;
  danger?: boolean;
}) {
  return (
    <div
      className={cn(
        "rounded-xl border px-4 py-3",
        danger ? "bg-red-50 border-red-100" : "bg-zinc-50 border-zinc-100",
      )}
    >
      <p className="text-xs text-zinc-400">{label}</p>
      <p
        className={cn(
          "mt-1 text-2xl font-semibold tabular-nums",
          danger ? "text-red-600" : "text-zinc-900",
        )}
      >
        {value.toLocaleString()}
        <span className="ml-1 text-sm font-normal text-zinc-400">{unit}</span>
      </p>
    </div>
  );
}

function Th({
  children,
  center,
}: {
  children: React.ReactNode;
  center?: boolean;
}) {
  return (
    <th
      className={cn(
        "px-6 py-3 text-xs font-medium text-zinc-500 uppercase tracking-wide",
        center ? "text-center" : "text-left",
      )}
    >
      {children}
    </th>
  );
}
