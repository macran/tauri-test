import { Loader2, Play } from "lucide-react";
import { useState } from "react";

import { commands, type ReviewSummary } from "@/bindings";
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

import { InputTaxTransferReviewTable } from "./components/InputTaxTransferReviewTable";
import { PeriodRangePicker } from "./components/PeriodRangePicker";
import {
  currentPeriod,
  fenToYuan,
  getErrorMessage,
} from "./utils";

export default function InputTaxTransferPage() {
  const initialPeriod = currentPeriod();

  const [periodFrom, setPeriodFrom] = useState(initialPeriod);
  const [periodTo, setPeriodTo] = useState(initialPeriod);

  /**
   * 独立记录是否启用跨期间。
   *
   * 不能通过 periodFrom !== periodTo 判断，
   * 因为刚勾选区间时两个期间可能暂时相同。
   */
  const [rangeEnabled, setRangeEnabled] = useState(false);

  /**
   * review 为 null 表示本次选择的期间尚未点击“开始匹配”。
   *
   * 期间选择本身不会触发后端查询。
   */
  const [review, setReview] = useState<ReviewSummary | null>(null);
  const [isRunning, setIsRunning] = useState(false);

  function handlePeriodChange(nextFrom: string, nextTo: string) {
    setPeriodFrom(nextFrom);
    setPeriodTo(nextTo);

    /**
     * 期间发生变化后，清空旧期间的列表。
     * 避免用户看到上一期间的数据，却误认为是新期间的数据。
     */
    setReview(null);
  }

  function handleRangeEnabledChange(enabled: boolean) {
    setRangeEnabled(enabled);

    if (!enabled) {
      /**
       * 关闭跨期间时，结束期间恢复为起始期间。
       */
      setPeriodTo(periodFrom);
    }

    /**
     * 区间模式发生变化后，旧列表也不再适用于当前选择。
     */
    setReview(null);
  }

  async function handleRun() {
    setIsRunning(true);

    const toastId = toast.add({
      type: "loading",
      title: "正在匹配……",
    });

    try {
      /**
       * 单期间时强制使用 periodFrom。
       * 跨期间时使用用户选择的 periodTo。
       */
      const requestPeriodTo = rangeEnabled ? periodTo : periodFrom;

      const result = await commands.runInputTaxTransferMatch({
        periodFrom,
        periodTo: requestPeriodTo,
      });

      if (result.status !== "ok") {
        throw new Error(getErrorMessage(result.error));
      }

      /**
       * runInputTaxTransferMatch 内部会：
       *
       * 1. 回填认证清单凭证号；
       * 2. 按期间查询认证清单；
       * 3. 执行规则匹配；
       * 4. 查询所选期间全部发票明细；
       * 5. 返回完整核对列表。
       */
      setReview(result.data.review);

      toast.update(toastId, {
        type: "success",
        title: "匹配完成",
        description: [
          `回填凭证号 ${result.data.voucherFilledCount} 条`,
          `参与匹配 ${result.data.qualifiedItemCount} 条`,
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
    periodFrom === periodTo
      ? periodFrom
      : `${periodFrom} – ${periodTo}`;

  return (
    <Card className="h-full">
      <CardHeader>
        <CardTitle>进项税转出</CardTitle>

        <p className="mt-1 text-sm text-muted-foreground">
          选择认证期间后点击“开始匹配”。系统会列出所选期间认证清单对应的全部发票明细；
          只有有凭证号的明细才参与规则匹配。
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
            <Badge variant="secondary">
              期间 {periodLabel}
            </Badge>

            <Badge variant="secondary">
              明细 {review.totalRowCount} 条
            </Badge>

            <Badge variant="secondary">
              已命中 {review.matchedRowCount} 条
            </Badge>

            <Badge variant="secondary">
              未命中 {review.notMatchedCount} 条
            </Badge>

            <Badge variant="secondary">
              缺凭证号 {review.missingVoucherCount} 条
            </Badge>

            <Badge variant="secondary">
              需人工核对 {review.needsReviewCount} 条
            </Badge>

            <Badge variant="secondary">
              人工修改 {review.manualCount} 条
            </Badge>

            <Badge>
              转出 {review.includedRowCount} 条，合计{" "}
              {fenToYuan(review.totalTransferTax)} 元
            </Badge>
          </div>
        )}

        <Separator />

        <InputTaxTransferReviewTable
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