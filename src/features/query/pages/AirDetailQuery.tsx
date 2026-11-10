// src/components/query/pages/AirDetailQuery.tsx
// 航空运输电子客票行程单明细查询

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";
import { z } from "zod";
import { commands, type AirDetailRow } from "@/bindings";
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
    passengerName: z.string(),
    flightNo: z.string(),
    flightDateFrom: z.string(),
    flightDateTo: z.string(),
    depart: z.string(),
    arrival: z.string(),
    idNo: z.string(),
    sellerName: z.string(),
  })
  .refine(
    (v) => !v.flightDateFrom || !v.flightDateTo || v.flightDateFrom <= v.flightDateTo,
    { message: "开始日期不能晚于结束日期", path: ["flightDateTo"] },
  );

type FilterValues = z.infer<typeof schema>;

const dateRange = defaultDateRange();

const defaultValues: FilterValues = {
  invoiceNumber: "",
  passengerName: "",
  flightNo: "",
  flightDateFrom: dateRange.from,
  flightDateTo: dateRange.to,
  depart: "",
  arrival: "",
  idNo: "",
  sellerName: "",
};

export default function AirDetailQuery() {
  const { rows, total, truncated, loading, error, search } =
    useQueryPage<FilterValues, AirDetailRow>({
      queryFn: commands.queryAirDetails,
    });

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search(value),
  });

  const layouts = useTableLayouts("air-detail");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, AirDetailRow>[]>(
    () => [
      ...headColumns<AirDetailRow>((r) => r.head),
      textCol<AirDetailRow>("passengerName", "旅客姓名"),
      textCol<AirDetailRow>("idNo", "有效身份证号", 170),
      textCol<AirDetailRow>("depart", "出发地"),
      textCol<AirDetailRow>("arrival", "到达地"),
      textCol<AirDetailRow>("carrier", "承运人"),
      textCol<AirDetailRow>("flightNo", "航班号"),
      textCol<AirDetailRow>("flightDate", "日期"),
      textCol<AirDetailRow>("flightTime", "时间"),
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
          <form.Field name="flightDateFrom">
            {(fromF) => (
              <form.Field name="flightDateTo">
                {(toF) => (
                  <DateRangeField label="飞行日期" fromField={fromF} toField={toF} />
                )}
              </form.Field>
            )}
          </form.Field>
          <form.Field name="invoiceNumber">
            {(f) => <TextField field={f} label="发票号码" />}
          </form.Field>
          <form.Field name="passengerName">
            {(f) => <TextField field={f} label="旅客姓名" />}
          </form.Field>
          <form.Field name="flightNo">
            {(f) => <TextField field={f} label="航班号" />}
          </form.Field>
        </>
      }
      advancedFields={
        <>
          <form.Field name="depart">
            {(f) => <TextField field={f} label="出发地" />}
          </form.Field>
          <form.Field name="arrival">
            {(f) => <TextField field={f} label="到达地" />}
          </form.Field>
          <form.Field name="idNo">
            {(f) => <TextField field={f} label="有效身份证号" />}
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
