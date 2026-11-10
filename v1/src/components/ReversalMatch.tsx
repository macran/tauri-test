// src/ReversalMatch.tsx
// 红冲核对页面
// 依赖: shadcn/ui, tailwind v4, @tauri-apps/api, src/bindings.ts

import { useState, useCallback } from "react";
import {
  commands,
  type ReversalMatchSummary,
  type ReversalAbnormalItem,
} from "@/bindings";
import { cn } from "@/lib/utils";

// ─── 状态机 ────────────────────────────────────────────────────────────
type MatchState =
  | { status: "idle" }
  | { status: "loading" }
  | { status: "success"; summary: ReversalMatchSummary }
  | { status: "error"; message: string };

// ─── 主组件 ────────────────────────────────────────────────────────────
export default function ReversalMatch() {
  const [state, setState] = useState<MatchState>({ status: "idle" });
  const [startDate, setStartDate] = useState<string>("");
  const [endDate, setEndDate] = useState<string>("");

  const isRangeValid =
    startDate.trim() !== "" &&
    endDate.trim() !== "" &&
    startDate <= endDate;

  const runMatch = useCallback(async () => {
    if (!isRangeValid) return;
    setState({ status: "loading" });
    try {
      const result = await commands.matchReversalInvoices(
        startDate,
        endDate,
      );
      if (result.status === "ok") {
        setState({ status: "success", summary: result.data });
      } else {
        setState({ status: "error", message: result.error ?? "核对失败" });
      }
    } catch (err: unknown) {
      setState({
        status: "error",
        message:
          typeof err === "string"
            ? err
            : ((err as Error)?.message ?? "未知错误"),
      });
    }
  }, [startDate, endDate, isRangeValid]);

  const handleReset = () => {
    setState({ status: "idle" });
  };

  return (
    <div className="min-h-screen bg-zinc-50 flex flex-col items-center pt-16 px-4">
      {/* ── 页头 ── */}
      <div className="mb-10 text-center">
        <h1 className="text-3xl font-semibold tracking-tight text-zinc-900">
          红冲核对
        </h1>
        <p className="mt-2 text-sm text-zinc-500">
          选择红字发票的开票日期区间（可跨月、跨年），对区间内尚未核对成功的红字发票执行明细级红冲匹配核对；
          此前因“找不到蓝字发票”而挂起的红票，只要蓝字发票已补录入库，重新选择覆盖该红票日期的区间即可自动重新匹配
        </p>
      </div>

      {/* ── 输入区 ── */}
      {(state.status === "idle" || state.status === "error") && (
        <div className="w-full max-w-md rounded-2xl border border-zinc-200 bg-white p-8 flex flex-col gap-5">
          <div className="flex gap-4">
            <div className="flex flex-1 flex-col gap-2">
              <label className="text-sm font-medium text-zinc-700">
                开始日期
              </label>
              <input
                type="date"
                value={startDate}
                onChange={(e) => setStartDate(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && runMatch()}
                className={cn(
                  "rounded-lg border px-4 py-2.5 text-sm text-zinc-800",
                  "focus:outline-none focus:ring-2 focus:ring-zinc-900 focus:border-transparent",
                  "transition-colors",
                  state.status === "error"
                    ? "border-red-300 bg-red-50"
                    : "border-zinc-200 bg-zinc-50",
                )}
              />
            </div>
            <div className="flex flex-1 flex-col gap-2">
              <label className="text-sm font-medium text-zinc-700">
                结束日期
              </label>
              <input
                type="date"
                value={endDate}
                onChange={(e) => setEndDate(e.target.value)}
                onKeyDown={(e) => e.key === "Enter" && runMatch()}
                className={cn(
                  "rounded-lg border px-4 py-2.5 text-sm text-zinc-800",
                  "focus:outline-none focus:ring-2 focus:ring-zinc-900 focus:border-transparent",
                  "transition-colors",
                  state.status === "error"
                    ? "border-red-300 bg-red-50"
                    : "border-zinc-200 bg-zinc-50",
                )}
              />
            </div>
          </div>
          {startDate && endDate && startDate > endDate && (
            <p className="text-xs text-red-500">开始日期不能晚于结束日期</p>
          )}
          {state.status === "error" && (
            <p className="text-xs text-red-500">{state.message}</p>
          )}

          <button
            type="button"
            onClick={runMatch}
            disabled={!isRangeValid}
            className={cn(
              "rounded-lg px-5 py-2.5 text-sm font-medium transition-colors",
              "bg-zinc-900 text-white hover:bg-zinc-700 active:bg-zinc-800",
              "disabled:opacity-40 disabled:cursor-not-allowed",
              "focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-zinc-900",
            )}
          >
            开始核对
          </button>
        </div>
      )}

      {/* ── 加载中 ── */}
      {state.status === "loading" && (
        <div className="w-full max-w-md rounded-2xl border border-zinc-200 bg-white px-8 py-14 flex flex-col items-center gap-5">
          <div className="relative w-12 h-12">
            <div className="absolute inset-0 rounded-full border-4 border-zinc-100" />
            <div className="absolute inset-0 rounded-full border-4 border-t-blue-500 animate-spin" />
          </div>
          <p className="text-sm font-medium text-zinc-700">正在执行红冲核对…</p>
        </div>
      )}

      {/* ── 成功结果 ── */}
      {state.status === "success" && (
        <div className="w-full m-2 flex flex-col gap-6">
          {/* 本次核对编号 + 重新核对按鈕 */}
          <div className="flex items-center justify-between">
            <span className="text-xs text-zinc-400">
              本次核对编号 #{state.summary.matchRunNo}
            </span>
            <button
              onClick={handleReset}
              className="text-xs text-zinc-400 hover:text-zinc-700 transition-colors"
            >
              重新核对
            </button>
          </div>

          {/* ── 汇总卡片区 ── */}
          <div className="grid grid-cols-3 gap-4">
            <SummaryCard
              label="成功匹配"
              value={state.summary.okCount}
              color="emerald"
            />
            <SummaryCard
              label="无蓝字发票"
              value={state.summary.blueMissingCount}
              color="amber"
            />
            <SummaryCard
              label="红字发票异常"
              value={state.summary.abnormalCount}
              color="red"
            />
          </div>

          {/* ── 异常列表 ── */}
          {state.summary.abnormalItems.length > 0 && (
            <div className="rounded-2xl border border-zinc-200 bg-white overflow-hidden">
              <div className="px-6 py-4 border-b border-zinc-100">
                <h2 className="text-sm font-semibold text-zinc-800">
                  异常明细
                  <span className="ml-2 text-xs font-normal text-zinc-400">
                    共 {state.summary.abnormalItems.length} 条
                  </span>
                </h2>
              </div>
              <div className="overflow-x-auto">
                <table className="w-full text-sm">
                  <thead>
                    <tr className="bg-zinc-50 border-b border-zinc-100">
                      <Th>红票号码</Th>
                      <Th center>行号</Th>
                      <Th center>类型</Th>
                      <Th>蓝票号码</Th>
                      <Th>开票日期</Th>
                      <Th>销售方</Th>
                      <Th>备注</Th>
                    </tr>
                  </thead>
                  <tbody className="divide-y divide-zinc-100">
                    {state.summary.abnormalItems.map((item, i) => (
                      <AbnormalRow key={i} item={item} />
                    ))}
                  </tbody>
                </table>
              </div>
            </div>
          )}

          {/* 全部正常时的提示 */}
          {state.summary.abnormalItems.length === 0 && (
            <div className="rounded-2xl border border-emerald-200 bg-emerald-50 px-8 py-10 flex flex-col items-center gap-3">
              <div className="rounded-full bg-emerald-100 p-3">
                <svg
                  className="w-6 h-6 text-emerald-600"
                  fill="none"
                  viewBox="0 0 24 24"
                  stroke="currentColor"
                  strokeWidth={2}
                >
                  <path
                    strokeLinecap="round"
                    strokeLinejoin="round"
                    d="M4.5 12.75l6 6 9-13.5"
                  />
                </svg>
              </div>
              <p className="text-sm font-medium text-emerald-700">
                本批次所有红冲发票均核对成功
              </p>
            </div>
          )}
        </div>
      )}
    </div>
  );
}

// ─── 子组件 ────────────────────────────────────────────────────────────

type CardColor = "emerald" | "amber" | "red";

function SummaryCard({
  label,
  value,
  color,
}: {
  label: string;
  value: number;
  color: CardColor;
}) {
  const styles: Record<CardColor, string> = {
    emerald: "bg-emerald-50 border-emerald-100 text-emerald-700",
    amber: "bg-amber-50 border-amber-100 text-amber-700",
    red: "bg-red-50 border-red-100 text-red-700",
  };
  const numStyles: Record<CardColor, string> = {
    emerald: "text-emerald-900",
    amber: "text-amber-900",
    red: "text-red-900",
  };
  return (
    <div className={cn("rounded-xl border px-5 py-4", styles[color])}>
      <p className="text-xs font-medium opacity-70">{label}</p>
      <p
        className={cn(
          "mt-1 text-3xl font-semibold tabular-nums",
          numStyles[color],
        )}
      >
        {value.toLocaleString()}
        <span className="ml-1 text-sm font-normal opacity-60">条</span>
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
        "px-5 py-3 text-xs font-medium text-zinc-500 uppercase tracking-wide whitespace-nowrap",
        center ? "text-center" : "text-left",
      )}
    >
      {children}
    </th>
  );
}

