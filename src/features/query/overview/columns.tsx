// src/components/query/overview/columns.tsx
//
// 综合查询的列注册表。后端返回的是「列名 → 文本值」的 map（见 query_dto.rs 的 ItemOverviewRow），
// 标签、格式、哪些列属于哪个记录组都只在这里定义。
//
//  - 单值组（item/header/...）：取 row.it.item 里的键
//  - 多值组（ap/ic/sp*/rev/wth/sth）：取 row.it.groups[组] 里每条记录的键；
//    汇总模式下多条值用 " / " 连接，展开模式下该组逐条一行

import type { CSSProperties } from "react";
import type { ItemOverviewRow } from "@/bindings";
import { displayOrDash, formatMoney } from "@/features/query/format";
import {
  AP_INV_TYPE_OPTIONS,
  CERT_RISK_STATUS_OPTIONS,
  type EnumOption,
  enumLabel,
  INV_SOURCE_OPTIONS,
  INV_STATE_OPTIONS,
  INV_TYPE_OPTIONS,
  IS_POSITIVE_OPTIONS,
  RISK_LEVEL_OPTIONS,
  SPECIAL_BIZ_TYPE_OPTIONS,
  TAX_CERTIFIED_OPTIONS,
} from "@/lib/enum-options";

export type Rec = Partial<Record<string, string | null>>;

/** 表格的一行：汇总模式 = 一个 item；展开模式 = item × 一条记录 */
export interface OvRow {
  key: string;
  it: ItemOverviewRow;
  /** 当前展开的记录组（"sp" 表示该 item 的特种发票记录）；汇总模式为 null */
  expand: string | null;
  rec?: Rec;
  /** 展开模式下这条记录在组内的序号 */
  idx?: number;
  /** 展开模式下，该 item 在展开组里没有记录（"保留无记录的 item"时出现） */
  none?: boolean;
}

type Fmt = (v: string | null | undefined) => string;
const text: Fmt = displayOrDash;
const money: Fmt = formatMoney;
const ENUM_LABELS = new Map<Fmt, Record<string, string>>();
const enumFmt = (opts: EnumOption[]): Fmt => {
  const f: Fmt = (v) => (v == null ? "—" : enumLabel(opts, Number(v)));
  ENUM_LABELS.set(f, Object.fromEntries(opts.map((o) => [String(o.value), o.label])));
  return f;
};
const yesNo = (yes = "是", no = "否"): Fmt => {
  const f: Fmt = (v) => (v == null ? "—" : v === "1" ? yes : no);
  ENUM_LABELS.set(f, { "1": yes, "0": no });
  return f;
};
const ratePpm: Fmt = (v) => (v == null ? "—" : `${Number(v) / 10000}%`);

// ───────── 记录组 ─────────
// hue：n 中性 / v 凭证 / s 特种发票 / r 红冲 / t 税务（色带与高亮的主题色）
type Hue = "n" | "v" | "s" | "r" | "t";
interface Group {
  label: string;
  /** 基数说明，显示在色带和列设置里 */
  card: string;
  hue: Hue;
  multi?: boolean;
  /** 特种发票明细组（只有对应 biz_type 的 item 才有记录） */
  sp?: boolean;
}
const SP_NAMES = Object.fromEntries(
  SPECIAL_BIZ_TYPE_OPTIONS.map((o) => [o.value, o.label])
);
const SP_TABLES = [1, 2, 3, 6, 7, 8, 9]; // 与后端 SP_TABLES 一致

