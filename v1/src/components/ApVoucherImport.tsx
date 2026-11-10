// src/ApVoucherImport.tsx
// AP 应付凭证导入页面
// 用户先填写接收日期和制表人，再选择文件

import { useState, useCallback } from "react";
import { commands, type ApVoucherImportSummary } from "@/bindings";
import { open } from "@tauri-apps/plugin-dialog";
import { cn } from "@/lib/utils";

// ─── 日期选择器（使用你已有的 shadcn DatePicker）────────────
// 这里用 input[type=date] 作为占位，替换为你的 shadcn 组件即可
function DateInput({
  value,
  onChange,
  label,
}: {
  value: string;
  onChange: (v: string) => void;
  label: string;
}) {
  return (
    <div className="flex flex-col gap-1">
      <label className="text-xs font-medium text-zinc-600">{label}</label>
      <input
        type="date"
        value={value}
        onChange={(e) => onChange(e.target.value)}
        className="rounded-lg border border-zinc-200 bg-white px-3 py-2 text-sm text-zinc-800 focus:outline-none focus:ring-2 focus:ring-zinc-900"
      />
    </div>
  );
}

// ─── 状态机 ──────────────────────────────────────────────────
type UploadState =
  | { status: "idle" }
  | { status: "loading"; fileName: string }
  | { status: "success"; fileName: string; summary: ApVoucherImportSummary }
  | { status: "error"; fileName: string; message: string };

