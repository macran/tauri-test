// src/components/query/pages/InvoiceHeaderQuery.tsx
// 发票头查询页面

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo, useState } from "react";
import { z } from "zod";
import { commands, type InvoiceHeaderFilter, type InvoiceHeaderRow } from "@/bindings";
import { HEAD_FILTERS, headColumns } from "@/features/query/invoice-columns";
import { DataTable, dataTableFeatures } from "@/features/query/DataTable";
import { formatMoney } from "@/features/query/format";
import { DateRangeField, SelectField, TextField, defaultDateRange } from "@/features/query/fields";
import { BatchQueryField, toBatchSet, type BatchDraft } from "@/features/query/BatchQuery";
import { LayoutSelect } from "@/features/query/LayoutSelect";
import { QueryShell } from "@/features/query/QueryShell";
import { useTableLayouts } from "@/features/query/table-layout";
import { useQueryPage } from "@/features/query/useQueryPage";
import {
  INV_SOURCE_OPTIONS,
  INV_STATE_OPTIONS,
  INV_TYPE_OPTIONS,
  IS_POSITIVE_OPTIONS,
  RISK_LEVEL_OPTIONS,
} from "@/lib/enum-options";

const schema = z
  .object({
    invoiceNumber: z.string(),
    sellerName: z.string(),
    issueDateFrom: z.string(),
    issueDateTo: z.string(),
    invType: z.array(z.number()),
    state: z.array(z.number()),
    sellerTaxNo: z.string(),
    isPos: z.array(z.number()),
    risk: z.array(z.number()),
    source: z.array(z.number()),
    remark: z.string(),
    revOrgKey: z.string(),
    redNoticeNo: z.string(),
  })
  .refine(
    (v) => !v.issueDateFrom || !v.issueDateTo || v.issueDateFrom <= v.issueDateTo,
    { message: "开始日期不能晚于结束日期", path: ["issueDateTo"] },
  );

type FilterValues = z.infer<typeof schema>;

const dateRange = defaultDateRange();

const defaultValues: FilterValues = {
  invoiceNumber: "",
  sellerName: "",
  issueDateFrom: dateRange.from,
  issueDateTo: dateRange.to,
  invType: [],
  state: [],
  sellerTaxNo: "",
  isPos: [],
  risk: [],
  source: [],
  remark: "",
  revOrgKey: "",
  redNoticeNo: "",
};

export default function InvoiceHeaderQuery() {
  const { rows, total, truncated, loading, error, search } =
    useQueryPage<InvoiceHeaderFilter, InvoiceHeaderRow>({
      queryFn: commands.queryInvoiceHeaders,
    });

  const [batch, setBatch] = useState<BatchDraft>({});
  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search({ ...value, batch: toBatchSet(batch) }),
  });

  const layouts = useTableLayouts("invoice-header");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, InvoiceHeaderRow>[]>(
    () => [
      ...headColumns<InvoiceHeaderRow>((r) => r),
      {
        accessorKey: "amount",
        header: "金额",
        size: 120,
        cell: ({ row }) => formatMoney(row.original.amount),
      },
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
      layout={<LayoutSelect layouts={layouts} />}
      onReset={() => {
        setBatch({});
        form.reset();
      }}
      queryTools={
        <BatchQueryField
          keys={["invoiceNumber", "sellerName", "sellerTaxNo"]}
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
            {(f) => <TextField field={f} label="发票号码" placeholder="发票代码+号码或数电号码" />}
          </form.Field>
          <form.Field name="sellerName">
            {(f) => <TextField field={f} label="销售方名称" />}
          </form.Field>
          <form.Field name="invType">
            {(f) => <SelectField field={f} label="发票票种" options={INV_TYPE_OPTIONS} />}
          </form.Field>
        </>
      }
      advancedFields={
        <>
          <form.Field name="state">
            {(f) => <SelectField field={f} label="发票状态" options={INV_STATE_OPTIONS} />}
          </form.Field>
          <form.Field name="sellerTaxNo">
            {(f) => <TextField field={f} label="销售方税号" />}
          </form.Field>
          <form.Field name="isPos">
            {(f) => <SelectField field={f} label="正负数" options={IS_POSITIVE_OPTIONS} />}
          </form.Field>
          <form.Field name="risk">
            {(f) => <SelectField field={f} label="风险等级" options={RISK_LEVEL_OPTIONS} />}
          </form.Field>
          <form.Field name="source">
            {(f) => <SelectField field={f} label="发票来源" options={INV_SOURCE_OPTIONS} />}
          </form.Field>
          <form.Field name="remark">
            {(f) => <TextField field={f} label="备注" />}
          </form.Field>
          <form.Field name="revOrgKey">
            {(f) => <TextField field={f} label="红冲原始关联号" />}
          </form.Field>
          <form.Field name="redNoticeNo">
            {(f) => <TextField field={f} label="红字信息表编号" />}
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