// 对象键顺序 = 列设置面板里的分组顺序
export const GROUPS: Record<string, Group> = {
  item: { label: "发票明细", card: "1", hue: "n" },
  header: { label: "发票头与销方", card: "1 · 发票级", hue: "n" },
  cert: { label: "认证与差旅", card: "0–1 · 发票级", hue: "n" },
  vsum: { label: "凭证概况", card: "汇总", hue: "v" },
  ap: { label: "AP 凭证", card: "0–n · 发票级", hue: "v", multi: true },
  ic: { label: "IC 凭证", card: "0–n · 发票级", hue: "v", multi: true },
  bank: { label: "Bank 凭证", card: "0–1 · 发票级", hue: "v" },
  spsum: { label: "特种发票", card: "汇总", hue: "s" },
  ...Object.fromEntries(
    SP_TABLES.map((n) => [
      `sp${n}`,
      {
        label: SP_NAMES[n],
        card: "0–n · 发票级",
        hue: "s",
        multi: true,
        sp: true,
      },
    ])
  ),
  rev: { label: "红冲匹配", card: "0–n · item级", hue: "r", multi: true },
  wt: { label: "进项税转出", card: "1 · item级 · 按期间", hue: "t" },
  wth: { label: "转出命中规则", card: "0–n", hue: "t", multi: true },
  st: { label: "印花税", card: "1 · item级 · 按期间", hue: "t" },
  sth: { label: "印花税命中规则", card: "0–n", hue: "t", multi: true },
};

const HUES: Record<Hue, [string, string]> = {
  n: ["#4A5560", "#ECEFF1"],
  v: ["#1E6D66", "#E3F0EE"],
  s: ["#8A5808", "#F5EBDA"],
  r: ["#A0374A", "#F6E3E7"],
  t: ["#4F6A22", "#E8EFDC"],
};
/** 设置 --hue / --bandbg 两个 CSS 变量，色带、序号和高亮都读它们 */
export const hueStyle = (group: string): CSSProperties =>
  ({
    "--hue": HUES[GROUPS[group].hue][0],
    "--bandbg": HUES[GROUPS[group].hue][1],
  }) as CSSProperties;

/** 可作为"按记录展开"的组 */
export const EXPAND_OPTIONS = [
  { value: "ap", label: "AP 凭证" },
  { value: "ic", label: "IC 凭证" },
  { value: "sp", label: "特种发票记录" },
  { value: "rev", label: "红冲匹配" },
  { value: "wth", label: "转出命中规则" },
  { value: "sth", label: "印花税命中规则" },
];

/** 展开时该 item 要展开的记录；undefined = 本组不适用于此 item */
export function expandRecs(
  it: ItemOverviewRow,
  expand: string
): Rec[] | undefined {
  return it.groups[expand === "sp" ? `sp${it.item.biz_type}` : expand];
}

export const isExpanded = (g: string, expand: string | null) =>
  expand !== null && (expand === "sp" ? !!GROUPS[g].sp : g === expand);

// ───────── 列定义 ─────────
export interface ColDef {
  id: string;
  group: string;
  label: string;
  fmt: Fmt;
  size: number;
  align: boolean;
  /** 取值；r 是单值组的 item 或多值组的一条记录 */
  get: (r: Rec, it: ItemOverviewRow) => string | null | undefined;
  /** 导出：直接取 item 里同名键的列才能由后端导出；自定义取值的列（拼接、汇总、税务“未计算”）没有 */
  exportKey?: string;
  isMoney?: boolean;
  /** 导出：枚举值 → 中文 */
  labels?: Record<string, string>;
}

export const COLS: ColDef[] = [];

function add(
  group: string,
  key: string,
  label: string,
  fmt: Fmt = text,
  size = 0,
  get?: ColDef["get"]
) {
  const money_ = fmt === money;
  const labels = ENUM_LABELS.get(fmt);
  COLS.push({
    exportKey: !get && !GROUPS[group].multi ? key : undefined,
    isMoney: money_,
    labels,
    id: `${group}_${key}`,
    group,
    label,
    fmt,
    size: size || (money_ ? 110 : 130),
    align: money_ || fmt === ratePpm,
    get: get ?? ((r) => r[key]),
  });
}

