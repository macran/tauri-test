import { useState } from "react";
import { Button } from "@/components/ui/button";
import { cn } from "@/lib/utils";
import { stApi } from "../api";
import { RunsPanel } from "../components/RunsPanel";
import type { TaxRun } from "../types";
import { StampCollectionPanel } from "./StampCollectionPanel";
import { StampConfirmedQuery } from "./StampConfirmedQuery";
import { StampReviewPanel } from "./StampReviewPanel";

type Tab = "review" | "runs" | "collection" | "query";

/**
 * 印花税
 *   匹配与复核：选凭证期间 → 匹配 → 复核 / 指定子目 → 导出本批清单
 *   所属期批次：回填会计凭证信息 → 确认锁定；管理员按所属期解锁
 *   信息采集表：按纳税所属期汇总，填税务局模版导出
 *   已确认查询
 */
export default function StampTaxPage() {
  const [tab, setTab] = useState<Tab>("review");
  const [openRun, setOpenRun] = useState<TaxRun | null>(null);
  const [collectionRun, setCollectionRun] = useState<number | null>(null);
  const [refreshKey, setRefreshKey] = useState(0);

  return (
    <div className="flex h-full min-h-0 flex-col gap-3 p-4">
      <header className="flex items-end justify-between gap-4">
        <h1 className="text-lg font-semibold">印花税</h1>
        <nav className="flex gap-1 border-b" role="tablist">
          {(
            [
              ["review", "匹配与复核"],
              ["runs", "纳税所属期批次"],
              ["collection", "信息采集表"],
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
        <StampReviewPanel openRun={openRun} onChanged={() => setRefreshKey((k) => k + 1)} />
      </div>
      {tab === "runs" && (
        <RunsPanel
          module="stamp_tax"
          moduleLabel="印花税"
          srcLabel="凭证期间"
          taxLabel="印花税额"
          filePrefix="印花税清单"
          refreshKey={refreshKey}
          loadStats={async (id) => (await stApi.listLines(id)).stats}
          exportPending={stApi.exportPending}
          exportConfirmedBatch={stApi.exportConfirmedBatch}
          confirm={stApi.confirm}
          onOpenRun={(r) => {
            setOpenRun({ ...r });
            setTab("review");
          }}
          extraActions={(r) =>
            r.filingPeriod ? (
              <Button
                size="sm"
                variant="outline"
                onClick={() => {
                  setCollectionRun(r.id);
                  setTab("collection");
                }}
              >
                采集表
              </Button>
            ) : null
          }
        />
      )}
      {tab === "collection" && <StampCollectionPanel refreshKey={refreshKey} presetRunId={collectionRun} />}
      {tab === "query" && <StampConfirmedQuery />}
    </div>
  );
}
