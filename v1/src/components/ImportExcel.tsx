// src/InvoiceImport.tsx
// 全量发票导入页面
// 依赖: shadcn/ui, tailwind v4, @tauri-apps/api, src/bindings.ts

import { useState, useCallback, useRef } from "react";
import {
  commands,
  type ImportSummary,
  type SheetImportResult,
} from "@/bindings";
import { open } from "@tauri-apps/plugin-dialog";
import { cn } from "@/lib/utils";
import { Card } from "./ui/card";

// ─── 状态机类型 ───────────────────────────────────────────────
type UploadState =
  | { status: "idle" }
  | { status: "dragging" }
  | { status: "loading"; fileName: string }
  | { status: "success"; fileName: string; summary: ImportSummary }
  | { status: "error"; fileName: string; message: string };

// ─── 主组件 ──────────────────────────────────────────────────
export default function InvoiceImport() {
  const [state, setState] = useState<UploadState>({ status: "idle" });
  const dropRef = useRef<HTMLDivElement>(null);

  // ── 核心导入逻辑 ──────────────────────────────────────────
  const runImport = useCallback(async (filePath: string, fileName: string) => {
    setState({ status: "loading", fileName });
    try {
      const result = await commands.importInvoiceExcel({ filePath });

      if (result.status === "ok") {
        setState({ status: "success", fileName, summary: result.data });
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

  // ── Drag & Drop 处理 ──────────────────────────────────────
  const handleDrop = useCallback(
    (e: React.DragEvent<HTMLDivElement>) => {
      e.preventDefault();
      e.stopPropagation();
      const file = e.dataTransfer.files?.[0];
      if (!file) return setState({ status: "idle" });
      // Tauri 桌面端 File 对象有 .path 属性
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
    // 只有真正离开 drop zone 才重置（防止子元素触发）
    if (dropRef.current && !dropRef.current.contains(e.relatedTarget as Node)) {
      setState((prev) =>
        prev.status === "dragging" ? { status: "idle" } : prev,
      );
    }
  }, []);

  // ── 点击按钮选择文件（使用 tauri-plugin-dialog）────────────
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
  // const isLoading = state.status === "loading";

  return (
    <div className="min-h-screen bg-zinc-50 flex flex-col items-center pt-16 px-4">
      {/* ── 页头 ── */}
      <div className="mb-10 text-center">
        <h1 className="text-3xl font-semibold tracking-tight text-zinc-900">
          全量发票导入
        </h1>
        <p className="mt-2 text-sm text-zinc-500">
          支持全量发票 Excel 文件，选择后自动解析全部 Sheet 并写入数据库
        </p>
      </div>
      

      {/* ── Drop Zone（idle / dragging 状态显示）── */}
      {(state.status === "idle" || state.status === "dragging") && (
        <div
          ref={dropRef}
          onDrop={handleDrop}
          onDragOver={handleDragOver}
          onDragLeave={handleDragLeave}
          className={cn(
            "w-full max-w-xl rounded-2xl border-2 border-dashed transition-all duration-200",
            "flex flex-col items-center justify-center gap-4 py-16 px-8 cursor-pointer",
            isDragging
              ? "border-blue-500 bg-blue-50 scale-[1.01]"
              : "border-zinc-300 bg-white hover:border-zinc-400 hover:bg-zinc-50",
          )}
          onClick={handlePickFile}
        >
          {/* 图标 */}
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

          {/* 文字 */}
          <div className="text-center">
            <p
              className={cn(
                "text-base font-medium",
                isDragging ? "text-blue-600" : "text-zinc-700",
              )}
            >
              {isDragging ? "松开以开始导入" : "拖拽发票文件到此处"}
            </p>
            <p className="mt-1 text-sm text-zinc-400">或</p>
          </div>

          {/* 按钮 */}
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
            <svg
              className="w-4 h-4"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={2}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M15.75 17.25v3.375c0 .621-.504 1.125-1.125 1.125h-9.75a1.125 1.125 0 01-1.125-1.125V7.875c0-.621.504-1.125 1.125-1.125H6.75a9.06 9.06 0 011.5.124m7.5 10.376h3.375c.621 0 1.125-.504 1.125-1.125V11.25c0-4.46-3.243-8.161-7.5-8.876a9.06 9.06 0 00-1.5-.124H9.375c-.621 0-1.125.504-1.125 1.125v3.5m7.5 10.375H9.375a1.125 1.125 0 01-1.125-1.125v-9.25m12 6.625v-1.875a3.375 3.375 0 00-3.375-3.375h-1.5a1.125 1.125 0 01-1.125-1.125v-1.5a3.375 3.375 0 00-3.375-3.375H9.75"
              />
            </svg>
            选择全量发票文件
          </button>

          <p className="text-xs text-zinc-400">.xlsx · .xls</p>
        </div>
      )}

      {/* ── 加载中 ── */}
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

      {/* ── 错误 ── */}
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

      {/* ── 成功：结果表格 ── */}
      {state.status === "success" && (
        <div className="w-full max-w-3xl flex flex-col gap-6">
          {/* 汇总卡片 */}
          <div className="rounded-2xl border border-zinc-200 bg-white p-6">
            <div className="flex items-start justify-between">
              <div>
                <div className="flex items-center gap-2">
                  {state.summary.committed ? (
                    <span className="inline-flex items-center gap-1.5 rounded-full bg-emerald-50 px-2.5 py-0.5 text-xs font-medium text-emerald-700 ring-1 ring-emerald-200">
                      <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
                      导入成功
                    </span>
                  ) : (
                    <span className="inline-flex items-center gap-1.5 rounded-full bg-red-50 px-2.5 py-0.5 text-xs font-medium text-red-700 ring-1 ring-red-200">
                      <span className="w-1.5 h-1.5 rounded-full bg-red-500" />
                      已回滚
                    </span>
                  )}
                  <span className="text-xs text-zinc-400">
                    批次号 #{state.summary.batchNo}
                  </span>
                </div>
                <p className="mt-2 text-sm text-zinc-500 truncate max-w-sm">
                  {state.fileName}
                </p>
              </div>
              <button
                onClick={handleReset}
                className="text-xs text-zinc-400 hover:text-zinc-700 transition-colors"
              >
                重新导入
              </button>
            </div>

            {/* 统计数字 */}
            <div className="mt-5 grid grid-cols-3 gap-4">
              <StatCard
                label="发票头"
                value={state.summary.headerInserted}
                unit="条"
              />
              <StatCard
                label="明细记录"
                value={state.summary.detailInserted}
                unit="条"
              />
              <StatCard
                label="供应商"
                value={state.summary.sellerCount}
                unit="家"
              />
            </div>

            {/* 回滚原因 */}
            {state.summary.rollbackReason && (
              <div className="mt-4 rounded-lg bg-red-50 border border-red-100 px-4 py-3">
                <p className="text-xs text-red-600">
                  <span className="font-medium">回滚原因：</span>
                  {state.summary.rollbackReason}
                </p>
              </div>
            )}
          </div>

          {/* Sheet 明细表格 */}
          <div className="rounded-2xl border border-zinc-200 bg-white overflow-hidden">
            <div className="px-6 py-4 border-b border-zinc-100">
              <h2 className="text-sm font-semibold text-zinc-800">
                各 Sheet 导入详情
              </h2>
            </div>
            <div className="overflow-x-auto">
              <table className="w-full text-sm">
                <thead>
                  <tr className="bg-zinc-50 border-b border-zinc-100">
                    <Th>Sheet 名称</Th>
                    <Th center>识别</Th>
                    <Th center>导入记录</Th>
                    <Th center>错误数</Th>
                    <Th>状态</Th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-zinc-100">
                  {state.summary.sheets.map((sheet) => (
                    <SheetRow key={sheet.sheetName} sheet={sheet} />
                  ))}
                </tbody>
              </table>
            </div>
          </div>

          {/* 错误详情（有错误时展开） */}
          {state.summary.sheets.some((s) => s.errors.length > 0) && (
            <div className="rounded-2xl border border-amber-200 bg-amber-50 overflow-hidden">
              <div className="px-6 py-4 border-b border-amber-100">
                <h2 className="text-sm font-semibold text-amber-800">
                  错误单元格详情
                </h2>
              </div>
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="bg-amber-50/80 border-b border-amber-100">
                      <Th>Sheet</Th>
                      <Th center>行</Th>
                      <Th center>列</Th>
                      <Th>错误信息</Th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-amber-100">
                    {state.summary.sheets.flatMap((sheet) =>
                      sheet.errors.map((err, i) => (
                        <tr
                          key={`${sheet.sheetName}-${i}`}
                          className="hover:bg-amber-50/60 transition-colors"
                        >
                          <td className="px-6 py-3 text-xs font-medium text-zinc-700">
                            {sheet.sheetName}
                          </td>
                          <td className="px-6 py-3 text-center text-xs text-zinc-500">
                            {err.row}
                          </td>
                          <td className="px-6 py-3 text-center text-xs font-mono text-zinc-500">
                            {err.col}
                          </td>
                          <td className="px-6 py-3 text-xs text-red-600">
                            {err.message}
                          </td>
                        </tr>
                      )),
                    )}
                  </tbody>
                </table>
              </div>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ─── 子组件 ──────────────────────────────────────────────────

function StatCard({
  label,
  value,
  unit,
}: {
  label: string;
  value: number;
  unit: string;
}) {
  return (
    <div className="card rounded-lg p-4 text-center">
      <p className="text-xs text-zinc-400">{label}</p>
      <p className="mt-1 text-2xl font-semibold text-zinc-900 tabular-nums">
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

function SheetRow({ sheet }: { sheet: SheetImportResult }) {
  const hasErrors = sheet.errorCount > 0;
  const notRecognized = !sheet.recognized;

  return (
    <tr className="hover:bg-zinc-50/80 transition-colors">
      {/* Sheet 名称 */}
      <td className="px-6 py-3.5">
        <span className="font-medium text-zinc-800">{sheet.sheetName}</span>
      </td>

      {/* 识别 */}
      <td className="px-6 py-3.5 text-center">
        {sheet.recognized ? (
          <span className="inline-flex items-center justify-center w-5 h-5 rounded-full bg-emerald-100">
            <svg
              className="w-3 h-3 text-emerald-600"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={3}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M4.5 12.75l6 6 9-13.5"
              />
            </svg>
          </span>
        ) : (
          <span className="inline-flex items-center justify-center w-5 h-5 rounded-full bg-zinc-100">
            <svg
              className="w-3 h-3 text-zinc-400"
              fill="none"
              viewBox="0 0 24 24"
              stroke="currentColor"
              strokeWidth={3}
            >
              <path
                strokeLinecap="round"
                strokeLinejoin="round"
                d="M6 18L18 6M6 6l12 12"
              />
            </svg>
          </span>
        )}
      </td>

      {/* 导入记录数 */}
      <td className="px-6 py-3.5 text-center">
        <span
          className={cn(
            "tabular-nums font-medium",
            notRecognized ? "text-zinc-300" : "text-zinc-800",
          )}
        >
          {notRecognized ? "—" : sheet.inserted.toLocaleString()}
        </span>
      </td>

      {/* 错误数 */}
      <td className="px-6 py-3.5 text-center">
        {hasErrors ? (
          <span className="inline-flex items-center justify-center rounded-full bg-red-100 px-2 py-0.5 text-xs font-semibold text-red-600">
            {sheet.errorCount}
          </span>
        ) : (
          <span className="text-zinc-300 text-xs">—</span>
        )}
      </td>

      {/* 状态 badge */}
      <td className="px-6 py-3.5">
        <StatusBadge sheet={sheet} />
      </td>
    </tr>
  );
}

function StatusBadge({ sheet }: { sheet: SheetImportResult }) {
  if (!sheet.recognized) {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-zinc-100 px-2.5 py-0.5 text-xs text-zinc-400">
        未识别
      </span>
    );
  }
  if (sheet.errorCount > 0) {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-red-50 px-2.5 py-0.5 text-xs font-medium text-red-600 ring-1 ring-red-200">
        <span className="w-1.5 h-1.5 rounded-full bg-red-400" />
        有错误
      </span>
    );
  }
  if (sheet.inserted === 0) {
    return (
      <span className="inline-flex items-center gap-1 rounded-full bg-zinc-100 px-2.5 py-0.5 text-xs text-zinc-400">
        空 Sheet
      </span>
    );
  }
  return (
    <span className="inline-flex items-center gap-1 rounded-full bg-emerald-50 px-2.5 py-0.5 text-xs font-medium text-emerald-700 ring-1 ring-emerald-200">
      <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
      成功
    </span>
  );
}
