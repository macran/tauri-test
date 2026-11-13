// src/components/query/pages/TollDetailQuery.tsx
// 通行费电子发票明细查询

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";
import { z } from "zod";
import { commands, type TollDetailRow } from "@/bindings";
import { HEAD_FILTERS, headColumns, textCol } from "@/features/query/invoice-columns";
import { DataTable, dataTableFeatures } from "@/features/query/DataTable";
import { DateRangeField, TextField, defaultDateRange } from "@/features/query/fields";
import { LayoutSelect } from "@/features/query/LayoutSelect";
import { QueryShell } from "@/features/query/QueryShell";
import { useTableLayouts } from "@/features/query/table-layout";
import { useQueryPage } from "@/features/query/useQueryPage";

const schema = z
  .object({
    invoiceNumber: z.string(),
    plateNo: z.string(),
    vehicleType: z.string(),
    tollDateFrom: z.string(),
    tollDateTo: z.string(),
    sellerName: z.string(),
  })
  .refine(
    (v) => !v.tollDateFrom || !v.tollDateTo || v.tollDateFrom <= v.tollDateTo,
    { message: "开始日期不能晚于结束日期", path: ["tollDateTo"] },
  );

type FilterValues = z.infer<typeof schema>;

const dateRange = defaultDateRange();

const defaultValues: FilterValues = {
  invoiceNumber: "",
  plateNo: "",
  vehicleType: "",
  tollDateFrom: dateRange.from,
  tollDateTo: dateRange.to,
  sellerName: "",
};

export default function TollDetailQuery() {
  const { rows, total, truncated, loading, error, search } =
    useQueryPage<FilterValues, TollDetailRow>({
      queryFn: commands.queryTollDetails,
    });

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search(value),
  });

  const layouts = useTableLayouts("toll-detail");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, TollDetailRow>[]>(
    () => [
      ...headColumns<TollDetailRow>((r) => r.head),
      textCol<TollDetailRow>("plateNo", "车牌号码"),
      textCol<TollDetailRow>("vehicleType", "车辆类型"),
      textCol<TollDetailRow>("tollDateFrom", "通行日期起"),
      textCol<TollDetailRow>("tollDateTo", "通行日期止"),
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
          <form.Field name="tollDateFrom">
            {(fromF) => (
              <form.Field name="tollDateTo">
                {(toF) => (
                  <DateRangeField label="通行日期" fromField={fromF} toField={toF} />
                )}
              </form.Field>
            )}
          </form.Field>
          <form.Field name="invoiceNumber">
            {(f) => <TextField field={f} label="发票号码" />}
          </form.Field>
          <form.Field name="plateNo">
            {(f) => <TextField field={f} label="车牌号码" />}
          </form.Field>
        </>
      }
      advancedFields={
        <>
          <form.Field name="vehicleType">
            {(f) => <TextField field={f} label="车辆类型" />}
          </form.Field>
          <form.Field name="sellerName">
            {(f) => <TextField field={f} label="销售方名称" />}
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
