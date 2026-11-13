// src/components/query/pages/ApVoucherQuery.tsx
// AP 应付凭证查询

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";
import { z } from "zod";
import { commands, type ApVoucherRowDto, type ApVoucherEdit } from "@/bindings";
import { DataTable, dataTableFeatures } from "@/features/query/DataTable";
import { formatMoney } from "@/features/query/format";
import { DateRangeField, SelectField, TextField, defaultDateRange } from "@/features/query/fields";
import { useVoucherActions } from "@/features/query/useVoucherActions";
import { LayoutSelect } from "@/features/query/LayoutSelect";
import { QueryShell } from "@/features/query/QueryShell";
import { useTableLayouts } from "@/features/query/table-layout";
import { useQueryPage } from "@/features/query/useQueryPage";
import { AP_INV_TYPE_OPTIONS, enumLabel, TAX_CERTIFIED_OPTIONS } from "@/lib/enum-options";

const schema = z
  .object({
    invoiceNoDisplay: z.string(),
    vendorName: z.string(),
    voucherNo: z.string(),
    entryDateFrom: z.string(),
    entryDateTo: z.string(),
    invType: z.array(z.number()),
    vendorCode: z.string(),
    poNo: z.string(),
    barcode: z.string(),
    entryNo: z.string(),
    taxCertified: z.array(z.number()),
    invoiceDateFrom: z.string(),
    invoiceDateTo: z.string(),
    receivedDateFrom: z.string(),
    receivedDateTo: z.string(),
    preparer: z.string(),
    remark: z.string(),
  })
  .refine(
    (v) => !v.entryDateFrom || !v.entryDateTo || v.entryDateFrom <= v.entryDateTo,
    { message: "开始日期不能晚于结束日期", path: ["entryDateTo"] },
  );

type FilterValues = z.infer<typeof schema>;

const dateRange = defaultDateRange();

const defaultValues: FilterValues = {
  invoiceNoDisplay: "",
  vendorName: "",
  voucherNo: "",
  entryDateFrom: dateRange.from,
  entryDateTo: dateRange.to,
  invType: [],
  vendorCode: "",
  poNo: "",
  barcode: "",
  entryNo: "",
  taxCertified: [],
  invoiceDateFrom: "",
  invoiceDateTo: "",
  receivedDateFrom: "",
  receivedDateTo: "",
  preparer: "",
  remark: "",
};

