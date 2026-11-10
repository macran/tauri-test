// src/components/query/pages/ItemOverviewQuery.tsx
// 综合查询：一行 = 一个发票明细 item，把发票头、销方、认证、AP/IC/Bank 凭证、
// 特种发票、红冲匹配、进项税转出、印花税结论放在同一张表里核对。

import { MagnifyingGlass, X } from "@phosphor-icons/react";
import { useForm } from "@tanstack/react-form";
import { useEffect, useMemo, useState } from "react";
import { z } from "zod";
import {
  commands,
  type ItemOverviewFilter,
  type ItemOverviewRow,
} from "@/bindings";
import { LayoutMenu } from "@/components/query/DataTableMenus";
import {
  DateField,
  DateRangeField,
  SelectField,
  TextField,
} from "@/components/query/fields";
import { formatMoney } from "@/components/query/format";
import { LayoutSelect } from "@/components/query/LayoutSelect";
import { ColumnPanel, sortCols } from "@/components/query/overview/ColumnPanel";
import {
  COLS,
  DEFAULT_VISIBLE,
  EXPAND_OPTIONS,
  expandRecs,
  GROUPS,
  type OvRow,
  rowText,
} from "@/components/query/overview/columns";
import { ItemDrawer } from "@/components/query/overview/ItemDrawer";
import { OverviewTable } from "@/components/query/overview/OverviewTable";
import { QueryShell } from "@/components/query/QueryShell";
import { useTableLayouts } from "@/components/query/table-layout";
import { useQueryPage } from "@/components/query/useQueryPage";
import { Button } from "@/components/ui/button";
import { Checkbox } from "@/components/ui/checkbox";
import { Input } from "@/components/ui/input";
import { Label } from "@/components/ui/label";
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from "@/components/ui/select";
import { type EnumOption, SPECIAL_BIZ_TYPE_OPTIONS } from "@/lib/enum-options";

const CERTIFIED_OPTIONS: EnumOption[] = [
  { value: 1, label: "已勾选" },
  { value: 0, label: "未勾选" },
];
const REVERSAL_OPTIONS: EnumOption[] = [
  { value: 1, label: "作为红字明细" },
  { value: 2, label: "作为蓝字被冲" },
  { value: 3, label: "无红冲匹配" },
];
const TRANSFER_OPTIONS: EnumOption[] = [
  { value: 1, label: "转出" },
  { value: 0, label: "不转出" },
];
const STAMP_OPTIONS: EnumOption[] = [
  { value: 1, label: "计入" },
  { value: 2, label: "计入 · 待确认类别" },
  { value: 0, label: "不计入" },
];

const schema = z
  .object({
    issueDateFrom: z.string(),
    issueDateTo: z.string(),
    invoiceNumber: z.string(),
    seller: z.string(),
    goodsName: z.string(),
    taxCode: z.string(),
    bizType: z.array(z.number()),
    voucherNo: z.string(),
    certified: z.array(z.number()),
    apVoucherNo: z.string(),
    apVendorCode: z.string(),
    reversal: z.array(z.number()),
    periodFrom: z.string(),
    periodTo: z.string(),
    transfer: z.array(z.number()),
    stamp: z.array(z.number()),
  })
  .refine(
    (v) =>
      !v.issueDateFrom || !v.issueDateTo || v.issueDateFrom <= v.issueDateTo,
    { message: "开始日期不能晚于结束日期", path: ["issueDateTo"] }
  );

type FilterValues = z.infer<typeof schema>;

const defaultValues: FilterValues = {
  issueDateFrom: "",
  issueDateTo: "",
  invoiceNumber: "",
  seller: "",
  goodsName: "",
  taxCode: "",
  bizType: [],
  voucherNo: "",
  certified: [],
  apVoucherNo: "",
  apVendorCode: "",
  reversal: [],
  periodFrom: "",
  periodTo: "",
  transfer: [],
  stamp: [],
};

/** 月份选择器给的是 2026-09-01，后端要 202609 */
const toPeriod = (v: string) => v.slice(0, 7).replace("-", "");

const sum = (
  rows: ItemOverviewRow[],
  pick: (r: ItemOverviewRow["item"]) => string | null | undefined
) => rows.reduce((a, r) => a + Number(pick(r.item) ?? 0), 0);