/** 税务单值列：该期间未计算时统一显示"未计算" */
const NOT_COMPUTED = "未计算";
function addTax(
  group: "wt" | "st",
  key: string,
  label: string,
  fmt: Fmt,
  get?: ColDef["get"]
) {
  add(
    group,
    key,
    label,
    (v) => (v === NOT_COMPUTED ? v : fmt(v)),
    0,
    (r, it) =>
      r[`${group}_computed`] === "1"
        ? (get ?? ((x) => x[`${group}_${key}`]))(r, it)
        : NOT_COMPUTED
  );
  const last = COLS[COLS.length - 1];
  last.align = fmt === money || fmt === ratePpm;
  last.isMoney = fmt === money;
  last.labels = ENUM_LABELS.get(fmt);
  // 未计算时后端 item 里没有该键，导出为空
  last.exportKey = get ? undefined : `${group}_${key}`;
}

// 发票头（发票号码放最前）
add("header", "inv_unq_key", "发票号码", text, 190);
for (const [k, l, w] of [
  ["code", "发票代码", 110],
  ["num", "发票号码（旧）", 100],
  ["e_num", "数电号码", 170],
  ["issue_date", "开票日期", 100],
] as const)
  add("header", k, l, text, w);
add("header", "inv_type", "发票类型", enumFmt(INV_TYPE_OPTIONS), 200);
add("header", "state", "发票状态", enumFmt(INV_STATE_OPTIONS), 100);
add("header", "is_pos", "蓝 / 红字", enumFmt(IS_POSITIVE_OPTIONS), 100);
add("header", "source", "来源", enumFmt(INV_SOURCE_OPTIONS), 150);
add("header", "risk", "风险", enumFmt(RISK_LEVEL_OPTIONS), 90);
for (const [k, l] of [
  ["issuer", "开票人"],
  ["remark", "发票备注"],
  ["rev_org_key", "被红冲蓝字号码"],
  ["red_notice_no", "红字信息表编号"],
] as const)
  add("header", k, l);
add("header", "h_amount", "发票金额", money);
add("header", "h_tax", "发票税额", money);
add("header", "h_total", "发票价税合计", money);

// 发票明细
add("item", "line_no", "行号", text, 70);
add("item", "goods_name", "货物或服务名称", text, 260, (r) =>
  r.goods_category ? `*${r.goods_category}*${r.goods_name}` : r.goods_name
);
COLS[COLS.length - 1].exportKey = "goods_name"; // 展示是 *类别*名称，导出只取名称，类别另有一列
add("item", "goods_category", "商品类别");
add("item", "tax_code", "税收分类编码", text, 170);
add("item", "spec_model", "规格型号");
add("item", "unit", "单位", text, 80);
add("item", "quantity", "数量", text, 90);
add("item", "unit_price", "单价", text, 100);
add("item", "amount", "金额", money);
add("item", "tax_rate", "税率", text, 80);
add("item", "tax", "税额", money);
add("item", "total", "价税合计", money);
add("item", "biz_type", "特种业务类型", enumFmt(SPECIAL_BIZ_TYPE_OPTIONS), 170);

// 销方（发票头的 N:1 属性，并入发票头一组）
add("header", "seller_name", "销方名称", text, 220);
add("header", "seller_tax_no", "销方税号", text, 170);

// 认证 / 差旅（certified_list）
add("cert", "cert_status", "认证状态", yesNo("已勾选", "未勾选"), 100);
for (const [k, l] of [
  ["tax_period", "所属税期"],
  ["check_time", "勾选时间"],
  ["invoice_tag", "发票标签"],
  ["domestic_proof_no", "国内凭证号"],
] as const)
  add("cert", k, l);
add("cert", "valid_deduct_amount", "有效抵扣税额", money);
add("cert", "risk_status", "风险状态", enumFmt(CERT_RISK_STATUS_OPTIONS));
add("cert", "is_travel_expense", "差旅费用", yesNo());
add("cert", "travel_voucher_no", "差旅凭证号");
add("cert", "travel_posted_date", "差旅入账日期");

