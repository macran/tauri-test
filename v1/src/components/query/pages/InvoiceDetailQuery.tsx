// src/components/query/pages/InvoiceDetailQuery.tsx
// 发票明细查询页面

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";
import { z } from "zod";
import { commands, type InvoiceDetailRow } from "@/bindings";
import { HEAD_FILTERS, headColumns, textCol } from "@/components/query/invoice-columns";
import { DataTable, dataTableFeatures } from "@/components/query/DataTable";
import { formatMoney } from "@/components/query/format";
import { DateRangeField, SelectField, TextField } from "@/components/query/fields";
import { LayoutSelect } from "@/components/query/LayoutSelect";
import { QueryShell } from "@/components/query/QueryShell";
import { useTableLayouts } from "@/components/query/table-layout";
import { useQueryPage } from "@/components/query/useQueryPage";
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

const defaultValues: FilterValues = {
  invoiceNumber: "",
  goodsName: "",
  issueDateFrom: "",
  issueDateTo: "",
  bizType: [],
  sellerName: "",
  goodsCategory: "",
  specModel: "",
  unit: "",
  taxCode: "",
};

export default function InvoiceDetailQuery() {
  const { rows, total, truncated, loading, error, search } =
    useQueryPage<FilterValues, InvoiceDetailRow>({
      queryFn: commands.queryInvoiceDetails,
    });

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search(value),
  });

  const layouts = useTableLayouts("invoice-detail");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, InvoiceDetailRow>[]>(
    () => [
      ...headColumns<InvoiceDetailRow>((r) => r.head),
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
      title="发票明细查询"
      description="按发票号码、货物名称等条件查询发票明细行"
      searching={loading}
      error={error}
      resultCount={total}
      onSubmit={() => form.handleSubmit()}
      onReset={() => form.reset()}
      layout={<LayoutSelect layouts={layouts} />}
      basicFields={
        <>
          <form.Field name="invoiceNumber">
            {(f) => <TextField field={f} label="发票号码" />}
          </form.Field>
          <form.Field name="goodsName">
            {(f) => <TextField field={f} label="货物或应税劳务名称" />}
          </form.Field>
          <form.Field name="issueDateFrom">
            {(fromF) => (
              <form.Field name="issueDateTo">
                {(toF) => (
                  <DateRangeField label="开票日期" fromField={fromF} toField={toF} />
                )}
              </form.Field>
            )}
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
        filterColumns={HEAD_FILTERS}
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
