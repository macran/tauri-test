import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { ittApi, pickSavePath } from "../api";
import { MatchTypeBadge } from "../components/Badges";
import { PeriodRangeInput } from "../components/PeriodRangeInput";
import { Clip, type Column, VirtualTable } from "../components/VirtualTable";
import { currentPeriod, fen, periodLabel } from "../format";
import type { ConfirmedQuery, ConfirmedTransferRow, PeriodRange } from "../types";

const cols: Column<ConfirmedTransferRow>[] = [
  { key: "fp", header: "纳税所属期", width: 110, sticky: true, cell: (r) => periodLabel(r.filingPeriod) },
  { key: "cp", header: "认证期间", width: 76, cell: (r) => r.certPeriod },
  { key: "bn", header: "批次", width: 48, align: "center", cell: (r) => r.batchNo },
  { key: "av", header: "会计凭证号", width: 120, cell: (r) => <Clip>{r.acctVoucherNo ?? ""}</Clip> },
  { key: "ad", header: "入账日期", width: 92, cell: (r) => r.acctPostedDate ?? "" },
  { key: "ap", header: "做账人", width: 80, cell: (r) => <Clip>{r.acctPreparer ?? ""}</Clip> },
  { key: "no", header: "发票号码", width: 190, cell: (r) => <Clip>{r.invoiceCode ? `${r.invoiceCode}-${r.invoiceNo}` : r.invoiceNo}</Clip> },
  { key: "dt", header: "开票日期", width: 92, cell: (r) => r.issueDate },
  { key: "sl", header: "销售方", width: 200, cell: (r) => <Clip>{r.sellerName}</Clip> },
  { key: "st", header: "销售方税号", width: 168, cell: (r) => <Clip>{r.sellerTaxNo}</Clip> },
  { key: "ln", header: "行", width: 40, align: "right", cell: (r) => r.lineNo },
  { key: "gn", header: "货物或应税劳务名称", width: 220, cell: (r) => <Clip>{r.goodsName}</Clip> },
  { key: "am", header: "金额", width: 100, align: "right", cell: (r) => <span className="tabular-nums">{fen(r.amount)}</span> },
  { key: "tx", header: "税额", width: 92, align: "right", cell: (r) => <span className="tabular-nums">{fen(r.detailTax)}</span> },
  { key: "tt", header: "转出税额", width: 100, align: "right", cell: (r) => <span className="font-medium tabular-nums">{fen(r.transferTax)}</span> },
  { key: "mt", header: "匹配类型", width: 130, cell: (r) => <MatchTypeBadge type={r.matchType} /> },
  { key: "hr", header: "命中规则", width: 220, cell: (r) => <Clip>{r.hitRules ?? ""}</Clip> },
  { key: "vn", header: "原记账凭证", width: 170, cell: (r) => <Clip>{r.voucherNos ?? ""}</Clip> },
  { key: "vd", header: "原记账日期", width: 100, cell: (r) => <Clip>{r.voucherDates ?? ""}</Clip> },
  { key: "rm", header: "备注", width: 180, cell: (r) => <Clip>{r.remark ?? ""}</Clip> },
  { key: "cb", header: "确认", width: 190, cell: (r) => <Clip>{`${r.confirmedAt} ${r.confirmedBy}`}</Clip> },
];

/** 任意纳税所属期的已确认进项转出明细 */
export function TransferConfirmedQuery() {
  const [filing, setFiling] = useState<PeriodRange>(() => ({ from: currentPeriod(-12), to: currentPeriod(0) }));
  const [useCert, setUseCert] = useState(false);
  const [cert, setCert] = useState<PeriodRange>(() => ({ from: currentPeriod(-1), to: currentPeriod(-1) }));
  const [keyword, setKeyword] = useState("");
  const [rows, setRows] = useState<ConfirmedTransferRow[]>([]);
  const [loading, setLoading] = useState(false);

  const query = (): ConfirmedQuery => ({
    filingFrom: filing.from,
    filingTo: filing.to,
    srcFrom: useCert ? cert.from : null,
    srcTo: useCert ? cert.to : null,
    keyword: keyword.trim() || null,
  });

  const search = async () => {
    setLoading(true);
    try {
      setRows(await ittApi.queryConfirmed(query()));
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setLoading(false);
    }
  };

  const total = rows.reduce((s, r) => s + r.transferTax, 0);

  return (
    <div className="flex min-h-0 flex-1 flex-col gap-3">
      <form
        className="flex flex-wrap items-center gap-3"
        onSubmit={(e) => {
          e.preventDefault();
          void search();
        }}
      >
        <PeriodRangeInput label="纳税所属期" value={filing} onChange={setFiling} />
        <label className="flex items-center gap-1 text-sm">
          <input type="checkbox" checked={useCert} onChange={(e) => setUseCert(e.target.checked)} />
          按认证期间
        </label>
        {useCert && <PeriodRangeInput label="认证期间" value={cert} onChange={setCert} />}
        <Input className="h-8 w-60" placeholder="发票号 / 销方 / 商品 / 会计凭证号" value={keyword} onChange={(e) => setKeyword(e.target.value)} />
        <Button type="submit" disabled={loading}>
          {loading ? "查询中…" : "查询"}
        </Button>
        <Button
          type="button"
          variant="outline"
          disabled={rows.length === 0}
          onClick={async () => {
            const path = await pickSavePath(`已确认进项转出_${periodLabel(filing)}.xlsx`);
            if (!path) return;
            try {
              const r = await ittApi.exportConfirmedQuery(query(), path);
              toast.success(`已导出 ${r.lineCount} 行`);
            } catch (e) {
              toast.error((e as Error).message);
            }
          }}
        >
          导出 Excel
        </Button>
        <span className="ml-auto text-sm text-muted-foreground">
          {rows.length} 行 · 转出税额合计 <b className="tabular-nums text-foreground">{fen(total)}</b> 元
        </span>
      </form>
      <VirtualTable className="min-h-[320px] flex-1" rows={rows} columns={cols} rowKey={(r) => r.detailId} empty="选择条件后点击查询" />
    </div>
  );
}