// 凭证概况：AP / IC / Bank / 差旅 汇总
const vouchers = (r: Rec, it: ItemOverviewRow): [string, string][] => [
  ...(it.groups.ap?.map((a): [string, string] => ["AP", a.voucher_no ?? ""]) ??
    []),
  ...(it.groups.ic?.map((i): [string, string] => ["IC", i.document_no ?? ""]) ??
    []),
  ...(r.bank_voucher_no
    ? [["Bank", r.bank_voucher_no] as [string, string]]
    : []),
  ...(r.is_travel_expense === "1" && r.travel_voucher_no
    ? [["差旅", r.travel_voucher_no] as [string, string]]
    : []),
];
add("vsum", "summary", "凭证类型与张数", text, 180, (r, it) => {
  const n: Record<string, number> = {};
  for (const [t] of vouchers(r, it)) n[t] = (n[t] ?? 0) + 1;
  return (
    Object.entries(n)
      .map(([t, c]) => `${t} ${c}`)
      .join(" · ") || "无凭证"
  );
});
add(
  "vsum",
  "all_no",
  "全部凭证号码",
  text,
  240,
  (r, it) =>
    [...new Set(vouchers(r, it).map(([, v]) => v))].join(" / ") || "无凭证"
);

// Bank
add("bank", "bank_voucher_no", "凭证号");
add("bank", "bank_posted_date", "入账日期");
add("bank", "bank_received_date", "接收日期");
add("bank", "bank_prepared_by", "制表人");

// AP（多值）
for (const [k, l] of [
  ["voucher_no", "凭证号"],
  ["entry_no", "入账号码"],
  ["entry_date", "入账日期"],
  ["barcode", "Barcode"],
  ["invoice_no_display", "原始发票号"],
  ["po_no", "PO 号"],
  ["tax_rate", "税率"],
  ["invoice_date", "开票日期"],
  ["remark", "备注"],
  ["received_date", "接收日期"],
  ["preparer", "制表人"],
  ["vendor_code", "供应商编码"],
  ["vendor_name", "供应商名称"],
] as const)
  add("ap", k, l);
add("ap", "amount", "金额", money);
add("ap", "tax", "税额", money);
add("ap", "total", "价税合计", money);
add("ap", "inv_type", "票据类型", enumFmt(AP_INV_TYPE_OPTIONS));
add("ap", "tax_certified", "认证标记", enumFmt(TAX_CERTIFIED_OPTIONS));

// IC（多值）
for (const [k, l] of [
  ["document_no", "凭证号"],
  ["posted_date", "入账日期"],
  ["legal_entity", "LE"],
  ["billing", "Billing"],
  ["original_po", "原 PO"],
  ["received_date", "接收日期"],
  ["preparer", "制表人"],
] as const)
  add("ic", k, l);

// 特种发票
add("spsum", "type", "特种类型", text, 150, (r) =>
  r.biz_type ? SP_NAMES[Number(r.biz_type)] : "非特种"
);
add("spsum", "count", "特种记录数", text, 100, (r, it) =>
  r.biz_type ? String(it.groups[`sp${r.biz_type}`]?.length ?? 0) : null
);
const SP_FIELDS: Record<number, [string, string][]> = {
  1: [
    ["passenger_name", "旅客"],
    ["id_no", "证件号"],
    ["depart", "出发站"],
    ["arrival", "到达站"],
    ["train_no", "车次"],
    ["travel_date", "乘车日期"],
    ["travel_time", "开车时间"],
  ],
  2: [
    ["passenger_name", "出行人"],
    ["id_no", "证件号"],
    ["travel_date", "出行日期"],
    ["depart", "出发地"],
    ["arrival", "到达地"],
    ["seat_class", "座位等级"],
    ["transport_type", "交通工具类型"],
  ],
  3: [
    ["plate_no", "车牌号"],
    ["vehicle_type", "车辆类型"],
    ["toll_date_from", "通行日期起"],
    ["toll_date_to", "通行日期止"],
  ],
  6: [
    ["transport_type", "运输工具种类"],
    ["vehicle_no", "运输工具牌号"],
    ["depart", "起运地"],
    ["arrival", "到达地"],
    ["cargo_name", "运输货物名称"],
  ],
  7: [
    ["cert_no", "不动产权证号"],
    ["plate_no", "车牌号"],
  ],
  8: [
    ["passenger_name", "旅客"],
    ["id_no", "证件号"],
    ["depart", "出发"],
    ["arrival", "到达"],
    ["carrier", "承运人"],
    ["flight_no", "航班号"],
    ["flight_date", "航班日期"],
    ["flight_time", "起飞时间"],
  ],
  9: [
    ["cross_region_no", "跨区域涉税事项编号"],
    ["service_location", "建筑服务发生地"],
    ["project_name", "建筑项目名称"],
  ],
};
for (const n of SP_TABLES) {
  add(`sp${n}`, "line_no", "行号", text, 70);
  for (const [k, l] of SP_FIELDS[n]) add(`sp${n}`, k, l);
}

