import type { ColumnDef } from "@tanstack/react-table";
import { Check, ListFilter, RotateCcw, X } from "lucide-react";
import { useEffect, useMemo, useState } from "react";

import {
  commands,
  type StampTaxCategoryOption,
  type StampTaxMatchedRule,
  type StampTaxOverridePatch,
  type StampTaxReviewRow,
  type StampTaxReviewSummary,
} from "@/bindings";
import { DataTable, type DataTableProps, type dataTableFeatures } from "@/components/query/DataTable";
import { useTableLayouts } from "@/components/query/table-layout";
import { LayoutSelect } from "@/components/query/LayoutSelect";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Popover, PopoverContent, PopoverTrigger } from "@/components/ui/popover";
import { toast } from "@/components/ui/toast";

import { fenToYuan, getErrorMessage } from "../utils";

type Column = ColumnDef<typeof dataTableFeatures, StampTaxReviewRow>;
type BulkAction = "include" | "exclude" | "reset";

const MATCH_STATE_LABEL: Record<StampTaxReviewRow["matchState"], string> = {
  matched: "已命中",
  notMatched: "未命中",
};

const YES_NO = [
  { value: "是", label: "是" },
  { value: "否", label: "否" },
];

/** 低基数列做下拉多选；其余列走全局搜索。"auto" = 取结果集里出现过的值。 */
const FILTER_COLUMNS: NonNullable<DataTableProps<StampTaxReviewRow>["filterColumns"]> = {
  taxPeriod: "auto",
  voucherNo: "auto",
  sellerName: "auto",
  goodsCategory: "auto",
  taxRate: "auto",
  matchState: [
    { value: "已命中", label: "已命中" },
    { value: "未命中", label: "未命中" },
  ],
  needsManualReview: YES_NO,
  decisionSource: [
    { value: "机器匹配", label: "机器匹配" },
    { value: "人工修改", label: "人工修改" },
  ],
  finalIncluded: YES_NO,
  stampTaxCategory: "auto",
};

/** 仅人工指定过的类别才回传，系统推断的类别不落库。 */
function manualCategoryId(row: StampTaxReviewRow) {
  return row.categoryManuallySet ? row.stampTaxCategoryId : null;
}

function rateText(rateBp: number) {
  return `${rateBp / 1000}%`;
}

