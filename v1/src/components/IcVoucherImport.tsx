// src/IcVoucherImport.tsx
// IC 认证清单导入页面
// 事务语义：全部成功提交 / 有错误全部回滚，不存在部分成功

import { useState, useCallback } from "react";
import { commands, type IcVoucherImportSummary } from "@/bindings";
import { open } from "@tauri-apps/plugin-dialog";
import { cn } from "@/lib/utils";

// ─── 状态机 ──────────────────────────────────────────────────
type UploadState =
  | { status: "idle" }
  | { status: "loading"; fileName: string }
  | { status: "success"; fileName: string; summary: IcVoucherImportSummary }
  | { status: "error"; fileName: string; message: string };

// ─── 主组件 ──────────────────────────────────────────────────
export default function IcVoucherImport() {
  const [receivedDate, setReceivedDate] = useState("");
  const [preparer, setPreparer] = useState("");
  const [state, setState] = useState<UploadState>({ status: "idle" });

  const canPickFile = receivedDate.trim() !== "" && preparer.trim() !== "";

  const runImport = useCallback(
    async (filePath: string, fileName: string) => {
      setState({ status: "loading", fileName });
      try {
        const result = await commands.importIcVoucherExcel({
          filePath,
          receivedDate: receivedDate.trim(),
          preparer: preparer.trim(),
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

  // summary 拿到后，判断是真正成功还是回滚
  const isCommitted = (s: IcVoucherImportSummary) => s.committed;

  return (
    <div className="min-h-screen bg-zinc-50 flex flex-col items-center pt-16 px-4">
      {/* ── 页头 ── */}
      <div className="mb-10 text-center">
        <h1 className="text-3xl font-semibold tracking-tight text-zinc-900">
          IC 认证清单导入
        </h1>
        <p className="mt-2 text-sm text-zinc-500">
          请先填写接收日期和制表人，再选择抵扣认证清单 Excel 文件
        </p>
      </div>

      {/* ── 表单区域 ── */}
      <div className="w-full max-w-xl rounded-2xl border border-zinc-200 bg-white p-6 mb-6">
        <div className="grid grid-cols-2 gap-4">
          <div className="flex flex-col gap-1">
            <label className="text-xs font-medium text-zinc-600">
              接收日期
            </label>
            <input
              type="date"
              value={receivedDate}
              onChange={(e) => setReceivedDate(e.target.value)}
              className="rounded-lg border border-zinc-200 bg-white px-3 py-2 text-sm text-zinc-800 focus:outline-none focus:ring-2 focus:ring-zinc-900"
            />
          </div>
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
              {canPickFile ? "点击选择认证清单文件" : "请先填写上方信息"}
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

      {/* ── 系统级错误（command 本身抛异常）── */}
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
            <p className="text-sm font-semibold text-red-700">系统错误</p>
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

      {/* ── 结果区域（committed 决定显示哪种）── */}
      {state.status === "success" && (
        <div className="w-full max-w-3xl flex flex-col gap-6">
          {/* ══ 分支 A：全部提交成功 ══ */}
          {isCommitted(state.summary) ? (
            <div className="rounded-2xl border border-emerald-200 bg-white p-6">
              <div className="flex items-start justify-between">
                <div className="flex items-center gap-2">
                  <span className="inline-flex items-center gap-1.5 rounded-full bg-emerald-50 px-2.5 py-0.5 text-xs font-medium text-emerald-700 ring-1 ring-emerald-200">
                    <span className="w-1.5 h-1.5 rounded-full bg-emerald-500" />
                    导入成功
                  </span>
                </div>
                <button
                  onClick={handleReset}
                  className="text-xs text-zinc-400 hover:text-zinc-700 transition-colors"
                >
                  重新导入
                </button>
              </div>

              {/* 成功只展示一个大数字 */}
              <div className="mt-6 flex items-end gap-2">
                <span className="text-5xl font-bold tabular-nums text-emerald-600">
                  {state.summary.inserted.toLocaleString()}
                </span>
                <span className="mb-1 text-sm text-zinc-400">
                  条记录已写入数据库
                </span>
              </div>

              <p className="mt-3 text-xs text-zinc-400">
                接收日期：{receivedDate}　制表人：{preparer}
              </p>
            </div>
          ) : (
            /* ══ 分支 B：有错误，全部回滚 ══ */
            <div className="w-full max-w-3xl flex flex-col gap-6">
              {/* 回滚提示卡片 */}
              <div className="rounded-2xl border border-red-200 bg-white p-6">
                <div className="flex items-start justify-between">
                  <div className="flex items-center gap-2">
                    <span className="inline-flex items-center gap-1.5 rounded-full bg-red-50 px-2.5 py-0.5 text-xs font-medium text-red-700 ring-1 ring-red-200">
                      <span className="w-1.5 h-1.5 rounded-full bg-red-500" />
                      已回滚
                    </span>
                  </div>
                  <button
                    onClick={handleReset}
                    className="text-xs text-zinc-400 hover:text-zinc-700 transition-colors"
                  >
                    重新导入
                  </button>
                </div>

                <p className="mt-4 text-sm text-red-600">
                  导入过程中发现{" "}
                  <span className="font-semibold">
                    {state.summary.errorCount}
                  </span>{" "}
                  条错误，所有数据已回滚，数据库未写入任何记录。
                  请修正下方错误后重新导入。
                </p>

                {/* 前置检查失败原因（如文件格式错误等） */}
                {state.summary.rollbackReason && (
                  <div className="mt-3 rounded-lg bg-red-50 border border-red-100 px-4 py-3">
                    <p className="text-xs text-red-600">
                      <span className="font-medium">失败原因：</span>
                      {state.summary.rollbackReason}
                    </p>
                  </div>
                )}

                {/* 错误类型汇总 */}
                {state.summary.errors.length > 0 && (
                  <div className="mt-4 flex flex-wrap gap-2">
                    {Object.entries(
                      state.summary.errors.reduce<Record<string, number>>(
                        (acc, e) => {
                          acc[e.errorKind] = (acc[e.errorKind] ?? 0) + 1;
                          return acc;
                        },
                        {},
                      ),
                    ).map(([kind, count]) => (
                      <span
                        key={kind}
                        className={cn(
                          "inline-flex items-center gap-1 rounded-full px-2.5 py-0.5 text-xs font-medium",
                          ERROR_KIND_COLORS[kind] ??
                            "bg-zinc-100 text-zinc-600",
                        )}
                      >
                        {kind}
                        <span className="ml-0.5 font-bold">{count}</span>
                      </span>
                    ))}
                  </div>
                )}
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
                          <Th>LE</Th>
                          <Th>发票号码</Th>
                          <Th>Document No.</Th>
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
                            <td className="px-4 py-3 text-xs text-zinc-600">
                              {err.legalEntity || "—"}
                            </td>
                            <td className="px-4 py-3 text-xs font-mono text-zinc-600">
                              {err.invNum || "—"}
                            </td>
                            <td className="px-4 py-3 text-xs font-mono text-zinc-600">
                              {err.documentNo || "—"}
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
      )}
    </div>
  );
}

// ─── 子组件 ──────────────────────────────────────────────────

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
  解析错误: "bg-zinc-100 text-zinc-600",
  发票未找到: "bg-red-100 text-red-700",
  税额不一致: "bg-amber-100 text-amber-700",
  价税合计不一致: "bg-orange-100 text-orange-700",
  重复数据: "bg-blue-100 text-blue-700",
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
