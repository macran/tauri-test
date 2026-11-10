// src/components/query/pages/CertifiedListQuery.tsx
// 认证清单查询（certified_list 表，is_travel_expense = 0）

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";
import { z } from "zod";
import { commands, type CertifiedListRow } from "@/bindings";
import { DataTable, dataTableFeatures } from "@/components/query/DataTable";
import { formatMoney } from "@/components/query/format";
import { DateRangeField, SelectField, TextField } from "@/components/query/fields";
import { LayoutSelect } from "@/components/query/LayoutSelect";
import { QueryShell } from "@/components/query/QueryShell";
import { useTableLayouts } from "@/components/query/table-layout";
import { useQueryPage } from "@/components/query/useQueryPage";
import { CERT_RISK_STATUS_OPTIONS, enumLabel } from "@/lib/enum-options";

const schema = z
  .object({
    invoiceNumber: z.string(),
    sellerName: z.string(),
    taxPeriod: z.string(),
    riskStatus: z.array(z.number()),
    domesticProofNo: z.string(),
    invoiceTag: z.string(),
    checkTimeFrom: z.string(),
    checkTimeTo: z.string(),
  })
  .refine(
    (v) => !v.checkTimeFrom || !v.checkTimeTo || v.checkTimeFrom <= v.checkTimeTo,
    { message: "开始日期不能晚于结束日期", path: ["checkTimeTo"] },
  );

type FilterValues = z.infer<typeof schema>;

const defaultValues: FilterValues = {
  invoiceNumber: "",
  sellerName: "",
  taxPeriod: "",
  riskStatus: [],
  domesticProofNo: "",
  invoiceTag: "",
  checkTimeFrom: "",
  checkTimeTo: "",
};

export default function CertifiedListQuery() {
  const { rows, total, truncated, loading, error, search } =
    useQueryPage<FilterValues, CertifiedListRow>({
      queryFn: commands.queryCertifiedList,
    });

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search(value),
  });

  const layouts = useTableLayouts("certified-list");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, CertifiedListRow>[]>(
    () => [
      { accessorKey: "invoiceNumber", header: "发票号码" },
      { accessorKey: "issueDate", header: "开票日期" },
      { accessorKey: "sellerName", header: "销售方名称" },
      { accessorKey: "taxPeriod", header: "税款所属期" },
      { accessorKey: "checkTime", header: "勾选时间" },
      {
        accessorKey: "validDeductAmount",
        header: "有效抵扣税额",
        cell: ({ row }) => formatMoney(row.original.validDeductAmount),
      },
      { accessorKey: "invoiceTag", header: "票种标签" },
      {
        accessorKey: "riskStatus",
        header: "风险状态",
        cell: ({ row }) => enumLabel(CERT_RISK_STATUS_OPTIONS, row.original.riskStatus),
      },
      { accessorKey: "domesticProofNo", header: "转内销证明编号" },
      {
        accessorKey: "amount",
        header: "发票金额",
        cell: ({ row }) => formatMoney(row.original.amount),
      },
      {
        accessorKey: "tax",
        header: "发票税额",
        cell: ({ row }) => formatMoney(row.original.tax),
      },
      {
        accessorKey: "total",
        header: "价税合计",
        cell: ({ row }) => formatMoney(row.original.total),
      },
      { accessorKey: "voucherNo", header: "凭证号（差旅费回填）" },
      { accessorKey: "postedDate", header: "过账日期（差旅费回填）" },
      { accessorKey: "id", header: "ID" },
      { accessorKey: "headerId", header: "发票头ID" },
    ],
    [],
  );

  return (
    <QueryShell
      title="认证清单查询"
      description="按发票号码、销售方、税款所属期等条件查询认证清单"
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
          <form.Field name="sellerName">
            {(f) => <TextField field={f} label="销售方名称" />}
          </form.Field>
          <form.Field name="taxPeriod">
            {(f) => <TextField field={f} label="税款所属期" placeholder="如 202604" />}
          </form.Field>
          <form.Field name="checkTimeFrom">
            {(fromF) => (
              <form.Field name="checkTimeTo">
                {(toF) => (
                  <DateRangeField label="勾选时间" fromField={fromF} toField={toF} />
                )}
              </form.Field>
            )}
          </form.Field>
        </>
      }
      advancedFields={
        <>
          <form.Field name="riskStatus">
            {(f) => (
              <SelectField field={f} label="风险状态" options={CERT_RISK_STATUS_OPTIONS} />
            )}
          </form.Field>
          <form.Field name="domesticProofNo">
            {(f) => <TextField field={f} label="转内销证明编号" />}
          </form.Field>
          <form.Field name="invoiceTag">
            {(f) => <TextField field={f} label="票种标签" />}
          </form.Field>
        </>
      }
    >
      <DataTable
        layouts={layouts}
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
