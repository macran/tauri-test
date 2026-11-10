import { useState } from "react";
import { toast } from "sonner";
import { Button } from "@/components/ui/button";
import { Input } from "@/components/ui/input";
import { pickSavePath, stApi } from "../api";
import { MatchTypeBadge } from "../components/Badges";
import { PeriodRangeInput } from "../components/PeriodRangeInput";
import { Clip, type Column, VirtualTable } from "../components/VirtualTable";
import { currentPeriod, fen, periodLabel, ratePpmLabel } from "../format";
import type { ConfirmedStampRow, PeriodRange, StampConfirmedQuery as Q } from "../types";
import { useStampCategories } from "./useStampCategories";

const cols: Column<ConfirmedStampRow>[] = [
  { key: "fp", header: "纳税所属期", width: 110, sticky: true, cell: (r) => periodLabel(r.filingPeriod) },
  { key: "vp", header: "凭证期间", width: 110, cell: (r) => r.voucherPeriods.map(periodLabel).join("；") },
  { key: "bn", header: "批次", width: 48, align: "center", cell: (r) => r.batchNo },
  { key: "av", header: "会计凭证号", width: 120, cell: (r) => <Clip>{r.acctVoucherNo ?? ""}</Clip> },
  { key: "ad", header: "入账日期", width: 92, cell: (r) => r.acctPostedDate ?? "" },
  { key: "no", header: "发票号码", width: 190, cell: (r) => <Clip>{r.invoiceCode ? `${r.invoiceCode}-${r.invoiceNo}` : r.invoiceNo}</Clip> },
  { key: "dt", header: "开票日期", width: 92, cell: (r) => r.issueDate },
  { key: "sl", header: "销售方", width: 200, cell: (r) => <Clip>{r.sellerName}</Clip> },
  { key: "gn", header: "货物或应税劳务名称", width: 220, cell: (r) => <Clip>{r.goodsName}</Clip> },
  { key: "it", header: "税目", width: 140, cell: (r) => <Clip>{r.itemName}</Clip> },
  { key: "si", header: "子目", width: 140, cell: (r) => <Clip>{r.subItemName ?? ""}</Clip> },
  { key: "rt", header: "税率", width: 70, align: "right", cell: (r) => ratePpmLabel(r.ratePpm) },
  { key: "ba", header: "计税金额", width: 110, align: "right", cell: (r) => <span className="tabular-nums">{fen(r.baseAmount)}</span> },
  { key: "tx", header: "印花税额", width: 96, align: "right", cell: (r) => <span className="font-medium tabular-nums">{fen(r.stampTax)}</span> },
  { key: "mt", header: "匹配类型", width: 130, cell: (r) => <MatchTypeBadge type={r.matchType} /> },
  { key: "hr", header: "命中规则", width: 200, cell: (r) => <Clip>{r.hitRules ?? ""}</Clip> },
  { key: "vn", header: "原凭证", width: 170, cell: (r) => <Clip>{r.voucherNos ?? ""}</Clip> },
  { key: "po", header: "PO", width: 110, cell: (r) => <Clip>{r.poNos ?? ""}</Clip> },
  { key: "cc", header: "成本中心", width: 100, cell: (r) => <Clip>{r.costCenters ?? ""}</Clip> },
  { key: "rm", header: "备注", width: 180, cell: (r) => <Clip>{r.remark ?? ""}</Clip> },
];

export function StampConfirmedQuery() {
  const { groups } = useStampCategories();
  const [filing, setFiling] = useState<PeriodRange>(() => ({ from: currentPeriod(-12), to: currentPeriod(0) }));
  const [category, setCategory] = useState("");
  const [keyword, setKeyword] = useState("");
  const [rows, setRows] = useState<ConfirmedStampRow[]>([]);
  const [loading, setLoading] = useState(false);

  const query = (): Q => ({ filingFrom: filing.from, filingTo: filing.to, categoryCode: category || null, keyword: keyword.trim() || null });

  const search = async () => {
    setLoading(true);
    try {
      setRows(await stApi.queryConfirmed(query()));
    } catch (e) {
      toast.error((e as Error).message);
    } finally {
      setLoading(false);
    }
  };

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
        <select aria-label="税目/子目" className="h-8 rounded-md border bg-background px-2 text-sm" value={category} onChange={(e) => setCategory(e.target.value)}>
          <option value="">全部税目</option>
          {groups.map((g) => (
            <optgroup key={g.parent.code} label={g.parent.name}>
              <option value={g.parent.code}>{g.parent.name}（全部子目）</option>
              {g.children
                .filter((c) => c.code !== g.parent.code)
                .map((c) => (
                  <option key={c.code} value={c.code}>
                    {c.name}
                  </option>
                ))}
            </optgroup>
          ))}
        </select>
        <Input className="h-8 w-60" placeholder="发票号 / 销方 / 商品 / 会计凭证号" value={keyword} onChange={(e) => setKeyword(e.target.value)} />
        <Button type="submit" disabled={loading}>
          {loading ? "查询中…" : "查询"}
        </Button>
        <Button
          type="button"
          variant="outline"
          disabled={rows.length === 0}
          onClick={async () => {
            const path = await pickSavePath(`已确认印花税_${periodLabel(filing)}.xlsx`);
            if (!path) return;
            try {
              const r = await stApi.exportConfirmedQuery(query(), path);
              toast.success(`已导出 ${r.lineCount} 行`);
            } catch (e) {
              toast.error((e as Error).message);
            }
          }}
        >
          导出 Excel
        </Button>
        <span className="ml-auto text-sm text-muted-foreground">
          {rows.length} 行 · 计税金额 {fen(rows.reduce((s, r) => s + r.baseAmount, 0))} · 印花税{" "}
          <b className="tabular-nums text-foreground">{fen(rows.reduce((s, r) => s + r.stampTax, 0))}</b> 元
        </span>
      </form>
      <VirtualTable className="min-h-[320px] flex-1" rows={rows} columns={cols} rowKey={(r) => r.detailId} empty="选择条件后点击查询" />
    </div>
  );
}
