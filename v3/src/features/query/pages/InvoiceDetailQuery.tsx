// src/components/query/pages/InvoiceDetailQuery.tsx
// 发票明细查询页面

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo, useState } from "react";
import { z } from "zod";
import { commands, type InvoiceDetailFilter, type InvoiceDetailRow } from "@/bindings";
import { HEAD_FILTERS, headColumns, textCol } from "@/features/query/invoice-columns";
import { DataTable, dataTableFeatures } from "@/features/query/DataTable";
import { formatMoney } from "@/features/query/format";
import { DateRangeField, SelectField, TextField, defaultDateRange } from "@/features/query/fields";
import { BatchQueryField, toBatchSet, type BatchDraft } from "@/features/query/BatchQuery";
import { LayoutSelect } from "@/features/query/LayoutSelect";
import { QueryShell } from "@/features/query/QueryShell";
import { useTableLayouts } from "@/features/query/table-layout";
import { useQueryPage } from "@/features/query/useQueryPage";
import { enumLabel, SPECIAL_BIZ_TYPE_OPTIONS } from "@/lib/enum-options";

const schema = z
  .object({
    invoiceNumber: z.string(),
    goodsName: z.string(),
    issueDateFrom: z.string(),
    issueDateTo: z.string(),
    bizType: z.array(z.number()),
    sellerName: z.string(),
    goodsCategory: z.string(),
    specModel: z.string(),
    unit: z.string(),
    taxCode: z.string(),
  })
  .refine(
    (v) => !v.issueDateFrom || !v.issueDateTo || v.issueDateFrom <= v.issueDateTo,
    { message: "开始日期不能晚于结束日期", path: ["issueDateTo"] },
  );

type FilterValues = z.infer<typeof schema>;

const dateRange = defaultDateRange();

const defaultValues: FilterValues = {
  invoiceNumber: "",
  goodsName: "",
  issueDateFrom: dateRange.from,
  issueDateTo: dateRange.to,
  bizType: [],
  sellerName: "",
  goodsCategory: "",
  specModel: "",
  unit: "",
  taxCode: "",
};

const FILTERS = { ...HEAD_FILTERS, lineFlag: [{ value: "折扣行", label: "折扣行" }] };

export default function InvoiceDetailQuery() {
  const { rows, total, truncated, loading, error, search } =
    useQueryPage<InvoiceDetailFilter, InvoiceDetailRow>({
      queryFn: commands.queryInvoiceDetails,
    });

  const [batch, setBatch] = useState<BatchDraft>({});
  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search({ ...value, batch: toBatchSet(batch) }),
  });

  const layouts = useTableLayouts("invoice-detail");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, InvoiceDetailRow>[]>(
    () => [
      ...headColumns<InvoiceDetailRow>((r) => r.head),
      // 折扣行由后端按整张发票判定（与红冲查询同一规则）；普通行留空，列筛选里可只看折扣行
      {
        id: "lineFlag",
        header: "行标志",
        size: 80,
        accessorFn: (r) => (r.isDiscount ? "折扣行" : ""),
      },
      textCol<InvoiceDetailRow>("taxCode", "税收分类编码", 160),
      {
        accessorKey: "bizType",
        header: "特定业务类型",
        cell: ({ row }) => enumLabel(SPECIAL_BIZ_TYPE_OPTIONS, row.original.bizType),
      },
      {
        id: "goodsName",
        header: "货物或应税劳务名称",
        size: 260,
        // 入库时拆成 goods_category + goods_name，展示时还原为 *类别*名称
        accessorFn: (r) =>
          r.goodsCategory ? `*${r.goodsCategory}*${r.goodsName}` : r.goodsName,
      },
      textCol<InvoiceDetailRow>("specModel", "规格型号"),
      textCol<InvoiceDetailRow>("unit", "单位", 80),
      textCol<InvoiceDetailRow>("quantity", "数量", 100),
      textCol<InvoiceDetailRow>("unitPrice", "单价", 110),
      {
        accessorKey: "amount",
        header: "金额",
        size: 120,
        cell: ({ row }) => formatMoney(row.original.amount),
      },
      textCol<InvoiceDetailRow>("taxRate", "税率", 80),
      {
        accessorKey: "tax",
        header: "税额",
        size: 120,
        cell: ({ row }) => formatMoney(row.original.tax),
      },
      {
        accessorKey: "total",
        header: "价税合计",
        size: 120,
        cell: ({ row }) => formatMoney(row.original.total),
      },
    ],
    [],
  );

  return (
    <QueryShell
      searching={loading}
      error={error}
      onSubmit={() => form.handleSubmit()}
      onReset={() => {
        setBatch({});
        form.reset();
      }}
      layout={<LayoutSelect layouts={layouts} />}
      queryTools={
        <BatchQueryField
          keys={["invoiceNumber", "sellerName", "sellerTaxNo", "goodsName", "taxCode"]}
          draft={batch}
          onChange={setBatch}
        />
      }
      basicFields={
        <>
          <form.Field name="issueDateFrom">
            {(fromF) => (
              <form.Field name="issueDateTo">
                {(toF) => (
                  <DateRangeField label="开票日期" fromField={fromF} toField={toF} />
                )}
              </form.Field>
            )}
          </form.Field>
          <form.Field name="invoiceNumber">
            {(f) => <TextField field={f} label="发票号码" />}
          </form.Field>
          <form.Field name="goodsName">
            {(f) => <TextField field={f} label="货物或应税劳务名称" />}
          </form.Field>
          <form.Field name="bizType">
            {(f) => (
              <SelectField field={f} label="特定业务类型" options={SPECIAL_BIZ_TYPE_OPTIONS} />
            )}
          </form.Field>
        </>
      }
      advancedFields={
        <>
          <form.Field name="sellerName">
            {(f) => <TextField field={f} label="销售方名称" />}
          </form.Field>
          <form.Field name="goodsCategory">
            {(f) => <TextField field={f} label="货物劳务类别" />}
          </form.Field>
          <form.Field name="specModel">
            {(f) => <TextField field={f} label="规格型号" />}
          </form.Field>
          <form.Field name="unit">
            {(f) => <TextField field={f} label="单位" />}
          </form.Field>
          <form.Field name="taxCode">
            {(f) => <TextField field={f} label="税收分类编码" />}
          </form.Field>
        </>
      }
    >
      <DataTable
        layouts={layouts}
        filterColumns={FILTERS}
        columns={columns}
        data={rows}
        loading={loading}
        total={total}
        truncated={truncated}
        rowKey={(r) => r.id}
      />
    </QueryShell>
  );
}
