// src/components/query/pages/TravelExpenseQuery.tsx
// 差旅费用凭证查询（certified_list 表，is_travel_expense = 1）

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";
import { z } from "zod";
import { commands, type CertifiedListRow } from "@/bindings";
import { DataTable, dataTableFeatures } from "@/features/query/DataTable";
import { formatMoney } from "@/features/query/format";
import { DateRangeField, SelectField, TextField, defaultDateRange } from "@/features/query/fields";
import { LayoutSelect } from "@/features/query/LayoutSelect";
import { QueryShell } from "@/features/query/QueryShell";
import { useTableLayouts } from "@/features/query/table-layout";
import { useQueryPage } from "@/features/query/useQueryPage";
import { CERT_RISK_STATUS_OPTIONS, enumLabel } from "@/lib/enum-options";

const schema = z
  .object({
    invoiceNumber: z.string(),
    voucherNo: z.string(),
    postedDateFrom: z.string(),
    postedDateTo: z.string(),
    sellerName: z.string(),
    taxPeriod: z.string(),
    riskStatus: z.array(z.number()),
  })
  .refine(
    (v) => !v.postedDateFrom || !v.postedDateTo || v.postedDateFrom <= v.postedDateTo,
    { message: "开始日期不能晚于结束日期", path: ["postedDateTo"] },
  );

type FilterValues = z.infer<typeof schema>;

const dateRange = defaultDateRange();

const defaultValues: FilterValues = {
  invoiceNumber: "",
  voucherNo: "",
  postedDateFrom: dateRange.from,
  postedDateTo: dateRange.to,
  sellerName: "",
  taxPeriod: "",
  riskStatus: [],
};

export default function TravelExpenseQuery() {
  const { rows, total, truncated, loading, error, search } =
    useQueryPage<FilterValues, CertifiedListRow>({
      queryFn: commands.queryTravelExpense,
    });

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search(value),
  });

  const layouts = useTableLayouts("travel-expense");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, CertifiedListRow>[]>(
    () => [
      { accessorKey: "invoiceNumber", header: "发票号码" },
      { accessorKey: "voucherNo", header: "凭证号" },
      { accessorKey: "postedDate", header: "过账日期" },
      { accessorKey: "issueDate", header: "开票日期" },
      { accessorKey: "sellerName", header: "销售方名称" },
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
      { accessorKey: "taxPeriod", header: "税款所属期" },
      { accessorKey: "checkTime", header: "勾选时间" },
      {
        accessorKey: "riskStatus",
        header: "风险状态",
        cell: ({ row }) => enumLabel(CERT_RISK_STATUS_OPTIONS, row.original.riskStatus),
      },
      { accessorKey: "invoiceTag", header: "票种标签" },
      { accessorKey: "domesticProofNo", header: "转内销证明编号" },
      {
        accessorKey: "validDeductAmount",
        header: "有效抵扣税额",
        cell: ({ row }) => formatMoney(row.original.validDeductAmount),
      },
      { accessorKey: "id", header: "ID" },
      { accessorKey: "headerId", header: "发票头ID" },
    ],
    [],
  );

  return (
    <QueryShell
      searching={loading}
      error={error}
      onSubmit={() => form.handleSubmit()}
      layout={<LayoutSelect layouts={layouts} />}
      onReset={() => form.reset()}
      basicFields={
        <>
          <form.Field name="postedDateFrom">
            {(fromF) => (
              <form.Field name="postedDateTo">
                {(toF) => (
                  <DateRangeField label="过账日期" fromField={fromF} toField={toF} />
                )}
              </form.Field>
            )}
          </form.Field>
          <form.Field name="invoiceNumber">
            {(f) => <TextField field={f} label="发票号码" />}
          </form.Field>
          <form.Field name="voucherNo">
            {(f) => <TextField field={f} label="凭证号" />}
          </form.Field>
          <form.Field name="sellerName">
            {(f) => <TextField field={f} label="销售方名称" />}
          </form.Field>
        </>
      }
      advancedFields={
        <>
          <form.Field name="taxPeriod">
            {(f) => <TextField field={f} label="税款所属期" />}
          </form.Field>
          <form.Field name="riskStatus">
            {(f) => (
              <SelectField field={f} label="风险状态" options={CERT_RISK_STATUS_OPTIONS} />
            )}
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
