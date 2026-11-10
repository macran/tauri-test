// src/CertifiedImport.tsx
// 认证清单导入页面
// 依赖: shadcn/ui, tailwind v4, @tauri-apps/api, src/bindings.ts

import { useState, useCallback, useRef } from "react";
import {
  commands,
  type CertifiedImportSummary,
  type RowParseError,
} from "@/bindings";
import { open } from "@tauri-apps/plugin-dialog";
import { cn } from "@/lib/utils";
import {
  Table,
  TableHeader,
  TableBody,
  TableRow,
  TableHead,
  TableCell,
} from "@/components/ui/table";

// ─── 状态机类型 ───────────────────────────────────────────────
type UploadState =
  | { status: "idle" }
  | { status: "dragging" }
  | { status: "loading"; fileName: string }
  | { status: "success"; fileName: string; summary: CertifiedImportSummary }
  | { status: "error"; fileName: string; message: string };

// ─── 主组件 ──────────────────────────────────────────────────
export default function CertifiedImport() {
  const [state, setState] = useState<UploadState>({ status: "idle" });
  const dropRef = useRef<HTMLDivElement>(null);

  // ── 核心导入逻辑 ──────────────────────────────────────────
  const runImport = useCallback(async (filePath: string, fileName: string) => {
    setState({ status: "loading", fileName });
    try {
      const result = await commands.importCertifiedExcel({ filePath });
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

  // ── Drag & Drop ───────────────────────────────────────────
  const handleDrop = useCallback(
    (e: React.DragEvent<HTMLDivElement>) => {
      e.preventDefault();
      e.stopPropagation();
      const file = e.dataTransfer.files?.[0];
      if (!file) return setState({ status: "idle" });
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

  // ── 点击选择文件 ──────────────────────────────────────────
  const handlePickFile = useCallback(async () => {
    const selected = await open({
      multiple: false,
      filters: [{ name: "Excel 文件", extensions: ["xlsx", "xls"] }],
    });
    if (!selected) return;
    const filePath = typeof selected === "string" ? selected : selected;
    const fileName = filePath.split(/[\\\/]/).pop() ?? filePath;
    runImport(filePath, fileName);
  }, [runImport]);

  const handleReset = () => setState({ status: "idle" });

  const isDragging = state.status === "dragging";

  return (
    <div className="min-h-screen bg-zinc-50 flex flex-col items-center pt-16 px-4">
      {/* ── 页头 ── */}
      <div className="mb-10 text-center">
        <h1 className="text-3xl font-semibold tracking-tight text-zinc-900">
          认证清单导入
        </h1>
        <p className="mt-2 text-sm text-zinc-500">
          支持增值税抵扣认证清单 Excel 文件，导入前请确保对应发票已入库
        </p>
      </div>

      {/* ── Drop Zone ── */}
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
              {isDragging ? "松开以开始导入" : "拖拽认证清单文件到此处"}
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
            选择认证清单文件
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

      {/* ── 系统级错误（命令本身失败） ── */}
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

      {/* ── 成功：结果卡片 ── */}
      {state.status === "success" && (
        <CertifiedResultCard
          summary={state.summary}
          fileName={state.fileName}
          onReset={handleReset}
        />
      )}
    </div>
  );
}

// ─────────────────────────────────────────────────────────────
// 结果卡片
// ─────────────────────────────────────────────────────────────
function CertifiedResultCard({
  summary,
  fileName,
  onReset,
}: {
  summary: CertifiedImportSummary;
  fileName: string;
  onReset: () => void;
}) {
  const hasAnyError =
    summary.parseErrorCount > 0 ||
    summary.headerNotFoundCount > 0 ||
    summary.duplicateCount > 0;

  return (
    <div className="w-full max-w-2xl flex flex-col gap-4">
      {/* ── 顶部汇总卡片 ── */}
      <div className="rounded-2xl border border-zinc-200 bg-white p-6">
        <div className="flex items-start justify-between">
          <div>
            {/* 状态 badge */}
            <div className="flex items-center gap-2">
              {!summary.committed ? (
                <span className="inline-flex items-center gap-1.5 rounded-full bg-red-50 px-2.5 py-0.5 text-xs font-medium text-red-700 ring-1 ring-red-200">
                  <span className="w-1.5 h-1.5 rounded-full bg-red-500" />
                  已回滚
                </span>
              ) : hasAnyError ? (
                <span className="inline-flex items-center gap-1.5 rounded-full bg-amber-50 px-2.5 py-0.5 text-xs font-medium text-amber-700 ring-1 ring-amber-200">
                  <span className="w-1.5 h-1.5 rounded-full bg-amber-500" />
                  部分成功
                </span>
              ) : (
                <span className="inline-flex items-center gap-1.5 rounded-full bg-emerald-50 px-2.5 py-0.5 text-xs font-medium text-emerald-700 ring-1 ring-emerald-200">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
                  导入成功
                </span>
              )}
              <span className="text-xs text-zinc-400">
                税款所属期 {summary.taxPeriod}
              </span>
            </div>
            {/* 文件名 */}
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

        {/* 四格计数 */}
        <div className="mt-5 grid grid-cols-4 gap-3">
          <StatCard label="成功导入" value={summary.inserted} unit="条" />
          <StatCard
            label="解析失败"
            value={summary.parseErrorCount}
            unit="条"
            highlight={summary.parseErrorCount > 0}
            highlightColor="red"
          />
          <StatCard
            label="发票未入库"
            value={summary.headerNotFoundCount}
            unit="条"
            highlight={summary.headerNotFoundCount > 0}
            highlightColor="amber"
          />
          <StatCard label="重复跳过" value={summary.duplicateCount} unit="条" />
        </div>

        {/* 回滚原因 */}
        {summary.rollbackReason && (
          <div className="mt-4 rounded-lg bg-red-50 border border-red-100 px-4 py-3">
            <p className="text-xs text-red-600">
              <span className="font-medium">回滚原因：</span>
              {summary.rollbackReason}
            </p>
          </div>
        )}
      </div>

      {/* ── 解析错误明细 ── */}
      {summary.parseErrorCount > 0 && (
        <ErrorDetailTable
          title={`解析失败明细（共 ${summary.parseErrorCount} 条）`}
          color="red"
        >
          <thead>
            <tr className="bg-red-50/80 border-b border-red-100">
              <Th center>行</Th>
              <Th>列</Th>
              <Th>错误信息</Th>
            </tr>
          </thead>
          <tbody className="divide-y divide-red-100">
            {summary.parseErrors.map((err: RowParseError, i: number) => (
              <tr key={i} className="hover:bg-red-50/60 transition-colors">
                <td className="px-6 py-3 text-center text-xs text-zinc-500">
                  {err.row}
                </td>
                <td className="px-6 py-3 text-xs font-mono text-zinc-500">
                  {err.col}
                </td>
                <td className="px-6 py-3 text-xs text-red-600">
                  {err.message}
                </td>
              </tr>
            ))}
          </tbody>
        </ErrorDetailTable>
      )}

      {/* ── 发票未入库明细 ── */}
      {summary.headerNotFoundCount > 0 && (
        <div className="rounded-2xl border border-amber-200 bg-amber-50 overflow-hidden">
          <div className="px-6 py-4 border-b border-amber-100">
            <h2 className="text-sm font-semibold text-amber-800">
              发票未入库（共 {summary.headerNotFoundCount} 条）
            </h2>
          </div>
          <Table>
            <TableHeader>
              <TableRow className="bg-amber-50/80 border-amber-100 hover:bg-amber-50/80">
                <TableHead className="text-amber-700">发票号码</TableHead>
                <TableHead className="text-amber-700">开票日期</TableHead>
                <TableHead className="text-amber-700">
                  销售方纳税识别号
                </TableHead>
                <TableHead className="text-amber-700">
                  销售方纳税名称
                </TableHead>
              </TableRow>
            </TableHeader>
            <TableBody>
              {summary.headerNotFoundKeys.map((item, i) => (
                <TableRow
                  key={i}
                  className="border-amber-100 hover:bg-amber-50/60"
                >
                  <TableCell className="font-mono text-xs text-amber-800">
                    {item.invoiceNumber}
                  </TableCell>
                  <TableCell className="text-xs text-amber-800">
                    {item.issueDate ?? "—"}
                  </TableCell>
                  <TableCell className="font-mono text-xs text-amber-800">
                    {item.sellerTaxNo ?? "—"}
                  </TableCell>
                  <TableCell className="text-xs text-amber-800">
                    {item.sellerName ?? "—"}
                  </TableCell>
                </TableRow>
              ))}
            </TableBody>
          </Table>
        </div>
      )}
    </div>
  );
}

// ─────────────────────────────────────────────────────────────
// 子组件
// ─────────────────────────────────────────────────────────────

function StatCard({
  label,
  value,
  unit,
  highlight = false,
  highlightColor = "red",
}: {
  label: string;
  value: number;
  unit: string;
  highlight?: boolean;
  highlightColor?: "red" | "amber";
}) {
  const bgMap = {
    red: "bg-red-50 border-red-100",
    amber: "bg-amber-50 border-amber-100",
  };
  const textMap = { red: "text-red-600", amber: "text-amber-600" };

  return (
    <div className="card">
      <p className="text-xs text-zinc-400">{label}</p>
      <p
        className={cn(
          "mt-1 text-2xl font-semibold tabular-nums",
          highlight ? textMap[highlightColor] : "text-zinc-900",
        )}
      >
        {value.toLocaleString()}
        <span className="ml-1 text-sm font-normal text-zinc-400">{unit}</span>
      </p>
    </div>
  );
}

function ErrorDetailTable({
  title,
  color,
  children,
}: {
  title: string;
  color: "red" | "amber";
  children: React.ReactNode;
}) {
  const borderMap = { red: "border-red-200", amber: "border-amber-200" };
  const bgMap = { red: "bg-red-50", amber: "bg-amber-50" };
  const titleMap = { red: "text-red-800", amber: "text-amber-800" };
  const divMap = { red: "border-red-100", amber: "border-amber-100" };

  return (
    <div
      className={cn(
        "rounded-2xl border overflow-hidden",
        borderMap[color],
        bgMap[color],
      )}
    >
      <div className={cn("px-6 py-4 border-b", divMap[color])}>
        <h2 className={cn("text-sm font-semibold", titleMap[color])}>
          {title}
        </h2>
      </div>
      <div className="overflow-x-auto">
        <table className="w-full text-sm">{children}</table>
      </div>
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