// 红冲匹配（多值）
add("rev", "dir", "方向", (v) =>
  v === "1" ? "作为红字" : v === "2" ? "作为蓝字被冲" : "—"
);
add("rev", "cp_inv", "对方发票号码", text, 190);
add("rev", "cp_line", "对方行号", text, 90);
add("rev", "matched_quantity", "匹配数量");
add("rev", "matched_amount", "匹配金额", money);
add("rev", "matched_tax", "匹配税额", money);
add("rev", "matched_total", "匹配价税合计", money);
add(
  "rev",
  "match_result_type",
  "匹配结果",
  (v) =>
    ({ "1": "成功", "2": "无蓝字发票", "3": "红字发票异常" })[v ?? ""] ?? "—"
);
add(
  "rev",
  "match_status",
  "匹配来源",
  (v) => ({ "1": "系统匹配", "2": "外部历史" })[v ?? ""] ?? "—"
);
add("rev", "match_batch_no", "匹配批次");
add("rev", "match_note", "匹配备注", text, 200);

/** 批次状态：未确认 → 待确认；已确认未锁定 → 已确认；已锁定 → 已锁定 */
const batchState = (v: string | null | undefined) => v ?? "—";
function addBatch(group: "wt" | "st", voucher: string) {
  const k = (name: string) => `${group}_${name}`;
  // 状态由 confirmed / locked_at 推出，没有对应的单一键，所以自定义取值（不导出，导出看下面各原始列）
  addTax(group, "batch_state", "批次状态", batchState, (r) =>
    r[k("confirmed")] !== "1" ? "待确认" : r[k("locked_at")] ? "已锁定" : "已确认"
  );
  addTax(group, "batch_no", "批次", text, (r) =>
    r[k("batch_no")] == null ? null : Number(r[k("batch_no")]) > 1 ? `补充批次 ${r[k("batch_no")]}` : "批次 1"
  );
  addTax(group, "filing", "纳税所属期", text, (r) =>
    r[k("filing_from")] == null
      ? null
      : r[k("filing_from")] === r[k("filing_to")]
        ? r[k("filing_from")]
        : `${r[k("filing_from")]}-${r[k("filing_to")]}`
  );
  addTax(group, "confirmed_by", "确认人", text);
  addTax(group, "confirmed_at", "确认时间", text);
  addTax(group, "locked_by", "锁定人", text);
  addTax(group, "locked_at", "锁定时间", text);
  addTax(group, "voucher_no", `${voucher}凭证号`, text);
  addTax(group, "posted_date", `${voucher}入账日期`, text);
  addTax(group, "preparer", `${voucher}做账人`, text);
  addTax(group, "open_reason", "补充批次原因", text, (r) => r[k("open_reason")]);
}

// 进项税转出
addTax("wt", "hit_count", "系统命中", (v) =>
  v === "0" ? "未命中" : `命中 ${v} 条`
);
addTax("wt", "final", "最终是否转出", yesNo("转出", "不转出"));
addTax("wt", "final_tax", "最终转出税额", money);
addTax("wt", "manual", "结论来源", yesNo("人工", "系统"));
addTax("wt", "review", "需人工核对", yesNo());
addTax("wt", "remark", "人工备注", text, (r) => r.wt_remark);
addTax("wt", "matched_at", "匹配时间", text);
addBatch("wt", "转出");

