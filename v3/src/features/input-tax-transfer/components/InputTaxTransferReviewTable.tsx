import type { ColumnDef } from "@tanstack/react-table";
import { Check, ListFilter, RotateCcw, X } from "lucide-react";
import { useMemo, useState } from "react";

import {
  commands,
  type MatchedRule,
  type OverridePatch,
  type ReviewRow,
  type ReviewSummary,
} from "@/bindings";
import { DataTable, type DataTableProps, type dataTableFeatures } from "@/features/query/DataTable";
import type { TableLayouts } from "@/features/query/table-layout";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { toast } from "@/components/ui/toast";

import { fenToInput, fenToYuan, getErrorMessage, yuanToFen } from "../utils";

const NUMERIC = ["amount", "tax", "finalTransferTax"] as const;
type Column = ColumnDef<typeof dataTableFeatures, ReviewRow>;
type BulkAction = "include" | "exclude" | "reset";

const MATCH_STATE_LABEL: Record<ReviewRow["matchState"], string> = {
  matched: "已命中",
  notMatched: "未命中",
  missingVoucher: "缺凭证号",
};

const YES_NO = [
  { value: "是", label: "是" },
  { value: "否", label: "否" },
];

/** 低基数列做下拉多选；其余列走全局搜索。"auto" = 取结果集里出现过的值。 */
const FILTER_COLUMNS: NonNullable<DataTableProps<ReviewRow>["filterColumns"]> = {
  certPeriod: "auto",
  voucherNo: "auto",
  batchLabel: "auto",
  transferVoucherNo: "auto",
  sellerName: "auto",
  goodsCategory: "auto",
  matchState: [
    { value: "已命中", label: "已命中" },
    { value: "未命中", label: "未命中" },
    { value: "缺凭证号", label: "缺凭证号" },
  ],
  needsManualReview: YES_NO,
  decisionSource: [
    { value: "机器匹配", label: "机器匹配" },
    { value: "人工修改", label: "人工修改" },
  ],
  finalIncluded: YES_NO,
};

/** 命中规则详情：类别、条件、备注、人工核对标记 */
function MatchedRuleDetail({ rule }: { rule: MatchedRule }) {
  return (
    <div className="rounded-md border border-border/60 bg-background p-2 text-xs">
      <div className="flex flex-wrap items-center gap-2">
        <span className="font-medium">{rule.ruleCategory}</span>
        <span className="text-muted-foreground">规则 #{rule.ruleId}</span>
        {rule.needsManualReview && <Badge variant="secondary">需人工核对</Badge>}
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
        <p className="mt-1 whitespace-pre-line text-muted-foreground">备注：{rule.note}</p>
      )}
    </div>
  );
}

function RulesPopover({ rules }: { rules: MatchedRule[] }) {
  if (rules.length === 0) return <span className="text-muted-foreground">—</span>;

  return (
    <Popover>
      <PopoverTrigger
        render={<Button type="button" variant="ghost" size="sm" className="h-6 px-1.5 text-xs" />}
      >
        <ListFilter className="mr-1 size-3.5" />
        {rules.length} 条
      </PopoverTrigger>
      <PopoverContent className="max-h-96 w-96 overflow-auto" align="start">
        {rules.map((rule) => (
          <MatchedRuleDetail key={rule.ruleId} rule={rule} />
        ))}
      </PopoverContent>
    </Popover>
  );
}

/** 转出税额输入：失焦/回车保存，Esc 放弃；自带草稿，不依赖父级状态。 */
function TaxCell({
  row,
  disabled,
  onCommit,
}: {
  row: ReviewRow;
  disabled: boolean;
  onCommit: (row: ReviewRow, fen: string) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const value = draft ?? fenToInput(row.finalTransferTax);

  const commit = () => {
    if (draft === null) return;
    const fen = yuanToFen(draft);
    if (fen === null) {
      toast.add({ type: "error", title: "转出税额格式错误，最多两位小数" });
      return;
    }
    setDraft(null);
    if (fen !== row.finalTransferTax) onCommit(row, fen);
  };

  return (
    <Input
      className="h-7 text-right"
      value={value}
      disabled={disabled || !row.finalIncluded}
      inputMode="decimal"
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
        if (e.key === "Escape") setDraft(null);
      }}
      aria-label="转出税额（元）"
    />
  );
}

/** 批次列：未确认为空；本所属期已确认 = 「批次 1 已确认/已锁定」；别的所属期确认的写明所属期 */
export function confirmText(r: ReviewRow): string {
  const c = r.confirm;
  if (!c) return "";
  const state = c.state === "locked" ? "已锁定" : "已确认";
  const batch = c.batchNo > 1 ? `补充批次 ${c.batchNo}` : "批次 1";
  return c.sameRun ? `${batch} ${state}` : `${c.filingPeriod} ${batch} ${state}`;
}