export default function ApVoucherQuery() {
  const { rows, total, truncated, loading, error, search, refresh } =
    useQueryPage<FilterValues, ApVoucherRowDto>({
      queryFn: commands.queryApVouchers,
    });

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search(value),
  });

  const actions = useVoucherActions<ApVoucherRowDto, ApVoucherEdit>({
    title: "编辑应付凭证",
    fields: [
      { key: "barcode", label: "Barcode", type: "text" },
      { key: "entryNo", label: "入账号码", type: "text" },
      { key: "poNo", label: "PO No.", type: "text" },
      { key: "vendorCode", label: "Vendor Code", type: "text", required: true },
      { key: "vendorName", label: "Vendor Name", type: "text", required: true },
      { key: "voucherNo", label: "凭证号", type: "text", required: true },
      { key: "entryDate", label: "入账日期", type: "date" },
      { key: "remark", label: "备注", type: "text" },
      { key: "taxCertified", label: "税务认证", type: "enum", options: TAX_CERTIFIED_OPTIONS },
    ],
    toInitial: (r) => ({
      barcode: r.barcode,
      entryNo: r.entryNo,
      poNo: r.poNo,
      vendorCode: r.vendorCode,
      vendorName: r.vendorName,
      voucherNo: r.voucherNo,
      entryDate: r.entryDate,
      remark: r.remark,
      taxCertified: r.taxCertified,
    }),
    toEdit: (r, v) => ({
      id: r.id,
      barcode: String(v.barcode ?? ""),
      entryNo: String(v.entryNo ?? ""),
      poNo: String(v.poNo ?? ""),
      vendorCode: String(v.vendorCode),
      vendorName: String(v.vendorName),
      voucherNo: String(v.voucherNo),
      entryDate: String(v.entryDate ?? ""),
      remark: String(v.remark ?? ""),
      taxCertified: Number(v.taxCertified),
    }),
    update: commands.updateApVoucher,
    remove: commands.deleteApVouchers,
    refresh,
  });

  const layouts = useTableLayouts("ap-voucher");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, ApVoucherRowDto>[]>(
    () => [
      { accessorKey: "invoiceNoDisplay", header: "发票号码" },
      { accessorKey: "vendorName", header: "供应商名称" },
      { accessorKey: "vendorCode", header: "供应商编码" },
      { accessorKey: "voucherNo", header: "凭证号" },
      { accessorKey: "entryDate", header: "入账日期" },
      { accessorKey: "invoiceDate", header: "开票日期" },
      {
        accessorKey: "amount",
        header: "金额",
        cell: ({ row }) => formatMoney(row.original.amount),
      },
      { accessorKey: "taxRate", header: "税率" },
      {
        accessorKey: "tax",
        header: "税额",
        cell: ({ row }) => formatMoney(row.original.tax),
      },
      {
        accessorKey: "total",
        header: "价税合计",
        cell: ({ row }) => formatMoney(row.original.total),
      },
      {
        accessorKey: "invType",
        header: "发票类型",
        cell: ({ row }) => enumLabel(AP_INV_TYPE_OPTIONS, row.original.invType),
      },
      { accessorKey: "poNo", header: "PO号" },
      { accessorKey: "barcode", header: "条码" },
      { accessorKey: "entryNo", header: "入账号码" },
      {
        accessorKey: "taxCertified",
        header: "是否认证",
        cell: ({ row }) => enumLabel(TAX_CERTIFIED_OPTIONS, row.original.taxCertified),
      },
      { accessorKey: "receivedDate", header: "接收日期" },
      { accessorKey: "preparer", header: "制表人" },
      { accessorKey: "remark", header: "备注" },
      { accessorKey: "invoiceNumber", header: "关联系统发票号" },
      { accessorKey: "id", header: "ID" },
      { accessorKey: "headerId", header: "发票头ID" },
      { accessorKey: "vendorId", header: "供应商ID" },
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
          <form.Field name="entryDateFrom">
            {(fromF) => (
              <form.Field name="entryDateTo">
                {(toF) => (
                  <DateRangeField label="入账日期" fromField={fromF} toField={toF} />
                )}
              </form.Field>
            )}
          </form.Field>
          <form.Field name="invoiceNoDisplay">
            {(f) => <TextField field={f} label="发票号码" />}
          </form.Field>
          <form.Field name="vendorName">
            {(f) => <TextField field={f} label="供应商名称" />}
          </form.Field>
          <form.Field name="voucherNo">
            {(f) => <TextField field={f} label="凭证号" />}
          </form.Field>
        </>
      }
      advancedFields={
        <>
          <form.Field name="invType">
            {(f) => <SelectField field={f} label="发票类型" options={AP_INV_TYPE_OPTIONS} />}
          </form.Field>
          <form.Field name="vendorCode">
            {(f) => <TextField field={f} label="供应商编码" />}
          </form.Field>
          <form.Field name="poNo">
            {(f) => <TextField field={f} label="PO号" />}
          </form.Field>
          <form.Field name="barcode">
            {(f) => <TextField field={f} label="条码" />}
          </form.Field>
          <form.Field name="entryNo">
            {(f) => <TextField field={f} label="入账号码" />}
          </form.Field>
          <form.Field name="taxCertified">
            {(f) => (
              <SelectField field={f} label="是否认证" options={TAX_CERTIFIED_OPTIONS} />
            )}
          </form.Field>
          <form.Field name="invoiceDateFrom">
            {(fromF) => (
              <form.Field name="invoiceDateTo">
                {(toF) => (
                  <DateRangeField label="开票日期" fromField={fromF} toField={toF} />
                )}
              </form.Field>
            )}
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
          <form.Field name="remark">
            {(f) => <TextField field={f} label="备注" />}
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
