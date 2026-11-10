// src/components/query/pages/IcVoucherQuery.tsx
// IC 凭证查询

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";
import { z } from "zod";
import { commands, type IcVoucherRow, type IcVoucherEdit } from "@/bindings";
import { DataTable, dataTableFeatures } from "@/features/query/DataTable";
import { DateRangeField, TextField, defaultDateRange } from "@/features/query/fields";
import { useVoucherActions } from "@/features/query/useVoucherActions";
import { LayoutSelect } from "@/features/query/LayoutSelect";
import { QueryShell } from "@/features/query/QueryShell";
import { useTableLayouts } from "@/features/query/table-layout";
import { useQueryPage } from "@/features/query/useQueryPage";

const schema = z
  .object({
    invoiceNumber: z.string(),
    documentNo: z.string(),
    legalEntity: z.string(),
    postedDateFrom: z.string(),
    postedDateTo: z.string(),
    sellerName: z.string(),
    originalPo: z.string(),
    billing: z.string(),
    receivedDateFrom: z.string(),
    receivedDateTo: z.string(),
    preparer: z.string(),
  })
  .refine(
    (v) => !v.postedDateFrom || !v.postedDateTo || v.postedDateFrom <= v.postedDateTo,
    { message: "开始日期不能晚于结束日期", path: ["postedDateTo"] },
  );

type FilterValues = z.infer<typeof schema>;

const dateRange = defaultDateRange();

const defaultValues: FilterValues = {
  invoiceNumber: "",
  documentNo: "",
  legalEntity: "",
  postedDateFrom: dateRange.from,
  postedDateTo: dateRange.to,
  sellerName: "",
  originalPo: "",
  billing: "",
  receivedDateFrom: "",
  receivedDateTo: "",
  preparer: "",
};

export default function IcVoucherQuery() {
  const { rows, total, truncated, loading, error, search, refresh } =
    useQueryPage<FilterValues, IcVoucherRow>({
      queryFn: commands.queryIcVouchers,
    });

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search(value),
  });

  const actions = useVoucherActions<IcVoucherRow, IcVoucherEdit>({
    title: "编辑关联方凭证",
    fields: [
      { key: "billing", label: "Billing", type: "text" },
      { key: "documentNo", label: "Document Number", type: "text", required: true },
      { key: "postedDate", label: "Posting Date", type: "date" },
      { key: "originalPo", label: "Original PO", type: "text" },
    ],
    toInitial: (r) => ({
      billing: r.billing,
      documentNo: r.documentNo,
      postedDate: r.postedDate,
      originalPo: r.originalPo,
    }),
    toEdit: (r, v) => ({
      id: r.id,
      billing: String(v.billing ?? ""),
      documentNo: String(v.documentNo),
      postedDate: String(v.postedDate ?? ""),
      originalPo: String(v.originalPo ?? ""),
    }),
    update: commands.updateIcVoucher,
    remove: commands.deleteIcVouchers,
    refresh,
  });

  const layouts = useTableLayouts("ic-voucher");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, IcVoucherRow>[]>(
    () => [
      { accessorKey: "invoiceNumber", header: "发票号码" },
      { accessorKey: "documentNo", header: "Document Number" },
      { accessorKey: "legalEntity", header: "LE" },
      { accessorKey: "postedDate", header: "过账日期" },
      { accessorKey: "sellerName", header: "销售方名称" },
      { accessorKey: "billing", header: "Billing" },
      { accessorKey: "originalPo", header: "Original PO" },
      { accessorKey: "receivedDate", header: "接收日期" },
      { accessorKey: "preparer", header: "制表人" },
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
          <form.Field name="documentNo">
            {(f) => <TextField field={f} label="Document Number" />}
          </form.Field>
          <form.Field name="legalEntity">
            {(f) => <TextField field={f} label="LE" />}
          </form.Field>
        </>
      }
      advancedFields={
        <>
          <form.Field name="sellerName">
            {(f) => <TextField field={f} label="销售方名称" />}
          </form.Field>
          <form.Field name="originalPo">
            {(f) => <TextField field={f} label="Original PO" />}
          </form.Field>
          <form.Field name="billing">
            {(f) => <TextField field={f} label="Billing" />}
          </form.Field>
          <form.Field name="receivedDateFrom">
            {(fromF) => (
              <form.Field name="receivedDateTo">
                {(toF) => (
                  <DateRangeField label="接收日期" fromField={fromF} toField={toF} />
                )}
              </form.Field>
            )}
          </form.Field>
          <form.Field name="preparer">
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
