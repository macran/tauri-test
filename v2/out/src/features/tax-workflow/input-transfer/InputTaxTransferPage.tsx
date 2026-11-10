import { useState } from "react";
import { ittApi } from "../api";
import { RunsPanel } from "../components/RunsPanel";
import { cn } from "@/lib/utils";
import type { TaxRun } from "../types";
import { TransferConfirmedQuery } from "./TransferConfirmedQuery";
import { TransferReviewPanel } from "./TransferReviewPanel";

type Tab = "review" | "runs" | "query";

/**
 * 进项税转出
 *   匹配与复核：选认证期间 → 匹配 → 复核/修改 → 导出本批清单
 *   所属期批次：找到纳税所属期结果集 → 导出 → 回填会计凭证号/入账日期/做账人 → 确认锁定；管理员按所属期解锁
 *   已确认查询：任意纳税所属期的转出明细
 */
export default function InputTaxTransferPage() {
  const [tab, setTab] = useState<Tab>("review");
  const [openRun, setOpenRun] = useState<TaxRun | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 p-4">
      <header className="flex items-end justify-between gap-4">
        <h1 className="text-lg font-semibold">进项税转出</h1>
        <nav className="flex gap-1 border-b" role="tablist">
          {(
            [
              ["review", "匹配与复核"],
              ["runs", "纳税所属期批次"],
              ["query", "已确认明细查询"],
            ] as const
          ).map(([k, t]) => (
            <button
              key={k}
              role="tab"
              aria-selected={tab === k}
              onClick={() => setTab(k)}
              className={cn(
                "-mb-px border-b-2 px-3 py-1.5 text-sm",
                tab === k ? "border-primary font-medium" : "border-transparent text-muted-foreground hover:text-foreground",
              )}
            >
              {t}
            </button>
          ))}
        </nav>
      </header>

      <div className={cn("flex min-h-0 flex-1 flex-col", tab !== "review" && "hidden")}>
        <TransferReviewPanel openRun={openRun} onChanged={() => setRefreshKey((k) => k + 1)} />
      </div>
      {tab === "runs" && (
        <RunsPanel
          module="input_tax_transfer"
          moduleLabel="进项税转出"
          srcLabel="认证期间"
          taxLabel="转出税额"
          filePrefix="进项转出清单"
          refreshKey={refreshKey}
          loadStats={async (id) => (await ittApi.listLines(id)).stats}
          exportPending={ittApi.exportPending}
          exportConfirmedBatch={ittApi.exportConfirmedBatch}
          confirm={ittApi.confirm}
          onOpenRun={(r) => {
            setOpenRun({ ...r }); // 新对象，确保重复点击同一期间也会重新加载
            setTab("review");
          }}
        />
      )}
      {tab === "query" && <TransferConfirmedQuery />}
    </div>
  );
}