type Props = {
  layouts: TableLayouts;
  /** 值变化时清空表格的搜索和过滤 */
  resetKey: number;
  /** 当前结果所属的纳税所属期 */
  filingFrom: string;
  filingTo: string;
  review: ReviewSummary | null;
  isLoading: boolean;
  onReviewChange: (review: ReviewSummary) => void;
  /** 表格工具栏里的表级操作（导出等） */
  toolbarSlot?: React.ReactNode;
};

export function InputTaxTransferReviewTable({
  layouts,
  resetKey,
  filingFrom,
  filingTo,
  review,
  isLoading,
  onReviewChange,
  toolbarSlot,
}: Props) {
  const [savingId, setSavingId] = useState<number | null>(null);
  const [isBulkSaving, setIsBulkSaving] = useState(false);
  // 批次已确认或已锁定时整体只读：后端也会拒绝，这里让用户别点了才报错
  const readOnly = review ? review.state !== "open" : false;

  const rows = review?.rows ?? [];

  async function saveSingle(row: ReviewRow, patch: OverridePatch) {
    setSavingId(row.detailId);
    try {
      const result = await commands.saveInputTaxTransferOverride({ filingFrom, filingTo, ...patch });
      if (result.status !== "ok") throw new Error(getErrorMessage(result.error));
      onReviewChange(result.data);
    } catch (error) {
      toast.add({ type: "error", title: "保存失败", description: getErrorMessage(error) });
    } finally {
      setSavingId(null);
    }
  }

  /**
   * 勾选转出只写 included；转出税额留空由后端派生，
   * 这样「税额被人工改过」只在用户真的输入数字时才成立。
   */
  const toggleIncluded = (row: ReviewRow, included: boolean) =>
    void saveSingle(row, {
      detailId: row.detailId,
      included,
      transferTax: row.taxManuallyEdited ? row.finalTransferTax : null,
      remark: row.remark,
    });

  const commitTax = (row: ReviewRow, fen: string) =>
    void saveSingle(row, {
      detailId: row.detailId,
      included: row.decisionManuallySet ? row.finalIncluded : null,
      transferTax: fen,
      remark: row.remark,
    });

  /** 三项全传 null，后端会删除该行，回到机器判定。 */
  const resetToSystem = (row: ReviewRow) =>
    void saveSingle(row, { detailId: row.detailId, included: null, transferTax: null, remark: null });

  async function applyBulk(targets: ReviewRow[], action: BulkAction, clear: () => void) {
    targets = targets.filter((row) => !row.locked);
    if (targets.length === 0) return;

    const items: OverridePatch[] = targets.map((row) =>
      action === "reset"
        ? { detailId: row.detailId, included: null, transferTax: null, remark: null }
        : {
            detailId: row.detailId,
            included: action === "include",
            transferTax: row.taxManuallyEdited ? row.finalTransferTax : null,
            remark: row.remark,
          },
    );

    setIsBulkSaving(true);
    const toastId = toast.add({ type: "loading", title: "正在保存……" });

    try {
      const result = await commands.saveInputTaxTransferOverridesBatch({ filingFrom, filingTo, items });
      if (result.status !== "ok") throw new Error(getErrorMessage(result.error));

      onReviewChange(result.data);
      clear();

      toast.update(toastId, {
        type: "success",
        title:
          action === "include"
            ? "已标记为转出"
            : action === "exclude"
              ? "已标记为不转出"
              : "已恢复系统结论",
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

  // 列定义依赖 savingId / isBulkSaving，变化时重建；DataTable 按列 id 记布局，不受影响。
  const columns = useMemo<Column[]>(() => {
    const busy = (row: ReviewRow) => readOnly || row.locked || savingId === row.detailId || isBulkSaving;

    return [
      { id: "certPeriod", header: "认证期间", size: 80, accessorFn: (r) => r.certPeriod },
      { id: "voucherNo", header: "原记账凭证号", size: 120, accessorFn: (r) => r.voucherNo ?? "" },
      { id: "invoiceNo", header: "发票号码", size: 190, accessorFn: (r) => r.invoiceNo },
      { id: "issueDate", header: "开票日期", size: 100, accessorFn: (r) => r.issueDate },
      { id: "sellerName", header: "销售方名称", size: 200, accessorFn: (r) => r.sellerName },
      { id: "goodsName", header: "商品名称", size: 200, accessorFn: (r) => r.goodsName },
      {
        id: "goodsCategory",
        header: "税收分类编码简称",
        size: 150,
        accessorFn: (r) => r.goodsCategory ?? "",
      },
      {
        id: "amount",
        header: "金额",
        size: 110,
        accessorFn: (r) => r.amount,
        cell: ({ row }) => <span className="block">{fenToYuan(row.original.amount)}</span>,
      },
      {
        id: "tax",
        header: "税额",
        size: 100,
        accessorFn: (r) => r.tax,
        cell: ({ row }) => <span className="block">{fenToYuan(row.original.tax)}</span>,
      },
      {
        id: "matchState",
        header: "匹配状态",
        size: 90,
        accessorFn: (r) => MATCH_STATE_LABEL[r.matchState],
      },
      {
        id: "needsManualReview",
        header: "需人工核对",
        size: 90,
        accessorFn: (r) => (r.needsManualReview ? "是" : "否"),
      },
      {
        id: "rules",
        header: "命中规则",
        size: 90,
        accessorFn: (r) => r.matchedRules.length,
        cell: ({ row }) => <RulesPopover rules={row.original.matchedRules} />,
      },
      {
        id: "decisionSource",
        header: "结论来源",
        size: 90,
        accessorFn: (r) => (r.decisionSource === "manual" ? "人工修改" : "机器匹配"),
      },
      {
        id: "finalIncluded",
        header: "转出",
        size: 70,
        accessorFn: (r) => (r.finalIncluded ? "是" : "否"),
        cell: ({ row }) => (
          <Checkbox
            checked={row.original.finalIncluded}
            disabled={busy(row.original)}
            onCheckedChange={(checked) => toggleIncluded(row.original, checked === true)}
            aria-label="是否作为进项税转出"
          />
        ),
      },
      {
        id: "finalTransferTax",
        header: "转出税额",
        size: 120,
        accessorFn: (r) => r.finalTransferTax,
        cell: ({ row }) => (
          <TaxCell row={row.original} disabled={busy(row.original)} onCommit={commitTax} />
        ),
      },
      {
        id: "batchLabel",
        header: "批次",
        size: 150,
        accessorFn: (r) => confirmText(r),
      },
      {
        id: "transferVoucherNo",
        header: "转出凭证号",
        size: 120,
        accessorFn: (r) => r.confirm?.voucherNo ?? "",
      },
      { id: "transferPostedDate", header: "转出入账日期", size: 110, accessorFn: (r) => r.confirm?.postedDate ?? "" },
      { id: "transferPreparer", header: "做账人", size: 80, accessorFn: (r) => r.confirm?.preparer ?? "" },
    ];
    // toggleIncluded/commitTax 只依赖 taxPeriod/periodFrom/periodTo/onReviewChange，随它们重建即可
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [savingId, isBulkSaving, readOnly, filingFrom, filingTo, onReviewChange]);

  return (
    <DataTable
        toolbarSlot={toolbarSlot}
        layouts={layouts}
        resetKey={resetKey}
        numericColumns={NUMERIC}
        columns={columns}
        data={rows}
        loading={isLoading}
        total={rows.length}
        rowKey={(r) => r.detailId}
        filterColumns={FILTER_COLUMNS}
        rowClassName={(r) =>
          r.confirmedElsewhere
            ? "bg-muted/40 text-muted-foreground"
            : r.decisionSource === "manual"
              ? "bg-sky-50/50 dark:bg-sky-500/5"
              : undefined
        }
        // 已锁定的期间只读：不出现批量操作栏
        renderBulkActions={(selected, clear) =>
          readOnly ? null : (
            <>
              <span className="text-xs font-medium">已选 {selected.length} 条</span>
              <Button
                type="button"
                size="sm"
                disabled={isBulkSaving}
                onClick={() => void applyBulk(selected, "include", clear)}
              >
                <Check className="mr-1 size-3.5" />
                转出
              </Button>
              <Button
                type="button"
                size="sm"
                variant="outline"
                disabled={isBulkSaving}
                onClick={() => void applyBulk(selected, "exclude", clear)}
              >
                <X className="mr-1 size-3.5" />
                不转出
              </Button>
              <Button
                type="button"
                size="sm"
                variant="outline"
                disabled={isBulkSaving}
                onClick={() => void applyBulk(selected, "reset", clear)}
              >
                <RotateCcw className="mr-1 size-3.5" />
                恢复系统结论
              </Button>
            </>
          )
        }
        renderActions={(row) =>
          row.decisionSource === "manual" && !row.locked ? (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-7 px-2 text-xs"
              disabled={readOnly || row.locked || savingId === row.detailId || isBulkSaving}
              onClick={() => resetToSystem(row)}
            >
              <RotateCcw className="mr-1 size-3.5" />
              恢复
            </Button>
          ) : null
        }
      />
  );
}
