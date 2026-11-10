import { AlertTriangle, ChevronDown, ChevronRight, RotateCcw } from "lucide-react";
import { Fragment, useMemo, useState } from "react";

import {
  commands,
  type MatchedRule,
  type OverridePatch,
  type ReviewRow,
  type ReviewSummary,
} from "@/bindings";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import { toast } from "@/components/ui/toast";

import { fenToInput, fenToYuan, getErrorMessage } from "../utils";
import { yuanToFen } from "../utils";
import { ReviewBulkActionBar, type BulkAction } from "./ReviewBulkActionBar";
import {
  applyFilter,
  DEFAULT_FILTER,
  ReviewFilterBar,
  type ReviewFilter,
} from "./ReviewFilterBar";

const COLUMN_COUNT = 14;

const MATCH_STATE_META: Record<
  ReviewRow["matchState"],
  { label: string; className: string }
> = {
  matched: {
    label: "已命中",
    className:
      "border-emerald-600/30 bg-emerald-100 text-emerald-700 dark:bg-emerald-500/15 dark:text-emerald-400",
  },
  notMatched: {
    label: "未命中",
    className: "border-transparent bg-muted text-muted-foreground",
  },
  missingVoucher: {
    label: "缺凭证号",
    className:
      "border-slate-400/30 bg-slate-100 text-slate-600 dark:bg-slate-500/15 dark:text-slate-300",
  },
};

/** 命中规则详情：类别、条件、备注、人工核对标记全部展开 */
function MatchedRuleDetail({ rule }: { rule: MatchedRule }) {
  return (
    <div className="rounded-md border border-border/60 bg-background p-2 text-xs">
      <div className="flex flex-wrap items-center gap-2">
        <span className="font-medium">{rule.ruleCategory}</span>
        <span className="text-muted-foreground">规则 #{rule.ruleId}</span>
        {rule.needsManualReview && (
          <Badge className="border-amber-600/30 bg-amber-100 text-amber-700 dark:bg-amber-500/15 dark:text-amber-400">
            需人工核对
          </Badge>
        )}
        {!rule.enabled && (
          <Badge variant="outline" className="text-muted-foreground">
            规则已禁用
          </Badge>
        )}
      </div>

      <ul className="mt-1 space-y-0.5 text-muted-foreground">
        {rule.conditions.map((condition) => (
          <li key={`${condition.field}-${condition.operator}`}>
            {condition.field} {condition.operator}{" "}
            <span className="font-mono text-foreground">{condition.values}</span>
          </li>
        ))}
      </ul>

      {rule.note.trim() !== "" && (
        <p className="mt-1 whitespace-pre-line text-muted-foreground">
          备注：{rule.note}
        </p>
      )}
    </div>
  );
}

function MatchStateCell({ row }: { row: ReviewRow }) {
  const meta = MATCH_STATE_META[row.matchState];

  return (
    <div className="flex flex-wrap items-center gap-1">
      <Badge className={meta.className}>{meta.label}</Badge>
      {row.needsManualReview && (
        <Badge className="border-amber-600/30 bg-amber-100 text-amber-700 dark:bg-amber-500/15 dark:text-amber-400">
          需核对
        </Badge>
      )}
    </div>
  );
}

function DecisionSourceCell({ row }: { row: ReviewRow }) {
  if (row.decisionSource === "system") {
    return <span className="text-muted-foreground">机器匹配</span>;
  }

  const changed = [
    row.decisionManuallySet ? "判定" : null,
    row.taxManuallyEdited ? "税额" : null,
  ].filter((value): value is string => value !== null);

  return (
    <div className="flex flex-wrap items-center gap-1">
      <Badge className="border-sky-600/30 bg-sky-100 text-sky-700 dark:bg-sky-500/15 dark:text-sky-400">
        人工修改
      </Badge>
      {changed.length > 0 && (
        <span className="text-xs text-muted-foreground">（{changed.join("、")}）</span>
      )}
    </div>
  );
}

