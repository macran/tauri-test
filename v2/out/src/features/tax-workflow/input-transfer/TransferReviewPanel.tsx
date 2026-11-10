import { useEffect, useState } from "react";
import { toast } from "sonner";
import { ittApi } from "../api";
import { FlagBadge, MatchTypeBadge } from "../components/Badges";
import { ReviewShell } from "../components/ReviewShell";
import { Clip, type Column } from "../components/VirtualTable";
import { fen, parseYuanToFen } from "../format";
import type { TaxRun, TransferAction, TransferLine } from "../types";

interface Props {
  openRun?: TaxRun | null;
  onChanged?: () => void;
}

/** 转出税额单元格：失焦/回车提交 */
function TaxCell({ line, editable, onCommit }: { line: TransferLine; editable: boolean; onCommit: (fenValue: number) => void }) {
  const [v, setV] = useState(fen(line.finalTax));
  useEffect(() => setV(fen(line.finalTax)), [line.finalTax]);
  if (!editable || line.locked || !line.finalIncluded) {
    return <span className="tabular-nums">{line.finalIncluded ? fen(line.finalTax) : ""}</span>;
  }
  const commit = () => {
    const n = parseYuanToFen(v);
    if (n == null) {
      toast.error("请输入金额，最多两位小数");
      setV(fen(line.finalTax));
      return;
    }
    if (n !== line.finalTax) onCommit(n);
  };
  return (
    <input
      aria-label={`转出税额 ${line.invoiceNo} 第${line.lineNo}行`}
      className="h-7 w-full rounded border bg-background px-1 text-right tabular-nums focus-visible:outline-none focus-visible:ring-2 focus-visible:ring-ring"
      value={v}
      onChange={(e) => setV(e.target.value)}
      onBlur={commit}
      onKeyDown={(e) => {
        if (e.key === "Enter") (e.target as HTMLInputElement).blur();
        if (e.key === "Escape") setV(fen(line.finalTax));
      }}
    />
  );
}

export function TransferReviewPanel({ openRun, onChanged }: Props) {
  return (
    <ReviewShell<TransferLine, TransferAction>
      module="input_tax_transfer"
      moduleLabel="进项税转出"
      srcLabel="认证期间"
      includeWord="转出"
      taxLabel="转出税额"
      filePrefix="进项转出清单"
      openRun={openRun}
      onChanged={onChanged}
      match={ittApi.match}
      load={ittApi.listLines}
      applyOverride={ittApi.applyOverride}
      exportPending={ittApi.exportPending}
      confirm={ittApi.confirm}
      lineTax={(l) => l.finalTax}
      hasVoucher={(l) => l.hasVoucher}
      filters={["all", "included", "pending", "review", "manual", "nohit", "locked", "novoucher"]}
      columns={({ editable, apply }): Column<TransferLine>[] => [
        {
          key: "inc",
          header: "转出",
          width: 52,
          sticky: true,
          align: "center",
          cell: (l) => (
            <input
              type="checkbox"
              aria-label={`是否转出 ${l.invoiceNo} 第${l.lineNo}行`}
              checked={l.finalIncluded}
              disabled={!editable || l.locked}
              onChange={() => void apply([l.detailId], { type: l.finalIncluded ? "exclude" : "include" })}
            />
          ),
        },
        { key: "cp", header: "认证期间", width: 76, cell: (l) => l.certPeriod },
        { key: "no", header: "发票号码", width: 190, sticky: true, cell: (l) => <Clip>{l.invoiceCode ? `${l.invoiceCode}-${l.invoiceNo}` : l.invoiceNo}</Clip> },
        { key: "dt", header: "开票日期", width: 92, cell: (l) => l.issueDate },
        { key: "sl", header: "销售方", width: 200, cell: (l) => <Clip>{l.sellerName}</Clip> },
        { key: "st", header: "销售方税号", width: 168, cell: (l) => <Clip className="tabular-nums">{l.sellerTaxNo}</Clip> },
        { key: "ln", header: "行", width: 40, align: "right", cell: (l) => l.lineNo },
        { key: "gn", header: "货物或应税劳务名称", width: 220, cell: (l) => <Clip>{l.goodsName}</Clip> },
        { key: "tc", header: "税收编码", width: 160, cell: (l) => <Clip className="tabular-nums">{l.taxCode ?? ""}</Clip> },
        { key: "am", header: "金额", width: 100, align: "right", cell: (l) => <span className="tabular-nums">{fen(l.amount)}</span> },
        { key: "tr", header: "税率", width: 56, align: "right", cell: (l) => l.taxRate ?? "" },
        { key: "tx", header: "税额", width: 92, align: "right", cell: (l) => <span className="tabular-nums">{fen(l.tax)}</span> },
        {
          key: "ft",
          header: "转出税额",
          width: 108,
          align: "right",
          cell: (l) => (
            <TaxCell line={l} editable={editable} onCommit={(t) => void apply([l.detailId], { type: "setTax", tax: t })} />
          ),
        },
        {
          key: "mt",
          header: "匹配类型",
          width: 230,
          cell: (l) => (
            <span className="flex items-center gap-1">
              <MatchTypeBadge type={l.matchType} locked={l.locked} />
              {l.matchedInOtherPeriod && <span className="inline-flex rounded px-1.5 py-0.5 text-xs font-medium bg-muted text-muted-foreground">其他所属期已命中</span>}
              {l.needsReview && <FlagBadge tone="warn">待复核</FlagBadge>}
            </span>
          ),
        },
        { key: "hr", header: "命中规则", width: 220, cell: (l) => <Clip>{l.hitRules ?? ""}</Clip> },
        {
          key: "vn",
          header: "原记账凭证",
          width: 170,
          cell: (l) => (l.hasVoucher ? <Clip>{l.voucherNos}</Clip> : <FlagBadge tone="error">无凭证</FlagBadge>),
        },
        { key: "vd", header: "原记账日期", width: 100, cell: (l) => <Clip>{l.voucherDates ?? ""}</Clip> },
        { key: "bn", header: "批次", width: 50, align: "center", cell: (l) => l.confirmedBatchNo ?? "" },
        {
          key: "rm",
          header: "备注",
          width: 200,
          cell: (l) => <Clip>{l.remark ? `${l.remark}${l.updatedBy ? `（${l.updatedBy}）` : ""}` : ""}</Clip>,
        },
      ]}
    />
  );
}