function AbnormalRow({ item }: { item: ReversalAbnormalItem }) {
  const isBlueMissing = item.matchResultType === "blueMissing";
  const isAbnormal = item.matchResultType === "abnormal";

  return (
    <tr className="hover:bg-zinc-50/80 transition-colors">
      {/* 红票号码 */}
      <td className="px-5 py-3.5">
        <span className="font-mono text-xs text-zinc-700">
          {item.redInvUnqKey}
        </span>
      </td>

      {/* 行号 */}
      <td className="px-5 py-3.5 text-center">
        <span className="text-xs text-zinc-500">{item.redLineNo}</span>
      </td>

      {/* 类型 badge */}
      <td className="px-5 py-3.5 text-center">
        {isBlueMissing ? (
          <span className="inline-flex items-center gap-1 rounded-full bg-amber-50 px-2.5 py-0.5 text-xs font-medium text-amber-700 ring-1 ring-amber-200">
            <span className="w-1.5 h-1.5 rounded-full bg-amber-400" />
            无蓝字发票
          </span>
        ) : isAbnormal ? (
          <span className="inline-flex items-center gap-1 rounded-full bg-red-50 px-2.5 py-0.5 text-xs font-medium text-red-700 ring-1 ring-red-200">
            <span className="w-1.5 h-1.5 rounded-full bg-red-400" />
            红字发票异常
          </span>
        ) : null}
      </td>

      {/* 蓝票号码 */}
      <td className="px-5 py-3.5">
        {item.blueInvUnqKey ? (
          <span className="font-mono text-xs text-zinc-700">
            {item.blueInvUnqKey}
          </span>
        ) : (
          <span className="text-xs text-zinc-300">—</span>
        )}
      </td>

      {/* 开票日期 */}
      <td className="px-5 py-3.5">
        {item.blueIssueDate ? (
          <span className="text-xs text-zinc-500">
            {item.blueIssueDate.slice(0, 10)}
          </span>
        ) : (
          <span className="text-xs text-zinc-300">—</span>
        )}
      </td>

      {/* 销售方 */}
      <td className="px-5 py-3.5">
        <div className="flex flex-col">
          {item.blueSellerName && (
            <span className="text-xs font-medium text-zinc-700">
              {item.blueSellerName}
            </span>
          )}
          {item.blueSellerTaxNo && (
            <span className="text-xs text-zinc-400 font-mono">
              {item.blueSellerTaxNo}
            </span>
          )}
          {!item.blueSellerName && !item.blueSellerTaxNo && (
            <span className="text-xs text-zinc-300">—</span>
          )}
        </div>
      </td>

      {/* 备注 */}
      <td className="px-5 py-3.5 max-w-xs">
        {item.matchNote ? (
          <span className="text-xs text-zinc-500 break-words">
            {item.matchNote}
          </span>
        ) : (
          <span className="text-xs text-zinc-300">—</span>
        )}
      </td>
    </tr>
  );
}
