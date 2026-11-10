// src/components/query/pages/BankVoucherQuery.tsx
// 银行回单进项发票凭证查询

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";
import { z } from "zod";
import { commands, type BankVoucherRow, type BankVoucherEdit } from "@/bindings";
import { DataTable, dataTableFeatures } from "@/features/query/DataTable";
import { formatMoney } from "@/features/query/format";
import { DateRangeField, TextField, defaultDateRange } from "@/features/query/fields";
import { useVoucherActions } from "@/features/query/useVoucherActions";
import { LayoutSelect } from "@/features/query/LayoutSelect";
import { QueryShell } from "@/features/query/QueryShell";
import { useTableLayouts } from "@/features/query/table-layout";
import { useQueryPage } from "@/features/query/useQueryPage";

const schema = z
  .object({
    invoiceNumber: z.string(),
    voucherNo: z.string(),
    postedDateFrom: z.string(),
    postedDateTo: z.string(),
    sellerName: z.string(),
    receivedDateFrom: z.string(),
    receivedDateTo: z.string(),
    preparedBy: z.string(),
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
  receivedDateFrom: "",
  receivedDateTo: "",
  preparedBy: "",
};

export default function BankVoucherQuery() {
  const { rows, total, truncated, loading, error, search, refresh } =
    useQueryPage<FilterValues, BankVoucherRow>({
      queryFn: commands.queryBankVouchers,
    });

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search(value),
  });

  const actions = useVoucherActions<BankVoucherRow, BankVoucherEdit>({
    title: "编辑银行费用凭证",
    fields: [
      { key: "postedDate", label: "入账日期", type: "date", required: true },
      { key: "voucherNo", label: "凭证号", type: "text", required: true },
    ],
    toInitial: (r) => ({ postedDate: r.postedDate, voucherNo: r.voucherNo }),
    toEdit: (r, v) => ({
      id: r.id,
      postedDate: String(v.postedDate),
      voucherNo: String(v.voucherNo),
    }),
    update: commands.updateBankVoucher,
    remove: commands.deleteBankVouchers,
    refresh,
  });

  const layouts = useTableLayouts("bank-voucher");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, BankVoucherRow>[]>(
    () => [
      { accessorKey: "invoiceNumber", header: "发票号码" },
      { accessorKey: "voucherNo", header: "凭证号" },
      { accessorKey: "postedDate", header: "入账日期" },
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
      { accessorKey: "receivedDate", header: "收到日期" },
      { accessorKey: "preparedBy", header: "制表人" },
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
      onReset={() => form.reset()}
      layout={<LayoutSelect layouts={layouts} />}
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
          <form.Field name="receivedDateFrom">
            {(fromF) => (
              <form.Field name="receivedDateTo">
                {(toF) => (
                  <DateRangeField label="收到日期" fromField={fromF} toField={toF} />
                )}
              </form.Field>
            )}
          </form.Field>
          <form.Field name="preparedBy">
            {(f) => <TextField field={f} label="制表人" />}
          </form.Field>
        </>
      }
    >
      <DataTable
        layouts={layouts}
        renderActions={actions.renderActions}
        onDeleteSelected={actions.onDeleteSelected}
        columns={columns}
        data={rows}
        loading={loading}
        total={total}
        truncated={truncated}
        rowKey={(r) => r.id}
      />
      {actions.dialog}
    </QueryShell>
  );
}
