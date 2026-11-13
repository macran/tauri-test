// src/components/query/overview/ItemDrawer.tsx
//
// 点击 item 编号打开的"关联记录"侧栏：一行一条关联记录（不同类型之间没有配对关系），
// 展开后列出该记录组的全部字段；底部是该期间的税务结论。

import type { ItemOverviewRow } from "@/bindings";
import { formatMoney } from "@/features/query/format";
import {
  Drawer,
  DrawerContent,
  DrawerHeader,
  DrawerTitle,
} from "@/components/ui/drawer";
import { COLS, GROUPS, hueStyle, type Rec } from "./columns";

const REV_DIR: Record<string, string> = {
  "1": "作为红字",
  "2": "作为蓝字被冲",
};

interface Entry {
  group: string;
  label: string;
  rec: Rec;
  summary: string;
}

function entries(it: ItemOverviewRow): Entry[] {
  const { item, groups } = it;
  const out: Entry[] = [];
  const add = (group: string, label: string, rec: Rec, summary: string) =>
    out.push({ group, label, rec, summary });
  if (item.cert_status === "1")
    add("cert", "认证", item, `${item.tax_period} · ${item.check_time}`);
  for (const r of groups.ap ?? [])
    add(
      "ap",
      "AP 凭证",
      r,
      `${r.voucher_no} · ${r.vendor_code} · ${r.tax_rate} · ${formatMoney(r.amount)}`
    );
  for (const r of groups.ic ?? [])
    add("ic", "IC 凭证", r, `${r.document_no} · ${r.legal_entity}`);
  if (item.bank_voucher_no)
    add(
      "bank",
      "Bank 凭证",
      item,
      `${item.bank_voucher_no} · ${item.bank_posted_date}`
    );
  const sp = `sp${item.biz_type}`;
  for (const r of groups[sp] ?? []) {
    const head = COLS.filter((c) => c.group === sp && c.id !== `${sp}_line_no`)
      .slice(0, 3)
      .map((c) => c.fmt(c.get(r, it)));
    add(sp, GROUPS[sp].label, r, `行 ${r.line_no} · ${head.join(" · ")}`);
  }
  for (const r of groups.rev ?? [])
    add(
      "rev",
      "红冲匹配",
      r,
      `${REV_DIR[r.dir ?? ""]} · ${r.cp_inv ?? "—"}${r.cp_line ? ` 行${r.cp_line}` : ""} · ${formatMoney(r.matched_amount)}`
    );
  for (const r of groups.wth ?? [])
    add("wth", "转出规则", r, `#${r.rule_id} ${r.rule_category}`);
  for (const r of groups.sth ?? [])
    add(
      "sth",
      "印花税规则",
      r,
      `#${r.rule_id} ${r.category_name} · ${r.rule_category}`
    );
  return out;
}

function RecordCard({
  it,
  group,
  label,
  rec,
  summary,
  open,
}: Entry & { it: ItemOverviewRow; open?: boolean }) {
  return (
    <details
      open={open}
      className="my-2 rounded-sm border border-l-[3px] border-l-(--hue) bg-background"
      style={hueStyle(group)}
    >
      <summary className="flex cursor-pointer items-baseline gap-2 px-2.5 py-1.5">
        <span className="min-w-20 font-semibold text-(--hue)">{label}</span>
        <span className="truncate font-mono">{summary}</span>
      </summary>
      <dl className="grid grid-cols-[auto_1fr] gap-x-3 gap-y-0.5 px-3 pt-1 pb-2.5">
        {COLS.filter((c) => c.group === group).map((c) => (
          <div key={c.id} className="contents">
            <dt className="text-muted-foreground">{c.label}</dt>
            <dd className="break-all">{c.fmt(c.get(rec, it))}</dd>
          </div>
        ))}
      </dl>
    </details>
  );
}

export function ItemDrawer({
  it,
  onClose,
}: {
  it: ItemOverviewRow | null;
  onClose: () => void;
}) {
  const list = it ? entries(it) : [];
  const i = it?.item;
  const tax = (g: "wt" | "st") => {
    const final = COLS.find((c) => c.id === `${g}_final`);
    const amount = COLS.find(
      (c) => c.id === (g === "wt" ? "wt_final_tax" : "st_amount")
    );
    return it && final && amount
      ? `${final.fmt(final.get(it.item, it))} · ${amount.fmt(amount.get(it.item, it))}`
      : "";
  };

  return (
    <Drawer
      open={!!it}
      onOpenChange={(o) => !o && onClose()}
      swipeDirection="right"
    >
      <DrawerContent className="h-dvh w-[min(440px,100vw)]">
        <DrawerHeader className="border-b pb-3">
          <DrawerTitle>关联记录</DrawerTitle>
        </DrawerHeader>
        {it && i && (
          <div className="thin-scrollbar flex-1 overflow-y-auto px-4 py-2">
            <div className="border-b pb-3">
              <div className="font-mono text-[15px] font-medium">
                {i.inv_unq_key} · 行 {i.line_no}
              </div>
              <div className="text-muted-foreground">
                {i.goods_name} · 金额 {formatMoney(i.amount)} · 税额{" "}
                {formatMoney(i.tax)} · {i.seller_name}
              </div>
            </div>
            <div className="mt-4 text-[11px] font-semibold text-muted-foreground">
              一行一条关联记录 · 共 {list.length} 条（不同类型之间没有配对关系）
            </div>
            {list.length ? (
              list.map((e, k) => <RecordCard key={k} it={it} {...e} />)
            ) : (
              <p className="mt-2 text-muted-foreground">
                该 item 没有关联记录。
              </p>
            )}
            <div className="mt-4 text-[11px] font-semibold text-muted-foreground">
              税务结果
            </div>
            <RecordCard
              it={it}
              group="wt"
              label="进项税转出"
              rec={i}
              summary={tax("wt")}
              open
            />
            <RecordCard
              it={it}
              group="st"
              label="印花税"
              rec={i}
              summary={tax("st")}
              open
            />
          </div>
        )}
      </DrawerContent>
    </Drawer>
  );
}
