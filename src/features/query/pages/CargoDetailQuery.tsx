// src/components/query/pages/CargoDetailQuery.tsx
// 货物运输服务发票明细查询

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";
import { z } from "zod";
import { commands, type CargoDetailRow } from "@/bindings";
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
    vehicleNo: z.string(),
    cargoName: z.string(),
    depart: z.string(),
    arrival: z.string(),
    issueDateFrom: z.string(),
    issueDateTo: z.string(),
    sellerName: z.string(),
  })
  .refine(
    (v) => !v.issueDateFrom || !v.issueDateTo || v.issueDateFrom <= v.issueDateTo,
    { message: "开始日期不能晚于结束日期", path: ["issueDateTo"] },
  );

type FilterValues = z.infer<typeof schema>;

const dateRange = defaultDateRange();

const defaultValues: FilterValues = {
  invoiceNumber: "",
  vehicleNo: "",
  cargoName: "",
  depart: "",
  arrival: "",
  issueDateFrom: dateRange.from,
  issueDateTo: dateRange.to,
  sellerName: "",
};

export default function CargoDetailQuery() {
  const { rows, total, truncated, loading, error, search } =
    useQueryPage<FilterValues, CargoDetailRow>({
      queryFn: commands.queryCargoDetails,
    });

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search(value),
  });

  const layouts = useTableLayouts("cargo-detail");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, CargoDetailRow>[]>(
    () => [
      ...headColumns<CargoDetailRow>((r) => r.head),
      textCol<CargoDetailRow>("transportType", "运输工具种类"),
      textCol<CargoDetailRow>("vehicleNo", "运输工具牌号"),
      textCol<CargoDetailRow>("depart", "起运地"),
      textCol<CargoDetailRow>("arrival", "到达地"),
      textCol<CargoDetailRow>("cargoName", "运输货物名称", 180),
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
          <form.Field name="vehicleNo">
            {(f) => <TextField field={f} label="运输工具牌号" />}
          </form.Field>
          <form.Field name="cargoName">
            {(f) => <TextField field={f} label="运输货物名称" />}
          </form.Field>
        </>
      }
      advancedFields={
        <>
          <form.Field name="depart">
            {(f) => <TextField field={f} label="起运地" />}
          </form.Field>
          <form.Field name="arrival">
            {(f) => <TextField field={f} label="到达地" />}
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