export default function ItemOverviewQuery() {
  const { rows, total, truncated, loading, error, search } = useQueryPage<
    ItemOverviewFilter,
    ItemOverviewRow
  >({ queryFn: commands.queryItemOverview });

  const [formError, setFormError] = useState<string | null>(null);
  const [mode, setMode] = useState<"summary" | "expand">("summary");
  const [expand, setExpand] = useState("ap");
  const [keepEmpty, setKeepEmpty] = useState(false);

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => {
      if (
        !value.periodFrom &&
        !value.periodTo &&
        (value.transfer.length || value.stamp.length)
      ) {
        return setFormError(
          "按进项税转出 / 印花税筛选时，请先选择税务计算期间"
        );
      }
      setFormError(null);
      return search({
        ...value,
        periodFrom: value.periodFrom && toPeriod(value.periodFrom),
        periodTo: value.periodTo && toPeriod(value.periodTo),
      });
    },
  });

  const layouts = useTableLayouts("item-overview");
  const [stack, setStack] = useState(false);
  const [hideSp, setHideSp] = useState(true);
  const [panelOpen, setPanelOpen] = useState(false);
  const [openItem, setOpenItem] = useState<ItemOverviewRow | null>(null);

  // 选中的列：系统默认布局由代码给出；用户保存的布局只记列 id 的集合
  const [sizes, setSizes] = useState<Record<string, number>>({});
  const [keyword, setKeyword] = useState("");
  const [debounced, setDebounced] = useState("");
  useEffect(() => {
    const t = setTimeout(() => setDebounced(keyword), 200);
    return () => clearTimeout(t);
  }, [keyword]);
  const [colIds, setColIds] = useState(DEFAULT_VISIBLE);
  const selectedLayout = layouts.selectedItem?.layout;
  useEffect(() => {
    setColIds(sortCols(selectedLayout?.columnOrder ?? DEFAULT_VISIBLE));
    setSizes(selectedLayout?.columnSizing ?? {});
  }, [selectedLayout]);

  // 本次结果里没有的特种类型，其列组可以隐藏
  const present = useMemo(
    () => new Set(rows.map((r) => r.item.biz_type)),
    [rows]
  );
  const hiddenSp = new Set<string>();
  const cols = useMemo(
    () =>
      colIds
        .map((id) => COLS.find((c) => c.id === id))
        .filter((c) => !!c)
        .filter((c) => {
          const hide =
            hideSp && GROUPS[c.group].sp && !present.has(c.group.slice(2));
          if (hide) hiddenSp.add(GROUPS[c.group].label);
          return !hide;
        }),
    [colIds, hideSp, present, hiddenSp]
  );

  const tableRows = useMemo<OvRow[]>(() => {
    if (mode === "summary") {
      return rows.map((it) => ({ key: it.item.id ?? "", it, expand: null }));
    }
    return rows.flatMap<OvRow>((it) => {
      const recs = expandRecs(it, expand);
      if (!recs?.length)
        return keepEmpty
          ? [{ key: `${it.item.id}`, it, expand, none: true }]
          : [];
      return recs.map((rec, i) => ({
        key: `${it.item.id}-${i}`,
        it,
        expand,
        rec,
        idx: i,
      }));
    });
  }, [rows, mode, expand, keepEmpty]);

  // 结果内搜索：只搜当前显示的列；每行拼成一个小写串，输入时只做 includes
  const shownRows = useMemo(() => {
    const q = debounced.trim().toLowerCase();
    if (!q) return tableRows;
    return tableRows.filter((r) =>
      cols.some((c) => rowText(c, r).toLowerCase().includes(q))
    );
  }, [tableRows, cols, debounced]);
  const shownItems = useMemo(
    () => [...new Set(shownRows.map((r) => r.it))],
    [shownRows]
  );

  const hasTax =
    rows[0]?.item.st_computed === "1" || rows[0]?.item.wt_computed === "1";

  return (
    <QueryShell
      title="综合查询"
      description="一行一个发票明细，按需展示发票头、凭证、特种发票、红冲匹配和税务结论；用「选择列」挑选字段，用「保存布局」记住"
      searching={loading}
      error={formError ?? error}
      resultCount={total}
      onSubmit={() => form.handleSubmit()}
      onReset={() => {
        setFormError(null);
        form.reset();
      }}
      layout={<LayoutSelect layouts={layouts} />}
      basicFields={
        <>
          <form.Field name="issueDateFrom">
            {(fromF) => (
              <form.Field name="issueDateTo">
                {(toF) => (
                  <DateRangeField
                    label="开票日期"
                    fromField={fromF}
                    toField={toF}
                  />
                )}
              </form.Field>
            )}
          </form.Field>
          <form.Field name="invoiceNumber">
            {(f) => (
              <TextField
                field={f}
                label="发票号码"
                placeholder="代码-号码 或 数电号码"
              />
            )}
          </form.Field>
          <form.Field name="seller">
            {(f) => <TextField field={f} label="销方名称或税号" />}
          </form.Field>
          <form.Field name="goodsName">
            {(f) => <TextField field={f} label="货物或服务名称" />}
          </form.Field>
        </>
      }
      advancedFields={
        <>
          <form.Field name="taxCode">
            {(f) => <TextField field={f} label="税收分类编码" />}
          </form.Field>
          <form.Field name="bizType">
            {(f) => (
              <SelectField
                field={f}
                label="特种业务类型"
                options={SPECIAL_BIZ_TYPE_OPTIONS}
              />
            )}
          </form.Field>
          <form.Field name="voucherNo">
            {(f) => (
              <TextField field={f} label="凭证号码（AP / IC / Bank / 差旅）" />
            )}
          </form.Field>
          <form.Field name="certified">
            {(f) => (
              <SelectField
                field={f}
                label="认证状态"
                options={CERTIFIED_OPTIONS}
              />
            )}
          </form.Field>
          <form.Field name="apVoucherNo">
            {(f) => (
              <TextField field={f} label="AP 凭证号（与供应商同一条记录）" />
            )}
          </form.Field>
          <form.Field name="apVendorCode">
            {(f) => <TextField field={f} label="AP 供应商编码" />}
          </form.Field>
          <form.Field name="reversal">
            {(f) => (
              <SelectField
                field={f}
                label="红冲匹配"
                options={REVERSAL_OPTIONS}
              />
            )}
          </form.Field>
          <form.Field name="periodFrom">
            {(f) => (
              <DateField
                field={f}
                label="税务计算期间（起）"
                precision="month"
              />
            )}
          </form.Field>
          <form.Field name="periodTo">
            {(f) => (
              <DateField
                field={f}
                label="税务计算期间（止）"
                precision="month"
              />
            )}
          </form.Field>
          <form.Field name="transfer">
            {(f) => (
              <SelectField
                field={f}
                label="进项税转出"
                options={TRANSFER_OPTIONS}
              />
            )}
          </form.Field>
          <form.Field name="stamp">
            {(f) => (
              <SelectField field={f} label="印花税" options={STAMP_OPTIONS} />
            )}
          </form.Field>
        </>
      }
    >
      <div className="flex flex-col gap-2">
        <div className="flex flex-wrap items-center gap-x-5 gap-y-2">
          <div className="flex items-center gap-2">
            <Label className="text-muted-foreground">行粒度</Label>
            <Toggle
              value={mode}
              onChange={setMode}
              options={[
                ["summary", "每个 item 一行"],
                ["expand", "按记录展开"],
              ]}
            />
            {mode === "expand" && (
              <>
                <Select
                  items={EXPAND_OPTIONS}
                  value={expand}
                  onValueChange={(v) => v && setExpand(v)}
                >
                  <SelectTrigger className="w-40">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {EXPAND_OPTIONS.map((o) => (
                      <SelectItem key={o.value} value={o.value}>
                        {o.label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                <Label className="gap-2 text-muted-foreground">
                  <Checkbox
                    checked={keepEmpty}
                    onCheckedChange={(c) => setKeepEmpty(c === true)}
                  />
                  保留无此类记录的 item
                </Label>
              </>
            )}
          </div>
          <div className="flex items-center gap-2">
            <Label className="text-muted-foreground">多值显示</Label>
            <Toggle
              value={stack ? "stack" : "slash"}
              onChange={(v) => setStack(v === "stack")}
              options={[
                ["slash", "/ 合并"],
                ["stack", "逐条堆叠"],
              ]}
            />
          </div>
          <Label className="gap-2 text-muted-foreground">
            <Checkbox
              checked={hideSp}
              onCheckedChange={(c) => setHideSp(c === true)}
            />
            隐藏本次结果中不存在的特种类型列组
          </Label>
          <div className="ml-auto flex items-center gap-2">
            <div className="relative w-56">
              <MagnifyingGlass className="absolute top-1/2 left-2 size-3.5 -translate-y-1/2 text-muted-foreground" />
              <Input
                value={keyword}
                onChange={(e) => setKeyword(e.target.value)}
                placeholder="在结果中搜索…"
                className="pr-7 pl-7"
              />
              {keyword && (
                <button
                  type="button"
                  aria-label="清除"
                  onClick={() => setKeyword("")}
                  className="absolute top-1/2 right-2 -translate-y-1/2 text-muted-foreground hover:text-foreground"
                >
                  <X className="size-3.5" />
                </button>
              )}
            </div>
            <Button
              type="button"
              variant="outline"
              size="sm"
              onClick={() => setPanelOpen(true)}
            >
              选择列
            </Button>
            <LayoutMenu
              mine={layouts.items.filter((i) => i.isMine)}
              selected={layouts.selectedItem}
              onSave={(name, isPrivate) =>
                void layouts.save(
                  name,
                  {
                    columnOrder: colIds,
                    columnVisibility: {},
                    columnSizing: sizes,
                  },
                  isPrivate
                )
              }
              onSetDefault={(id) => void layouts.setDefault(id)}
              onDelete={(id) => void layouts.remove(id)}
            />
          </div>
        </div>

        <p className="text-xs text-muted-foreground">
          {mode === "summary" ? (
            <>
              当前一行 = <b className="text-foreground">一个发票明细 item</b>
              。多值列按记录组对齐，悬停任一值可高亮同一条记录的其他字段。
            </>
          ) : (
            <>
              当前一行 ={" "}
              <b className="text-foreground">
                一个 item × 一条
                {EXPAND_OPTIONS.find((o) => o.value === expand)?.label}
              </b>
              。只有带“逐行”标记的列逐条显示，其他记录组仍按 item
              合并，避免笛卡尔积。
            </>
          )}
          {hiddenSp.size > 0 &&
            ` 已隐藏本次结果中不存在的特种列组：${[...hiddenSp].join("、")}。`}
          {truncated &&
            ` 结果共 ${total} 条，仅显示前 ${rows.length} 条，请缩小查询范围。`}
        </p>

        <OverviewTable
          rows={shownRows}
          sizes={sizes}
          onResize={(id, w) => setSizes((p) => ({ ...p, [id]: w }))}
          cols={cols}
          stack={stack}
          loading={loading}
          onOpenItem={setOpenItem}
        />

        <p className="flex flex-wrap gap-x-5 text-xs text-muted-foreground">
          <span>
            item {shownItems.length}
            {shownItems.length !== rows.length && ` / ${rows.length}`}
          </span>
          <span>表格行 {shownRows.length}</span>
          <span>
            明细金额 {formatMoney(String(sum(shownItems, (i) => i.amount)))}
          </span>
          <span>
            明细税额 {formatMoney(String(sum(shownItems, (i) => i.tax)))}
          </span>
          {hasTax && (
            <>
              <span>
                最终转出税额{" "}
                {formatMoney(String(sum(shownItems, (i) => i.wt_final_tax)))}
              </span>
              <span>
                印花税额{" "}
                {formatMoney(String(sum(shownItems, (i) => i.st_amount)))}
              </span>
            </>
          )}
          <span className="basis-full">
            合计均按 item 去重计算，展开行不会重复累加；凭证金额属于发票级，不在
            item 合计中求和。
          </span>
        </p>
      </div>

      <ColumnPanel
        open={panelOpen}
        onOpenChange={setPanelOpen}
        selected={colIds}
        onChange={setColIds}
      />
      <ItemDrawer it={openItem} onClose={() => setOpenItem(null)} />
    </QueryShell>
  );
}

function Toggle<V extends string>({
  value,
  onChange,
  options,
}: {
  value: V;
  onChange: (v: V) => void;
  options: [V, string][];
}) {
  return (
    <div className="flex gap-1">
      {options.map(([v, label]) => (
        <Button
          key={v}
          type="button"
          size="sm"
          variant={value === v ? "default" : "outline"}
          onClick={() => onChange(v)}
        >
          {label}
        </Button>
      ))}
    </div>
  );
}
