import { useState } from "react";
import { Button } from "@/components/ui/button";
import { stApi } from "../api";
import { FlagBadge, MatchTypeBadge } from "../components/Badges";
import { ReviewShell } from "../components/ReviewShell";
import { Clip, type Column } from "../components/VirtualTable";
import { fen, ratePpmLabel } from "../format";
import type { StampAction, StampLine, TaxRun } from "../types";
import { CategorySelect, useStampCategories } from "./useStampCategories";

export function StampReviewPanel({ openRun, onChanged }: { openRun?: TaxRun | null; onChanged?: () => void }) {
  const { groups, byCode } = useStampCategories();
  const [bulkCat, setBulkCat] = useState<string | null>(null);

  return (
    <ReviewShell<StampLine, StampAction>
      module="stamp_tax"
      moduleLabel="印花税"
      srcLabel="凭证期间"
      includeWord="计入"
      taxLabel="印花税额"
      filePrefix="印花税清单"
      openRun={openRun}
      onChanged={onChanged}
      match={stApi.match}
      load={stApi.listLines}
      applyOverride={stApi.applyOverride}
      exportPending={stApi.exportPending}
      confirm={stApi.confirm}
      lineTax={(l) => l.stampTax}
      invalid={(l) => l.categoryInvalid}
      filters={["all", "included", "pending", "review", "invalid", "manual", "nohit", "locked"]}
      extraBulk={({ selectedIds, apply }) => (
        <span className="flex items-center gap-1">
          <CategorySelect
            ariaLabel="批量指定子目"
            className="h-8 w-56 rounded-md border bg-background px-2 text-sm"
            value={bulkCat}
            groups={groups}
            onChange={setBulkCat}
          />
          <Button
            size="sm"
            variant="outline"
            disabled={!bulkCat}
            onClick={() => bulkCat && void apply(selectedIds, { type: "setCategory", categoryCode: bulkCat })}
          >
            指定子目
          </Button>
        </span>
      )}
      columns={({ editable, apply }): Column<StampLine>[] => [
        {
          key: "inc",
          header: "计入",
          width: 52,
          sticky: true,
          align: "center",
          cell: (l) => (
            <input
              type="checkbox"
              aria-label={`是否计入印花税 ${l.invoiceNo} 第${l.lineNo}行`}
              checked={l.finalIncluded}
              disabled={!editable || l.locked}
              onChange={() => void apply([l.detailId], { type: l.finalIncluded ? "exclude" : "include" })}
            />
          ),
        },
        { key: "no", header: "发票号码", width: 190, sticky: true, cell: (l) => <Clip>{l.invoiceCode ? `${l.invoiceCode}-${l.invoiceNo}` : l.invoiceNo}</Clip> },
        { key: "dt", header: "开票日期", width: 92, cell: (l) => l.issueDate },
        { key: "sl", header: "销售方", width: 200, cell: (l) => <Clip>{l.sellerName}</Clip> },
        { key: "ln", header: "行", width: 40, align: "right", cell: (l) => l.lineNo },
        { key: "gn", header: "货物或应税劳务名称", width: 220, cell: (l) => <Clip>{l.goodsName}</Clip> },
        { key: "am", header: "计税金额", width: 110, align: "right", cell: (l) => <span className="tabular-nums">{fen(l.amount)}</span> },
        {
          key: "cat",
          header: "印花税子目",
          width: 230,
          cell: (l) =>
            l.locked || !editable || !l.finalIncluded ? (
              <Clip>{l.finalCategory ? `${l.itemName ?? ""}${l.subItemName ? ` / ${l.subItemName}` : ""}` : ""}</Clip>
            ) : (
              <CategorySelect
                ariaLabel={`印花税子目 ${l.invoiceNo} 第${l.lineNo}行`}
                value={l.finalCategory && byCode.get(l.finalCategory)?.ratePpm != null ? l.finalCategory : null}
                groups={groups}
                onChange={(code) => void apply([l.detailId], { type: "setCategory", categoryCode: code })}
              />
            ),
        },
        { key: "rt", header: "税率", width: 70, align: "right", cell: (l) => ratePpmLabel(l.ratePpm) },
        { key: "tx", header: "印花税额", width: 96, align: "right", cell: (l) => <span className="font-medium tabular-nums">{l.finalIncluded ? fen(l.stampTax) : ""}</span> },
        {
          key: "mt",
          header: "匹配类型",
          width: 250,
          cell: (l) => (
            <span className="flex items-center gap-1">
              <MatchTypeBadge type={l.matchType} locked={l.locked} />
              {l.matchedInOtherPeriod && <span className="inline-flex rounded px-1.5 py-0.5 text-xs font-medium bg-muted text-muted-foreground">其他所属期已命中</span>}
              {l.needsReview && <FlagBadge tone="warn">{l.hitCategories.length > 1 ? "类别冲突" : "待复核"}</FlagBadge>}
              {l.categoryInvalid && <FlagBadge tone="error">缺子目</FlagBadge>}
            </span>
          ),
        },
        { key: "hr", header: "命中规则", width: 220, cell: (l) => <Clip>{l.hitRules ?? ""}</Clip> },
        { key: "vn", header: "凭证", width: 170, cell: (l) => <Clip>{l.voucherNos ?? ""}</Clip> },
        { key: "vd", header: "入账日期", width: 100, cell: (l) => <Clip>{l.voucherDates ?? ""}</Clip> },
        { key: "po", header: "PO", width: 110, cell: (l) => <Clip>{l.poNos ?? ""}</Clip> },
        { key: "cc", header: "成本中心", width: 100, cell: (l) => <Clip>{l.costCenters ?? ""}</Clip> },
        { key: "bn", header: "批次", width: 50, align: "center", cell: (l) => l.confirmedBatchNo ?? "" },
        { key: "rm", header: "备注", width: 200, cell: (l) => <Clip>{l.remark ?? ""}</Clip> },
      ]}
    />
  );
}