/** 命中规则详情：类别、印花税税目、条件、备注、人工核对标记 */
function MatchedRuleDetail({ rule }: { rule: StampTaxMatchedRule }) {
  return (
    <div className="rounded-md border border-border/60 bg-background p-2 text-xs">
      <div className="flex flex-wrap items-center gap-2">
        <span className="font-medium">{rule.ruleCategory}</span>
        {rule.stampTaxCategory && <Badge variant="outline">{rule.stampTaxCategory}</Badge>}
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

function RulesPopover({ rules }: { rules: StampTaxMatchedRule[] }) {
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

/** 备注输入：失焦/回车保存，Esc 放弃；自带草稿，不依赖父级状态。 */
function RemarkCell({
  row,
  disabled,
  onCommit,
}: {
  row: StampTaxReviewRow;
  disabled: boolean;
  onCommit: (row: StampTaxReviewRow, remark: string | null) => void;
}) {
  const [draft, setDraft] = useState<string | null>(null);
  const value = draft ?? row.remark ?? "";

  const commit = () => {
    if (draft === null) return;
    const trimmed = draft.trim();
    setDraft(null);
    if (trimmed !== (row.remark ?? "")) onCommit(row, trimmed === "" ? null : trimmed);
  };

  return (
    <Input
      className="h-7"
      value={value}
      disabled={disabled}
      placeholder="备注"
      onChange={(e) => setDraft(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") commit();
        if (e.key === "Escape") setDraft(null);
      }}
      aria-label="备注"
    />
  );
}

type Props = {
  periodFrom: string;
  periodTo: string;
  review: StampTaxReviewSummary | null;
  isLoading: boolean;
  onReviewChange: (review: StampTaxReviewSummary) => void;
};

export function StampTaxReviewTable({
  periodFrom,
  periodTo,
  review,
  isLoading,
  onReviewChange,
}: Props) {
  const layouts = useTableLayouts("stamp-tax-review");
  const [categories, setCategories] = useState<StampTaxCategoryOption[]>([]);
  const [savingId, setSavingId] = useState<number | null>(null);
  const [isBulkSaving, setIsBulkSaving] = useState(false);

  useEffect(() => {
    void commands.listStampTaxCategories().then((result) => {
      if (result.status === "ok") setCategories(result.data);
    });
  }, []);

  const rows = review?.rows ?? [];

  async function saveSingle(row: StampTaxReviewRow, patch: StampTaxOverridePatch) {
    setSavingId(row.detailId);
    try {
      const result = await commands.saveStampTaxOverride({ periodFrom, periodTo, ...patch });
      if (result.status !== "ok") throw new Error(getErrorMessage(result.error));
      onReviewChange(result.data);
    } catch (error) {
      toast.add({ type: "error", title: "保存失败", description: getErrorMessage(error) });
    } finally {
      setSavingId(null);
    }
  }

  const toggleIncluded = (row: StampTaxReviewRow, included: boolean) =>
    void saveSingle(row, {
      detailId: row.detailId,
      included,
      stampTaxCategoryId: included ? manualCategoryId(row) : null,
      remark: row.remark,
    });

  const chooseCategory = (row: StampTaxReviewRow, value: string) =>
    void saveSingle(row, {
      detailId: row.detailId,
      included: true,
      stampTaxCategoryId: value === "" ? null : Number(value),
      remark: row.remark,
    });

  const commitRemark = (row: StampTaxReviewRow, remark: string | null) =>
    void saveSingle(row, {
      detailId: row.detailId,
      included: row.decisionManuallySet ? row.finalIncluded : null,
      stampTaxCategoryId: manualCategoryId(row),
      remark,
    });

  /** 三项全传 null，后端会删除该行，回到机器判定。 */
  const resetToSystem = (row: StampTaxReviewRow) =>
    void saveSingle(row, {
      detailId: row.detailId,
      included: null,
      stampTaxCategoryId: null,
      remark: null,
    });

  async function applyBulk(targets: StampTaxReviewRow[], action: BulkAction, clear: () => void) {
    if (targets.length === 0) return;

    const items: StampTaxOverridePatch[] = targets.map((row) =>
      action === "reset"
        ? { detailId: row.detailId, included: null, stampTaxCategoryId: null, remark: null }
        : {
            detailId: row.detailId,
            included: action === "include",
            stampTaxCategoryId: action === "include" ? manualCategoryId(row) : null,
            remark: row.remark,
          },
    );

    setIsBulkSaving(true);
    const toastId = toast.add({ type: "loading", title: "正在保存……" });

    try {
      const result = await commands.saveStampTaxOverridesBatch({ periodFrom, periodTo, items });
      if (result.status !== "ok") throw new Error(getErrorMessage(result.error));

      onReviewChange(result.data);
      clear();

      toast.update(toastId, {
        type: "success",
        title:
          action === "include"
            ? "已标记为计入印花税"
            : action === "exclude"
              ? "已标记为不计入印花税"
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

  // 列定义依赖 categories / savingId / isBulkSaving，变化时重建；DataTable 按列 id 记布局，不受影响。
  const columns = useMemo<Column[]>(() => {
    const busy = (row: StampTaxReviewRow) => savingId === row.detailId || isBulkSaving;

    return [
      { id: "taxPeriod", header: "做账期间", size: 90, accessorFn: (r) => r.taxPeriod },
      { id: "voucherNo", header: "凭证号", size: 120, accessorFn: (r) => r.voucherNo },
      { id: "invoiceNo", header: "发票号码", size: 190, accessorFn: (r) => r.invoiceNo },
      { id: "issueDate", header: "开票日期", size: 100, accessorFn: (r) => r.issueDate },
      { id: "sellerTaxNo", header: "销方识别号", size: 160, accessorFn: (r) => r.sellerTaxNo },
      { id: "sellerName", header: "销方名称", size: 200, accessorFn: (r) => r.sellerName },
      { id: "goodsName", header: "商品名称", size: 200, accessorFn: (r) => r.goodsName },
      { id: "taxCode", header: "税收分类编码", size: 150, accessorFn: (r) => r.taxCode ?? "" },
      {
        id: "goodsCategory",
        header: "税收分类编码简称",
        size: 150,
        accessorFn: (r) => r.goodsCategory ?? "",
      },
      { id: "taxRate", header: "税率", size: 70, accessorFn: (r) => r.taxRate ?? "" },
      {
        id: "amount",
        header: "金额",
        size: 110,
        accessorFn: (r) => r.amount,
        cell: ({ row }) => <span className="block text-right">{fenToYuan(row.original.amount)}</span>,
      },
      {
        id: "tax",
        header: "税额",
        size: 100,
        accessorFn: (r) => r.tax,
        cell: ({ row }) => <span className="block text-right">{fenToYuan(row.original.tax)}</span>,
      },
      {
        id: "total",
        header: "价税合计",
        size: 110,
        accessorFn: (r) => r.total,
        cell: ({ row }) => <span className="block text-right">{fenToYuan(row.original.total)}</span>,
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
        header: "计入印花税",
        size: 100,
        accessorFn: (r) => (r.finalIncluded ? "是" : "否"),
        cell: ({ row }) => (
          <Checkbox
            checked={row.original.finalIncluded}
            disabled={busy(row.original)}
            onCheckedChange={(checked) => toggleIncluded(row.original, checked === true)}
            aria-label="是否计入印花税计算"
          />
        ),
      },
      {
        id: "stampTaxCategory",
        header: "印花税类别",
        size: 210,
        accessorFn: (r) => (r.finalIncluded ? (r.stampTaxCategory ?? "待选类别") : ""),
        cell: ({ row }) => {
          const r = row.original;
          if (!r.finalIncluded) return null;
          return (
            <select
              className={`h-7 w-full rounded-md border bg-background px-1 text-xs ${
                r.needsCategoryChoice ? "border-destructive" : ""
              }`}
              value={r.stampTaxCategoryId ?? ""}
              disabled={busy(r)}
              onChange={(e) => chooseCategory(r, e.target.value)}
              title={
                r.needsCategoryChoice
                  ? r.matchedRules.length > 0
                    ? "命中多条规则，必须选定类别"
                    : "手工计入，必须选定类别"
                  : undefined
              }
              aria-label="印花税类别"
            >
              <option value="">请选择类别</option>
              {categories.map((c) => (
                <option key={c.id} value={c.id}>
                  {c.categoryName}（{rateText(c.rateBp)}）
                </option>
              ))}
            </select>
          );
        },
      },
      {
        id: "remark",
        header: "备注",
        size: 180,
        accessorFn: (r) => r.remark ?? "",
        cell: ({ row }) => (
          <RemarkCell row={row.original} disabled={busy(row.original)} onCommit={commitRemark} />
        ),
      },
    ];
    // toggleIncluded/chooseCategory/commitRemark 只依赖 periodFrom/periodTo/onReviewChange，随它们重建即可
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [categories, savingId, isBulkSaving, periodFrom, periodTo, onReviewChange]);

  return (
    <div className="space-y-2">
      <LayoutSelect layouts={layouts} />

      <DataTable
        layouts={layouts}
        columns={columns}
        data={rows}
        loading={isLoading}
        total={rows.length}
        rowKey={(r) => r.detailId}
        filterColumns={FILTER_COLUMNS}
        rowClassName={(r) => (r.decisionSource === "manual" ? "bg-sky-50/50 dark:bg-sky-500/5" : undefined)}
        renderBulkActions={(selected, clear) =>
          selected.length === 0 ? null : (
            <>
              <span className="text-xs font-medium">已选 {selected.length} 条</span>
              <Button
                type="button"
                size="sm"
                disabled={isBulkSaving}
                onClick={() => void applyBulk(selected, "include", clear)}
              >
                <Check className="mr-1 size-3.5" />
                计入
              </Button>
              <Button
                type="button"
                size="sm"
                variant="outline"
                disabled={isBulkSaving}
                onClick={() => void applyBulk(selected, "exclude", clear)}
              >
                <X className="mr-1 size-3.5" />
                不计入
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
          row.decisionSource === "manual" ? (
            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="h-7 px-2 text-xs"
              disabled={savingId === row.detailId || isBulkSaving}
              onClick={() => resetToSystem(row)}
            >
              <RotateCcw className="mr-1 size-3.5" />
              恢复
            </Button>
          ) : null
        }
      />
    </div>
  );
}
