// src/pages/BankVoucherImport.tsx
//
// Bank 进项发票凭证导入页面
// 依赖：shadcn/ui, tailwind v4, @tauri-apps/api, src/bindings.ts

import { useCallback, useRef, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import {
  commands,
  type BankVoucherImportSummary,
  type BankVoucherImportError,
} from "@/bindings";
import { cn } from "@/lib/utils";

type UploadState =
  | { status: "idle" }
  | { status: "dragging" }
  | { status: "loading"; fileName: string }
  | {
      status: "success";
      fileName: string;
      summary: BankVoucherImportSummary;
    }
  | { status: "error"; fileName?: string; message: string };

export default function BankVoucherImport() {
  const [receivedDate, setReceivedDate] = useState("");
  const [preparedBy, setPreparedBy] = useState("");
  const [state, setState] = useState<UploadState>({ status: "idle" });
  const dropRef = useRef<HTMLDivElement>(null);

  const canImport = receivedDate.trim() !== "" && preparedBy.trim() !== "";

  const runImport = useCallback(
    async (filePath: string, fileName: string) => {
      if (!receivedDate.trim()) {
        setState({ status: "error", message: "请先选择收到日期" });
        return;
      }

      if (!preparedBy.trim()) {
        setState({ status: "error", message: "请先填写制表人" });
        return;
      }

      setState({ status: "loading", fileName });

      try {
        const result = await commands.importBankVoucherExcel({
          filePath,
          receivedDate,
          preparedBy,
        });

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
            message: result.error || "Bank 凭证导入失败",
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
    },
    [receivedDate, preparedBy],
  );

  const handlePickFile = useCallback(async () => {
    if (!canImport) {
      setState({ status: "error", message: "请先选择收到日期并填写制表人" });
      return;
    }

    const selected = await open({
      multiple: false,
      filters: [{ name: "Excel 文件", extensions: ["xlsx", "xls"] }],
    });

    if (!selected) return;

    const filePath = typeof selected === "string" ? selected : selected;
    const fileName = filePath.split(/[\\/]/).pop() ?? filePath;

    runImport(filePath, fileName);
  }, [canImport, runImport]);

  const handleDrop = useCallback(
    (e: React.DragEvent<HTMLDivElement>) => {
      e.preventDefault();
      e.stopPropagation();

      if (!canImport) {
        setState({ status: "error", message: "请先选择收到日期并填写制表人" });
        return;
      }

      const file = e.dataTransfer.files?.[0];

      if (!file) {
        setState({ status: "idle" });
        return;
      }

      const filePath = (file as File & { path?: string }).path ?? file.name;

      runImport(filePath, file.name);
    },
    [canImport, runImport],
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

  const handleReset = () => setState({ status: "idle" });

  const isDragging = state.status === "dragging";

  return (
    <div className="min-h-screen  flex flex-col items-center px-4 pt-12">
      <div className="mb-8 text-center">
        <h1 className="text-3xl font-semibold tracking-tight">
          Bank 进项发票凭证导入
        </h1>
        <p className="mt-2 text-sm text-muted-foreground">
          选择收到日期、填写制表人后，导入 Bank 提供的进项发票凭证 Excel
        </p>
      </div>

      <div className="w-full max-w-3xl rounded-2xl bg-card p-6 surface-card">
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
          <label className="flex flex-col gap-1.5">
            <span className="text-xs font-medium text-muted-foreground">
              收到日期
            </span>
            <input
              type="date"
              value={receivedDate}
              onChange={(e) => setReceivedDate(e.target.value)}
              className={cn(
                "h-10 rounded-lg border border-input bg-background px-3 text-sm",
                "outline-none transition-colors",
                "focus-visible:ring-2 focus-visible:ring-ring/40",
              )}
            />
          </label>

          <label className="flex flex-col gap-1.5">
            <span className="text-xs font-medium text-muted-foreground">
              制表人
            </span>
            <input
              value={preparedBy}
              onChange={(e) => setPreparedBy(e.target.value)}
              placeholder="请输入制表人"
              className={cn(
                "h-10 rounded-lg border border-input bg-background px-3 text-sm",
                "outline-none transition-colors",
                "focus-visible:ring-2 focus-visible:ring-ring/40",
              )}
            />
          </label>
        </div>

        {(state.status === "idle" || state.status === "dragging") && (
          <div
            ref={dropRef}
            onDrop={handleDrop}
            onDragOver={handleDragOver}
            onDragLeave={handleDragLeave}
            onClick={handlePickFile}
            className={cn(
              "mt-6 flex cursor-pointer flex-col items-center justify-center gap-4",
              "rounded-2xl border-2 border-dashed px-8 py-14 transition-all duration-200",
              !canImport && "cursor-not-allowed opacity-60",
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
                  "h-8 w-8",
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
                {isDragging ? "松开以开始导入" : "拖拽 Bank Excel 到此处"}
              </p>
              <p className="mt-1 text-sm text-zinc-400">或点击选择文件</p>
            </div>

            <button
              type="button"
              disabled={!canImport}
              onClick={(e) => {
                e.stopPropagation();
                handlePickFile();
              }}
              className={cn(
                "inline-flex items-center rounded-lg px-5 py-2.5",
                "text-sm font-medium transition-colors",
                "bg-zinc-900 text-white hover:bg-zinc-700 active:bg-zinc-800",
                "disabled:cursor-not-allowed disabled:opacity-50",
                "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-zinc-900",
              )}
            >
              选择 Bank 凭证文件
            </button>

            <p className="text-xs text-zinc-400">.xlsx · .xls</p>
          </div>
        )}

        {state.status === "loading" && (
          <div className="mt-6 flex flex-col items-center gap-5 rounded-2xl border border-zinc-200 bg-white px-8 py-14">
            <div className="relative h-12 w-12">
              <div className="absolute inset-0 rounded-full border-4 border-zinc-100" />
              <div className="absolute inset-0 animate-spin rounded-full border-4 border-t-blue-500" />
            </div>

            <div className="text-center">
              <p className="text-sm font-medium text-zinc-700">
                正在导入 Bank 凭证
              </p>
              <p className="mt-1 max-w-xs truncate text-xs text-zinc-400">
                {state.fileName}
              </p>
            </div>
          </div>
        )}

        {state.status === "error" && (
          <div className="mt-6 rounded-2xl border border-red-200 bg-red-50 px-6 py-5">
            <p className="text-sm font-semibold text-red-700">导入失败</p>
            <p className="mt-1 text-xs text-red-500">{state.message}</p>

            <button
              onClick={handleReset}
              className="mt-3 text-xs text-red-600 underline underline-offset-2 hover:text-red-800"
            >
              重新选择
            </button>
          </div>
        )}

        {state.status === "success" && (
          <ImportResult
            fileName={state.fileName}
            summary={state.summary}
            onReset={handleReset}
          />
        )}
      </div>
    </div>
  );
}

function ImportResult({
  fileName,
  summary,
  onReset,
}: {
  fileName: string;
  summary: BankVoucherImportSummary;
  onReset: () => void;
}) {
  return (
    <div className="mt-6 flex flex-col gap-5">
      <div className="rounded-2xl border border-zinc-200 bg-white p-6">
        <div className="flex items-start justify-between gap-4">
          <div>
            <div className="flex items-center gap-2">
              {summary.committed ? (
                <span className="inline-flex items-center gap-1.5 rounded-full bg-emerald-50 px-2.5 py-0.5 text-xs font-medium text-emerald-700 ring-1 ring-emerald-200">
                  <span className="h-1.5 w-1.5 rounded-full bg-emerald-500" />
                  已提交
                </span>
              ) : (
                <span className="inline-flex items-center gap-1.5 rounded-full bg-red-50 px-2.5 py-0.5 text-xs font-medium text-red-700 ring-1 ring-red-200">
                  <span className="h-1.5 w-1.5 rounded-full bg-red-500" />
                  已回滚
                </span>
              )}

              <span className="text-xs text-zinc-400">{fileName}</span>
            </div>

            {summary.rollbackReason && (
              <p className="mt-2 text-xs text-red-500">
                {summary.rollbackReason}
              </p>
            )}
          </div>

          <button
            onClick={onReset}
            className="text-xs text-zinc-400 transition-colors hover:text-zinc-700"
          >
            重新导入
          </button>
        </div>

        <div className="mt-5 grid grid-cols-2 gap-4 md:grid-cols-5">
          <StatCard label="成功写入" value={summary.inserted} />
          <StatCard label="未找到" value={summary.notFoundCount} />
          <StatCard label="税额不一致" value={summary.taxMismatchCount} />
          <StatCard label="重复匹配" value={summary.ambiguousCount} />
          <StatCard label="解析错误" value={summary.parseErrorCount} />
        </div>
      </div>

      {summary.errors.length > 0 && <ErrorTable errors={summary.errors} />}
    </div>
  );
}

function StatCard({ label, value }: { label: string; value: number }) {
  return (
    <div className="rounded-xl border border-zinc-100 bg-zinc-50 px-4 py-3">
      <p className="text-xs text-zinc-400">{label}</p>
      <p className="mt-1 text-2xl font-semibold tabular-nums text-zinc-900">
        {value.toLocaleString()}
      </p>
    </div>
  );
}

function ErrorTable({ errors }: { errors: BankVoucherImportError[] }) {
  return (
    <div className="overflow-hidden rounded-2xl border border-amber-200 bg-amber-50">
      <div className="border-b border-amber-100 px-6 py-4">
        <h2 className="text-sm font-semibold text-amber-800">错误详情</h2>
      </div>

      <div className="overflow-x-auto">
        <table className="w-full text-sm">
          <thead>
            <tr className="border-b border-amber-100 bg-amber-50/80">
              <Th>行</Th>
              <Th>类型</Th>
              <Th>发票号码</Th>
              <Th>凭证号码</Th>
              <Th>错误信息</Th>
            </tr>
          </thead>

          <tbody className="divide-y divide-amber-100">
            {errors.map((err, index) => (
              <tr
                key={`${err.row}-${index}`}
                className="transition-colors hover:bg-amber-50/60"
              >
                <td className="px-6 py-3 text-xs text-zinc-500">{err.row}</td>
                <td className="px-6 py-3 text-xs text-zinc-500">
                  {formatErrorKind(err.kind)}
                </td>
                <td className="px-6 py-3 font-mono text-xs text-zinc-700">
                  {err.invoiceNo || "—"}
                </td>
                <td className="px-6 py-3 font-mono text-xs text-zinc-700">
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
  );
}

function Th({ children }: { children: React.ReactNode }) {
  return (
    <th className="px-6 py-3 text-left text-xs font-medium uppercase tracking-wide text-zinc-500">
      {children}
    </th>
  );
}

function formatErrorKind(kind: BankVoucherImportError["kind"]) {
  switch (kind) {
    case "parse":
      return "解析错误";
    case "notFound":
      return "未找到";
    case "taxMismatch":
      return "税额不一致";
    case "ambiguous":
      return "重复匹配";
    default:
      return kind;
  }
}
