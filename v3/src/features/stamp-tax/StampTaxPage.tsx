import { useState } from "react";

import { commands, type StampTaxMatchRunResult, type StampTaxReviewSummary } from "@/bindings";
import {
  AlertDialog,
  AlertDialogAction,
  AlertDialogCancel,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from "@/components/ui/alert-dialog";
import { Badge } from "@/components/ui/badge";
import { Card, CardContent } from "@/components/ui/card";
import { toast } from "@/components/ui/toast";
import { PeriodRangePicker } from "@/features/input-tax-transfer/components/PeriodRangePicker";
import { useTableLayouts } from "@/features/query/table-layout";
import { PeriodStatusBar, usePeriodStatus } from "@/features/tax-period/PeriodStatusBar";
import { Field, LayoutField, TaxPeriodForm } from "@/features/tax-period/TaxPeriodForm";
import { parseResetRequired, useRunMatch } from "@/features/tax-period/useRunMatch";

import { StampTaxReviewTable } from "./components/StampTaxReviewTable";
import { ExportButton } from "@/features/query/ExportButton";

import { CollectionFormButton } from "./CollectionFormButton";
import { STAMP_EXPORT } from "./exportColumns";
import { fenToYuan, getErrorMessage } from "./utils";

export default function StampTaxPage() {
  const layouts = useTableLayouts("stamp-tax-review");

  const m = useRunMatch<StampTaxReviewSummary, StampTaxMatchRunResult>({
    load: (f) => commands.loadStampTaxReview({ filingFrom: f.from, filingTo: f.to }),
    match: (req) => commands.runStampTaxMatch(req),
    reviewOf: (r) => r.review,
  });
  const { review, effectiveFiling } = m;

  // 状态栏跟随当前结果所属的纳税所属期
  const filingFrom = review?.filingFrom ?? effectiveFiling.from;
  const filingTo = review?.filingTo ?? effectiveFiling.to;
  const { status, setStatus, refresh } = usePeriodStatus("stamp_tax", filingFrom, filingTo);
  // 已确认 / 已锁定的批次只读：后端会拒绝匹配，这里一开始就禁用并写明原因，不让用户点了才报错
  const readOnlyBatch = status?.state === "confirmed" || status?.state === "locked";
  const readOnlyReason = readOnlyBatch
    ? status.state === "locked"
      ? "批次已锁定，只读"
      : "批次已确认，只读；取消确认后可重新匹配"
    : undefined;

  /** 更改批次 1 的凭证期间会清空已有结果和人工修改：用户同意后带 reset 重试 */
  const [resetAsk, setResetAsk] = useState<{ count: number } | null>(null);

  async function reload() {
    const result = await commands.loadStampTaxReview({ filingFrom, filingTo });
    if (result.status === "ok") m.setReview(result.data);
  }

  async function handleRun(reset = false) {
    const toastId = toast.add({ type: "loading", title: "正在匹配……" });
    try {
      const result = await m.run(reset);
      if (!result) {
        toast.close(toastId);
        return;
      }
      if (result.status !== "ok") {
        const ask = parseResetRequired(result.error);
        if (ask) {
          toast.close(toastId);
          setResetAsk({ count: ask.count });
          return;
        }
        throw new Error(getErrorMessage(result.error));
      }
      void refresh();
      const d = result.data;
      toast.update(toastId, {
        type: "success",
        title: "匹配完成",
        description: [
          `已做账参与匹配 ${d.qualifiedItemCount} 条`,
          `命中 ${d.matchedItemCount} 条`,
          `保留人工结论 ${d.preservedOverrideCount} 条`,
        ].join("，"),
      });
    } catch (error) {
      toast.update(toastId, { type: "error", title: "匹配失败", description: getErrorMessage(error) });
    }
  }

  // 需人工核对只提示；待选类别算不出税，必须先处理
  const blockedReason =
    review && review.unresolvedCount > 0 ? `还有 ${review.unresolvedCount} 条明细待选印花税类别` : null;

  return (
    <div className="flex h-full min-h-0 min-w-0 flex-col gap-3 p-3">
      <Card>
        <CardContent>
          <TaxPeriodForm
            running={m.isRunning}
            onRun={() => void handleRun()}
            onReset={m.reset}
            runDisabled={readOnlyBatch}
            runDisabledReason={readOnlyReason}
          >
            <Field wide label="纳税所属期（最多 3 个月）" error={m.errors.filingFrom ?? m.errors.filingTo}>
              <PeriodRangePicker
                periodFrom={m.filing.from}
                periodTo={m.filing.to}
                onChange={(from, to) => m.changeFiling({ from, to })}
              />
            </Field>

            <Field wide label="凭证入账期间" error={m.errors.srcFrom ?? m.errors.srcTo}>
              <PeriodRangePicker
                periodFrom={m.src.from}
                periodTo={m.src.to}
                onChange={(from, to) => m.changeSrc({ from, to })}
              />
            </Field>

            <LayoutField layouts={layouts} />
          </TaxPeriodForm>
        </CardContent>
      </Card>

      <Card className="min-h-0 flex-1">
        <CardContent className="flex min-h-0 flex-1 flex-col gap-2">
          <StampTaxReviewTable
            resetKey={m.runCount}
            layouts={layouts}
            filingFrom={filingFrom}
            filingTo={filingTo}
            review={review}
            isLoading={m.isRunning}
            onReviewChange={(next) => {
              m.setReview(next);
              void refresh();
            }}
          />

          {review && (
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="secondary">
                纳税所属期 {filingFrom === filingTo ? filingFrom : `${filingFrom}-${filingTo}`}
              </Badge>
              <Badge variant="secondary">明细 {review.totalRowCount} 条</Badge>
              <Badge variant="secondary">已命中 {review.matchedRowCount} 条</Badge>
              <Badge variant="secondary">未命中 {review.notMatchedCount} 条</Badge>
              <Badge variant={review.needsReviewCount > 0 ? "destructive" : "secondary"}>
                需人工核对 {review.needsReviewCount} 条
              </Badge>
              <Badge variant="secondary">人工修改 {review.manualCount} 条</Badge>
              {review.unresolvedCount > 0 && (
                <Badge variant="destructive">待选类别 {review.unresolvedCount} 条（必须处理）</Badge>
              )}
              <Badge>
                本所属期计入 {review.includedRowCount} 条，行级印花税 {fenToYuan(review.totalStampTax)} 元
              </Badge>
              {review.batchNo > 1 && (
                <Badge variant="outline">
                  本批（补充）{review.batchRowCount} 条，行级印花税 {fenToYuan(review.batchStampTax)} 元
                </Badge>
              )}
              <div className="ml-auto">
                <PeriodStatusBar
                  module="stamp_tax"
                  filingFrom={filingFrom}
                  filingTo={filingTo}
                  status={status}
                  onStatusChange={(next) => {
                    setStatus(next);
                    void reload();
                  }}
                  onConfirm={() =>
                    commands.confirmStampTax({ module: "stamp_tax", filingFrom, filingTo })
                  }
                  hasResult={review.batchRowCount > 0}
                  confirmSummary={{
                    rows: review.batchRowCount,
                    amount: fenToYuan(review.batchStampTax),
                    amountLabel: "行级印花税合计",
                  }}
                  blockedReason={blockedReason}
                  extraActions={
                    status && (
                      <>
                        <ExportButton
                          defaultName={`印花税做账清单_${filingFrom === filingTo ? filingFrom : filingFrom + "-" + filingTo}${review.batchNo > 1 ? "_补充" : ""}`}
                          label="导出做账清单"
                          run={(path) =>
                            commands.exportStampTaxAccounting({ filingFrom, filingTo }, STAMP_EXPORT, path)
                          }
                        />
                        <CollectionFormButton
                          filingFrom={filingFrom}
                          filingTo={filingTo}
                          batchNo={status.batchNo}
                          disabled={review.batchRowCount === 0}
                        />
                      </>
                    )
                  }
                />
              </div>
            </div>
          )}
        </CardContent>
      </Card>

      <AlertDialog open={resetAsk !== null} onOpenChange={(open) => !open && setResetAsk(null)}>
        <AlertDialogContent>
          <AlertDialogHeader>
            <AlertDialogTitle>更改凭证期间将重新开始？</AlertDialogTitle>
            <AlertDialogDescription>
              本纳税所属期已有 {resetAsk?.count} 条匹配结果和人工修改，更改凭证期间会全部清空，从头开始匹配。
            </AlertDialogDescription>
          </AlertDialogHeader>
          <AlertDialogFooter>
            <AlertDialogCancel>取消</AlertDialogCancel>
            <AlertDialogAction
              variant="destructive"
              onClick={(e) => {
                e.preventDefault();
                setResetAsk(null);
                void handleRun(true);
              }}
            >
              清空并重新匹配
            </AlertDialogAction>
          </AlertDialogFooter>
        </AlertDialogContent>
      </AlertDialog>
    </div>
  );
}