const cond = (r: Rec) =>
  (
    [
      ["seller_tax_id", "销方税号", "=", "≠"],
      ["tax_code", "编码", "=", "≠"],
      ["tax_name", "名称", "含", "不含"],
    ] as const
  )
    .filter(([k]) => r[`${k}_match_type`])
    .map(
      ([k, l, eq, neq]) =>
        `${l} ${r[`${k}_match_type`] === "eq" ? eq : neq} ${r[`${k}_values`]}`
    )
    .join(" 且 ");
add("wth", "rule_id", "规则 ID", text, 80);
add("wth", "rule_category", "规则类别");
add("wth", "cond", "匹配条件", text, 280, cond);
add("wth", "note", "规则备注", text, 200);
add("wth", "rule_enabled", "规则状态", yesNo("启用", "已停用"));
add("wth", "rule_source", "规则来源", (v) => ({ excel: "Excel 导入", manual: "手工添加" })[v ?? ""] ?? "—");
add("wth", "rule_created_at", "规则创建时间");

// 印花税
addTax("st", "hit_count", "系统命中规则数", text);
addTax("st", "final", "最终是否计入", yesNo("计入", "不计入"));
addTax("st", "category", "印花税类别", text, (r) =>
  r.st_pending === "1" ? "待确认 · 命中多条" : r.st_category
);
addTax("st", "rate_ppm", "税率", ratePpm);
addTax("st", "amount", "印花税额", money);
addTax("st", "manual", "结论来源", yesNo("人工", "系统"));
addTax("st", "remark", "人工备注", text);
addBatch("st", "印花税");
add("sth", "rule_id", "规则 ID", text, 80);
add("sth", "rule_category", "规则类别");
add("sth", "category_name", "对应印花税类别");
add("sth", "rate_ppm", "对应税率", ratePpm);
add("sth", "needs_manual_review", "需人工核对", yesNo());
add("sth", "cond", "匹配条件", text, 280, cond);
add("sth", "rule_enabled", "规则状态", yesNo("启用", "已停用"));
add("sth", "rule_source", "规则来源", (v) => ({ excel: "Excel 导入", manual: "手工添加" })[v ?? ""] ?? "—");
add("sth", "rule_created_at", "规则创建时间");

// ───────── 取值 ─────────
/** 一行在某列上的全部显示文本（供结果内搜索） */
export function rowText(c: ColDef, row: OvRow): string {
  if (!GROUPS[c.group].multi) return c.fmt(c.get(row.it.item, row.it));
  if (isExpanded(c.group, row.expand))
    return row.rec ? c.fmt(c.get(row.rec, row.it)) : "";
  return (row.it.groups[c.group] ?? [])
    .map((r) => c.fmt(c.get(r, row.it)))
    .join(" ");
}

/** 单值组的显示文本（多值组由表格按记录逐条渲染） */
/** 当前显示的列 → 后端导出列；自定义取值的列跳过。货物名称后端 item 里是分开的两个键，导出用名称 */
export function exportColumns(cols: ColDef[]) {
  return cols
    .filter((c) => c.exportKey)
    .map((c) => ({
      key: c.exportKey!,
      title: c.label,
      kind: c.isMoney ? "money" : null,
      labels: c.labels ?? null,
    }));
}

export const cellText = (c: ColDef, it: ItemOverviewRow) =>
  c.fmt(c.get(it.item, it));

/** 已保存的布局里存着旧的列 id：组合并/改名后在这里映射到新 id，布局不会因此丢列。 */
export const LEGACY_COL_IDS: Record<string, string> = {
  seller_seller_name: "header_seller_name",
  seller_seller_tax_no: "header_seller_tax_no",
};

/** 默认显示的列（通用核对） */
export const DEFAULT_VISIBLE = [
  "header_inv_unq_key",
  "item_line_no",
  "header_issue_date",
  "header_seller_name",
  "item_goods_name",
  "item_amount",
  "item_tax_rate",
  "item_tax",
  "spsum_type",
  "cert_cert_status",
  "vsum_summary",
  "ap_voucher_no",
  "ap_tax_rate",
  "ic_document_no",
  "rev_dir",
  "rev_cp_inv",
  "rev_matched_amount",
  "wt_final_tax",
  "st_category",
  "st_amount",
];