type Props = {
  periodFrom: string;
  periodTo: string;
  review: ReviewSummary | null;
  isLoading: boolean;
  onReviewChange: (review: ReviewSummary) => void;
};

export function InputTaxTransferReviewTable({
  periodFrom,
  periodTo,
  review,
  isLoading,
  onReviewChange,
}: Props) {
  const [filter, setFilter] = useState<ReviewFilter>(DEFAULT_FILTER);
  const [expanded, setExpanded] = useState<Set<number>>(new Set());
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [drafts, setDrafts] = useState<Record<number, string>>({});
  const [savingId, setSavingId] = useState<number | null>(null);
  const [isBulkSaving, setIsBulkSaving] = useState(false);

  const allRows = review?.rows ?? [];
  const rows = useMemo(() => applyFilter(allRows, filter), [allRows, filter]);

  const visibleIds = useMemo(() => rows.map((row) => row.detailId), [rows]);
  const selectedVisibleCount = visibleIds.filter((id) => selected.has(id)).length;
  const allVisibleSelected =
    visibleIds.length > 0 && selectedVisibleCount === visibleIds.length;

  function toggleExpanded(detailId: number) {
    setExpanded((previous) => {
      const next = new Set(previous);
      if (next.has(detailId)) {
        next.delete(detailId);
      } else {
        next.add(detailId);
      }
      return next;
    });
  }

  function toggleSelected(detailId: number, checked: boolean) {
    setSelected((previous) => {
      const next = new Set(previous);
      if (checked) {
        next.add(detailId);
      } else {
        next.delete(detailId);
      }
      return next;
    });
  }

  function toggleSelectAllVisible(checked: boolean) {
    setSelected((previous) => {
      const next = new Set(previous);
      for (const id of visibleIds) {
        if (checked) {
          next.add(id);
        } else {
          next.delete(id);
        }
      }
      return next;
    });
  }

  function clearDraft(detailId: number) {
    setDrafts((previous) => {
      const next = { ...previous };
      delete next[detailId];
      return next;
    });
  }

  async function saveSingle(row: ReviewRow, patch: OverridePatch) {
    setSavingId(row.detailId);
    try {
      const result = await commands.saveInputTaxTransferOverride({
        periodFrom,
        periodTo,
        ...patch,
      });
      if (result.status !== "ok") throw new Error(getErrorMessage(result.error));

      onReviewChange(result.data);
      clearDraft(row.detailId);
    } catch (error) {
      toast.add({
        type: "error",
        title: "保存失败",
        description: getErrorMessage(error),
      });
    } finally {
      setSavingId(null);
    }
  }

  /**
   * 勾选转出只写 included；转出税额留空由后端派生，
   * 这样「税额被人工改过」只在用户真的输入数字时才成立。
   * 若之前已人工改过税额，则保留该值。
   */
  function toggleIncluded(row: ReviewRow, included: boolean) {
    void saveSingle(row, {
      detailId: row.detailId,
      included,
      transferTax: row.taxManuallyEdited ? row.finalTransferTax : null,
      remark: row.remark,
    });
  }

  function commitTax(row: ReviewRow) {
    const draft = drafts[row.detailId];
    if (draft === undefined) return;

    const fen = yuanToFen(draft);
    if (fen === null) {
      toast.add({ type: "error", title: "转出税额格式错误，最多两位小数" });
      return;
    }

    if (fen === row.finalTransferTax) {
      clearDraft(row.detailId);
      return;
    }

    void saveSingle(row, {
      detailId: row.detailId,
      included: row.decisionManuallySet ? row.finalIncluded : null,
      transferTax: fen,
      remark: row.remark,
    });
  }

  /** 三项全传 null，后端会删除该行，回到机器判定。 */
  function resetToSystem(row: ReviewRow) {
    void saveSingle(row, {
      detailId: row.detailId,
      included: null,
      transferTax: null,
      remark: null,
    });
  }

  async function applyBulk(action: BulkAction) {
    const targets = allRows.filter((row) => selected.has(row.detailId));
    if (targets.length === 0) return;

    const items: OverridePatch[] = targets.map((row) => {
      if (action === "reset") {
        return {
          detailId: row.detailId,
          included: null,
          transferTax: null,
          remark: null,
        };
      }

      return {
        detailId: row.detailId,
        included: action === "include",
        transferTax: row.taxManuallyEdited ? row.finalTransferTax : null,
        remark: row.remark,
      };
    });

    setIsBulkSaving(true);
    const toastId = toast.add({ type: "loading", title: "正在保存……" });

    try {
      const result = await commands.saveInputTaxTransferOverridesBatch({
        periodFrom,
        periodTo,
        items,
      });
      if (result.status !== "ok") throw new Error(getErrorMessage(result.error));

      onReviewChange(result.data);
      setSelected(new Set());
      setDrafts({});

      const label =
        action === "include"
          ? "已标记为转出"
          : action === "exclude"
            ? "已标记为不转出"
            : "已恢复系统结论";

      toast.update(toastId, {
        type: "success",
        title: label,
        description: `共 ${items.length} 条`,
      });
    } catch (error) {
      toast.update(toastId, {
        type: "error",
        title: "批量保存失败",
        description: getErrorMessage(error),
      });
    } finally {
      setIsBulkSaving(false);
    }
  }

  return (
    <div className="space-y-3">
      <ReviewFilterBar
        filter={filter}
        onChange={setFilter}
        visibleCount={rows.length}
        totalCount={allRows.length}
      />

      <ReviewBulkActionBar
        selectedCount={selected.size}
        isSaving={isBulkSaving}
        onApply={applyBulk}
        onClearSelection={() => setSelected(new Set())}
      />

      <div className="rounded-md border">
        <Table>
          <TableHeader>
            <TableRow>
              <TableHead className="w-10">
                <Checkbox
                  checked={allVisibleSelected}
                  disabled={visibleIds.length === 0}
                  onCheckedChange={(checked) => toggleSelectAllVisible(checked === true)}
                  aria-label="选择当前筛选出的全部明细"
                />
              </TableHead>
              <TableHead className="w-8" />
              <TableHead className="w-20">期间</TableHead>
              <TableHead className="w-28">凭证号</TableHead>
              <TableHead className="w-36">销售方名称</TableHead>
              <TableHead className="w-48">商品名称</TableHead>
              <TableHead className="w-36">税收分类编码简称</TableHead>
              <TableHead className="w-24 text-right">金额</TableHead>
              <TableHead className="w-24 text-right">税额</TableHead>
              <TableHead className="w-32">匹配状态</TableHead>
              <TableHead className="w-36">结论来源</TableHead>
              <TableHead className="w-16 text-center">转出</TableHead>
              <TableHead className="w-28 text-right">转出税额</TableHead>
              <TableHead className="w-24">操作</TableHead>
            </TableRow>
          </TableHeader>

          <TableBody>
            {isLoading ? (
              <TableRow>
                <TableCell colSpan={COLUMN_COUNT} className="h-24 text-center">
                  正在加载……
                </TableCell>
              </TableRow>
            ) : allRows.length === 0 ? (
              <TableRow>
                <TableCell
                  colSpan={COLUMN_COUNT}
                  className="h-24 text-center text-muted-foreground"
                >
                  所选期间的认证清单没有发票明细
                </TableCell>
              </TableRow>
            ) : rows.length === 0 ? (
              <TableRow>
                <TableCell
                  colSpan={COLUMN_COUNT}
                  className="h-24 text-center text-muted-foreground"
                >
                  当前筛选条件下没有明细，调整筛选即可查看
                </TableCell>
              </TableRow>
            ) : (
              rows.map((row) => {
                const isExpanded = expanded.has(row.detailId);
                const isSaving = savingId === row.detailId || isBulkSaving;
                const taxValue = drafts[row.detailId] ?? fenToInput(row.finalTransferTax);

                return (
                  <Fragment key={row.detailId}>
                    <TableRow
                      className={
                        row.decisionSource === "manual"
                          ? "bg-sky-50/50 dark:bg-sky-500/5"
                          : undefined
                      }
                    >
                      <TableCell>
                        <Checkbox
                          checked={selected.has(row.detailId)}
                          onCheckedChange={(checked) =>
                            toggleSelected(row.detailId, checked === true)
                          }
                          aria-label={`选择明细 ${row.detailId}`}
                        />
                      </TableCell>

                      <TableCell>
                        {row.matchedRules.length > 0 && (
                          <Button
                            type="button"
                            variant="ghost"
                            size="icon"
                            className="size-6"
                            onClick={() => toggleExpanded(row.detailId)}
                            aria-label={isExpanded ? "收起命中规则" : "展开命中规则"}
                          >
                            {isExpanded ? (
                              <ChevronDown className="size-4" />
                            ) : (
                              <ChevronRight className="size-4" />
                            )}
                          </Button>
                        )}
                      </TableCell>

                      <TableCell>{row.taxPeriod}</TableCell>

                      <TableCell>
                        {row.voucherNo ?? (
                          <span className="flex items-center gap-1 text-muted-foreground">
                            <AlertTriangle className="size-3.5" />
                            缺凭证号
                          </span>
                        )}
                      </TableCell>

                      <TableCell>
                        <span className="block max-w-36 truncate" title={row.sellerName}>
                          {row.sellerName}
                        </span>
                      </TableCell>

                      <TableCell>
                        <span className="block max-w-48 truncate" title={row.goodsName}>
                          {row.goodsName}
                        </span>
                      </TableCell>

                      <TableCell>{row.goodsCategory ?? "—"}</TableCell>
                      <TableCell className="text-right">{fenToYuan(row.amount)}</TableCell>
                      <TableCell className="text-right">{fenToYuan(row.tax)}</TableCell>

                      <TableCell>
                        <MatchStateCell row={row} />
                      </TableCell>

                      <TableCell>
                        <DecisionSourceCell row={row} />
                      </TableCell>

                      <TableCell className="text-center">
                        <Checkbox
                          checked={row.finalIncluded}
                          disabled={isSaving}
                          onCheckedChange={(checked) =>
                            toggleIncluded(row, checked === true)
                          }
                          aria-label="是否作为进项税转出"
                        />
                      </TableCell>

                      <TableCell>
                        <Input
                          className="h-8 text-right"
                          value={taxValue}
                          disabled={!row.finalIncluded || isSaving}
                          inputMode="decimal"
                          onChange={(event) =>
                            setDrafts((previous) => ({
                              ...previous,
                              [row.detailId]: event.target.value,
                            }))
                          }
                          onBlur={() => commitTax(row)}
                          onKeyDown={(event) => {
                            if (event.key === "Enter") commitTax(row);
                            if (event.key === "Escape") clearDraft(row.detailId);
                          }}
                          aria-label="转出税额（元）"
                        />
                      </TableCell>

                      <TableCell>
                        {row.decisionSource === "manual" && (
                          <Button
                            type="button"
                            variant="ghost"
                            size="sm"
                            className="h-7 px-2 text-xs"
                            disabled={isSaving}
                            onClick={() => resetToSystem(row)}
                          >
                            <RotateCcw className="mr-1 size-3.5" />
                            恢复
                          </Button>
                        )}
                      </TableCell>
                    </TableRow>

                    {isExpanded && row.matchedRules.length > 0 && (
                      <TableRow>
                        <TableCell colSpan={COLUMN_COUNT} className="bg-muted/30 p-3">
                          <div className="space-y-2">
                            {row.matchedRules.map((rule) => (
                              <MatchedRuleDetail key={rule.ruleId} rule={rule} />
                            ))}
                          </div>
                        </TableCell>
                      </TableRow>
                    )}
                  </Fragment>
                );
              })
            )}
          </TableBody>
        </Table>
      </div>
    </div>
  );
}