// ─── 主组件 ──────────────────────────────────────────────────
export default function ApVoucherImport() {
  const [receivedDate, setReceivedDate] = useState("");
  const [preparer, setPreparer] = useState("");
  const [state, setState] = useState<UploadState>({ status: "idle" });

  const canPickFile = receivedDate.trim() !== "" && preparer.trim() !== "";

  const runImport = useCallback(
    async (filePath: string, fileName: string) => {
      setState({ status: "loading", fileName });
      try {
        const result = await commands.importApVoucherExcel({
          filePath,
          receivedDate,
          preparer,
        });

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
    },
    [receivedDate, preparer],
  );

  const handlePickFile = useCallback(async () => {
    if (!canPickFile) return;
    const selected = await open({
      multiple: false,
      filters: [{ name: "Excel 文件", extensions: ["xlsx", "xls"] }],
    });
    if (!selected) return;
    const filePath = typeof selected === "string" ? selected : selected;
    const fileName = filePath.split(/[\\\/]/).pop() ?? filePath;
    runImport(filePath, fileName);
  }, [canPickFile, runImport]);

  const handleReset = () => setState({ status: "idle" });

  return (
    <div className="min-h-screen bg-zinc-50 flex flex-col items-center pt-16 px-4">
      {/* ── 页头 ── */}
      <div className="mb-10 text-center">
        <h1 className="text-3xl font-semibold tracking-tight text-zinc-900">
          AP 应付凭证导入
        </h1>
        <p className="mt-2 text-sm text-zinc-500">
          请先填写接收日期和制表人，再选择 AP 凭证 Excel 文件
        </p>
      </div>

      {/* ── 表单区域 ── */}
      <div className="w-full max-w-xl rounded-2xl border border-zinc-200 bg-white p-6 mb-6">
        <div className="grid grid-cols-2 gap-4">
          <DateInput
            label="接收日期"
            value={receivedDate}
            onChange={setReceivedDate}
          />
          <div className="flex flex-col gap-1">
            <label className="text-xs font-medium text-zinc-600">制表人</label>
            <input
              type="text"
              value={preparer}
              onChange={(e) => setPreparer(e.target.value)}
              placeholder="请输入制表人姓名"
              className="rounded-lg border border-zinc-200 bg-white px-3 py-2 text-sm text-zinc-800 focus:outline-none focus:ring-2 focus:ring-zinc-900"
            />
          </div>
        </div>
        {!canPickFile && (
          <p className="mt-3 text-xs text-amber-600">
            请先填写接收日期和制表人，才能选择文件
          </p>
        )}
      </div>

      {/* ── 文件选择区域（idle 状态）── */}
      {state.status === "idle" && (
        <div
          onClick={canPickFile ? handlePickFile : undefined}
          className={cn(
            "w-full max-w-xl rounded-2xl border-2 border-dashed transition-all duration-200",
            "flex flex-col items-center justify-center gap-4 py-16 px-8",
            canPickFile
              ? "border-zinc-300 bg-white hover:border-zinc-400 hover:bg-zinc-50 cursor-pointer"
              : "border-zinc-200 bg-zinc-50 cursor-not-allowed opacity-50",
          )}
        >
          <div className="rounded-full bg-zinc-100 p-4">
            <svg
              className="w-8 h-8 text-zinc-400"
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
            <p className="text-base font-medium text-zinc-700">
              {canPickFile ? "点击选择 AP 凭证文件" : "请先填写上方信息"}
            </p>
            <p className="mt-1 text-xs text-zinc-400">.xlsx · .xls</p>
          </div>
        </div>
      )}

      {/* ── 加载中 ── */}
      {state.status === "loading" && (
        <div className="w-full max-w-xl rounded-2xl border border-zinc-200 bg-white px-8 py-14 flex flex-col items-center gap-5">
          <div className="relative w-12 h-12">
            <div className="absolute inset-0 rounded-full border-4 border-zinc-100" />
            <div className="absolute inset-0 rounded-full border-4 border-t-blue-500 animate-spin" />
          </div>
          <p className="text-sm font-medium text-zinc-700">正在导入...</p>
          <p className="text-xs text-zinc-400 truncate max-w-xs">
            {state.fileName}
          </p>
        </div>
      )}

      {/* ── 错误 ── */}
      {state.status === "error" && (
        <div className="w-full max-w-xl rounded-2xl border border-red-200 bg-red-50 px-8 py-10 flex flex-col items-center gap-4">
          <p className="text-sm font-semibold text-red-700">导入失败</p>
          <p className="text-xs text-red-500">{state.message}</p>
          <button
            onClick={handleReset}
            className="text-xs text-red-600 underline underline-offset-2 hover:text-red-800"
          >
            重新选择文件
          </button>
        </div>
      )}

      {/* ── 成功：结果展示 ── */}
      {state.status === "success" && (
        <div className="w-full max-w-3xl flex flex-col gap-6">
          {/* 汇总卡片 */}
          <div className="rounded-2xl border border-zinc-200 bg-white p-6">
            <div className="flex items-start justify-between">
              <div className="flex items-center gap-2">
                <span className="inline-flex items-center gap-1.5 rounded-full bg-emerald-50 px-2.5 py-0.5 text-xs font-medium text-emerald-700 ring-1 ring-emerald-200">
                  <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
                  导入完成
                </span>
              </div>
              <button
                onClick={handleReset}
                className="text-xs text-zinc-400 hover:text-zinc-700 transition-colors"
              >
                重新导入
              </button>
            </div>

            {/* 统计数字 */}
            <div className="mt-5 grid grid-cols-2 gap-4 sm:grid-cols-3">
              <StatCard
                label="新插入"
                value={state.summary.inserted}
                unit="条"
              />
              <StatCard
                label="凭证号已更新（红冲重做）"
                value={state.summary.updated}
                unit="条"
              />
              <StatCard
                label="完全重复已忽略"
                value={state.summary.skipped}
                unit="条"
              />
              <StatCard
                label="错误条数"
                value={state.summary.errorCount}
                unit="条"
                color="red"
              />
              <StatCard
                label="新增供应商"
                value={state.summary.vendorNewCount}
                unit="家"
              />
              <StatCard
                label="已有供应商"
                value={state.summary.vendorExistingCount}
                unit="家"
              />
            </div>
          </div>

          {/* 错误详情表格 */}
          {state.summary.errors.length > 0 && (
            <div className="rounded-2xl border border-amber-200 bg-amber-50 overflow-hidden">
              <div className="px-6 py-4 border-b border-amber-100">
                <h2 className="text-sm font-semibold text-amber-800">
                  错误详情（共 {state.summary.errorCount} 条）
                </h2>
              </div>
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="bg-amber-50/80 border-b border-amber-100">
                      <Th center>行号</Th>
                      <Th>Barcode</Th>
                      <Th>发票号码</Th>
                      <Th>错误类型</Th>
                      <Th>错误详情</Th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-amber-100">
                    {state.summary.errors.map((err, i) => (
                      <tr
                        key={i}
                        className="hover:bg-amber-50/60 transition-colors"
                      >
                        <td className="px-4 py-3 text-center text-xs text-zinc-500">
                          {err.row}
                        </td>
                        <td className="px-4 py-3 text-xs font-mono text-zinc-600">
                          {err.barcode ?? "—"}
                        </td>
                        <td className="px-4 py-3 text-xs font-mono text-zinc-600">
                          {err.invNum ?? "—"}
                        </td>
                        <td className="px-4 py-3">
                          <ErrorKindBadge kind={err.errorKind} />
                        </td>
                        <td className="px-4 py-3 text-xs text-red-600">
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
      )}
    </div>
  );
}

// ─── 子组件 ──────────────────────────────────────────────────

function StatCard({
  label,
  value,
  unit,
  color = "default",
}: {
  label: string;
  value: number;
  unit: string;
  color?: "default" | "red";
}) {
  return (
    <div className="rounded-xl bg-zinc-50 border border-zinc-100 px-4 py-3">
      <p className="text-xs text-zinc-400">{label}</p>
      <p
        className={cn(
          "mt-1 text-2xl font-semibold tabular-nums",
          color === "red" && value > 0 ? "text-red-600" : "text-zinc-900",
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
        "px-4 py-3 text-xs font-medium text-zinc-500 uppercase tracking-wide",
        center ? "text-center" : "text-left",
      )}
    >
      {children}
    </th>
  );
}

const ERROR_KIND_COLORS: Record<string, string> = {
  必填字段为空: "bg-zinc-100 text-zinc-600",
  金额解析失败: "bg-orange-100 text-orange-700",
  发票类型非法: "bg-purple-100 text-purple-700",
  税务认证非法: "bg-blue-100 text-blue-700",
  发票匹配失败: "bg-red-100 text-red-700",
  金额核对失败: "bg-rose-100 text-rose-700",
  发票类型核对失败: "bg-amber-100 text-amber-700",
};

function ErrorKindBadge({ kind }: { kind: string }) {
  const cls = ERROR_KIND_COLORS[kind] ?? "bg-zinc-100 text-zinc-600";
  return (
    <span
      className={cn(
        "inline-flex rounded-full px-2 py-0.5 text-xs font-medium",
        cls,
      )}
    >
      {kind}
    </span>
  );
}
