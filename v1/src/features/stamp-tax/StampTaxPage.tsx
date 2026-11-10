import { Loader2, Play } from "lucide-react";
import { useState } from "react";

import { commands, type StampTaxReviewSummary } from "@/bindings";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import {
  Card,
  CardContent,
  CardHeader,
  CardTitle,
} from "@/components/ui/card";
import { Separator } from "@/components/ui/separator";
import { toast } from "@/components/ui/toast";

import { PeriodRangePicker } from "../input-tax-transfer/components/PeriodRangePicker";
import { StampTaxReviewTable } from "./components/StampTaxReviewTable";
import { currentPeriod, getErrorMessage } from "./utils";

export default function StampTaxPage() {
  const initialPeriod = currentPeriod();

  const [periodFrom, setPeriodFrom] = useState(initialPeriod);
  const [periodTo, setPeriodTo] = useState(initialPeriod);

  /**
   * 不能通过 periodFrom !== periodTo 判断是否启用跨期间，
   * 因为刚勾选区间时两个期间可能暂时相同。
   */
  const [rangeEnabled, setRangeEnabled] = useState(false);

  /**
   * review 为 null 表示本次选择的期间尚未点击“开始匹配”。
   */
  const [review, setReview] = useState<StampTaxReviewSummary | null>(null);
  const [isRunning, setIsRunning] = useState(false);

  function handlePeriodChange(nextFrom: string, nextTo: string) {
    setPeriodFrom(nextFrom);
    setPeriodTo(nextTo);
    setReview(null);
  }

  function handleRangeEnabledChange(enabled: boolean) {
    setRangeEnabled(enabled);

    if (!enabled) {
      setPeriodTo(periodFrom);
    }

    setReview(null);
  }

  async function handleRun() {
    setIsRunning(true);

    const toastId = toast.add({
      type: "loading",
      title: "正在匹配……",
    });

    try {
      const requestPeriodTo = rangeEnabled ? periodTo : periodFrom;

      const result = await commands.runStampTaxMatch({
        periodFrom,
        periodTo: requestPeriodTo,
      });

      if (result.status !== "ok") {
        throw new Error(getErrorMessage(result.error));
      }

      /**
       * runStampTaxMatch 内部会：
       *
       * 1. 按做账期间（AP/Bank/IC 凭证入账日期，或差旅认证清单过账日期）
       *    找出该期间内已做账的发票；
       * 2. 按 scope = stamp_tax 的规则逐条匹配其发票明细；
       * 3. 返回所选期间全部已做账明细的核对列表（含未命中的）。
       */
      setReview(result.data.review);

      toast.update(toastId, {
        type: "success",
        title: "匹配完成",
        description: [
          `已做账参与匹配 ${result.data.qualifiedItemCount} 条`,
          `命中 ${result.data.matchedItemCount} 条`,
          `保留人工结论 ${result.data.preservedOverrideCount} 条`,
        ].join("，"),
      });
    } catch (error) {
      toast.update(toastId, {
        type: "error",
        title: "匹配失败",
        description: getErrorMessage(error),
      });
    } finally {
      setIsRunning(false);
    }
  }

  const periodLabel =
    periodFrom === periodTo ? periodFrom : `${periodFrom} – ${periodTo}`;

  return (
    <Card className="h-full">
      <CardHeader>
        <CardTitle>印花税计算</CardTitle>

        <p className="mt-1 text-sm text-muted-foreground">
          选择期间后点击“开始匹配”。系统按做账日期（AP / IC / 银行凭证入账日期，
          或差旅认证清单过账日期）找出该期间已做账的发票明细，并按印花税规则逐条匹配——
          无论是否通过增值税认证，只要已做账就会显示。
        </p>
      </CardHeader>

      <CardContent className="space-y-4">
        <div className="flex flex-wrap items-center gap-3">
          <PeriodRangePicker
            periodFrom={periodFrom}
            periodTo={periodTo}
            rangeEnabled={rangeEnabled}
            onRangeEnabledChange={handleRangeEnabledChange}
            onChange={handlePeriodChange}
          />

          <Button
            type="button"
            onClick={() => void handleRun()}
            disabled={isRunning}
          >
            {isRunning ? (
              <Loader2 className="mr-2 size-4 animate-spin" />
            ) : (
              <Play className="mr-2 size-4" />
            )}
            开始匹配
          </Button>
        </div>

        {review && (
          <div className="flex flex-wrap items-center gap-2">
            <Badge variant="secondary">期间 {periodLabel}</Badge>

            <Badge variant="secondary">明细 {review.totalRowCount} 条</Badge>

            <Badge variant="secondary">已命中 {review.matchedRowCount} 条</Badge>

            <Badge variant="secondary">未命中 {review.notMatchedCount} 条</Badge>

            <Badge variant="secondary">
              需人工核对 {review.needsReviewCount} 条
            </Badge>

            <Badge variant="secondary">人工修改 {review.manualCount} 条</Badge>

            {review.unresolvedCount > 0 && (
              <Badge variant="destructive">
                待选类别 {review.unresolvedCount} 条（必须处理）
              </Badge>
            )}

            <Badge>计入印花税 {review.includedRowCount} 条</Badge>
          </div>
        )}

        <Separator />

        <StampTaxReviewTable
          periodFrom={periodFrom}
          periodTo={rangeEnabled ? periodTo : periodFrom}
          review={review}
          isLoading={isRunning}
          onReviewChange={setReview}
        />
      </CardContent>
    </Card>
  );
}
