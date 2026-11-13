// src/components/query/pages/PassengerDetailQuery.tsx
// 旅客运输服务发票明细查询

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";
import { z } from "zod";
import { commands, type PassengerDetailRow } from "@/bindings";
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
    depart: z.string(),
    arrival: z.string(),
    travelDateFrom: z.string(),
    travelDateTo: z.string(),
    idNo: z.string(),
    transportType: z.string(),
    sellerName: z.string(),
  })
  .refine(
    (v) => !v.travelDateFrom || !v.travelDateTo || v.travelDateFrom <= v.travelDateTo,
    { message: "开始日期不能晚于结束日期", path: ["travelDateTo"] },
  );

type FilterValues = z.infer<typeof schema>;

const dateRange = defaultDateRange();

const defaultValues: FilterValues = {
  invoiceNumber: "",
  passengerName: "",
  depart: "",
  arrival: "",
  travelDateFrom: dateRange.from,
  travelDateTo: dateRange.to,
  idNo: "",
  transportType: "",
  sellerName: "",
};

export default function PassengerDetailQuery() {
  const { rows, total, truncated, loading, error, search } =
    useQueryPage<FilterValues, PassengerDetailRow>({
      queryFn: commands.queryPassengerDetails,
    });

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search(value),
  });

  const layouts = useTableLayouts("passenger-detail");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, PassengerDetailRow>[]>(
    () => [
      ...headColumns<PassengerDetailRow>((r) => r.head),
      textCol<PassengerDetailRow>("passengerName", "出行人"),
      textCol<PassengerDetailRow>("idNo", "有效身份证号", 170),
      textCol<PassengerDetailRow>("travelDate", "出行日期"),
      textCol<PassengerDetailRow>("depart", "出发地"),
      textCol<PassengerDetailRow>("arrival", "到达地"),
      textCol<PassengerDetailRow>("seatClass", "等级"),
      textCol<PassengerDetailRow>("transportType", "交通工具类型"),
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
          <form.Field name="travelDateFrom">
            {(fromF) => (
              <form.Field name="travelDateTo">
                {(toF) => (
                  <DateRangeField label="出行日期" fromField={fromF} toField={toF} />
                )}
              </form.Field>
            )}
          </form.Field>
          <form.Field name="invoiceNumber">
            {(f) => <TextField field={f} label="发票号码" />}
          </form.Field>
          <form.Field name="passengerName">
            {(f) => <TextField field={f} label="出行人" />}
          </form.Field>
          <form.Field name="depart">
            {(f) => <TextField field={f} label="出发地" />}
          </form.Field>
        </>
      }
      advancedFields={
        <>
          <form.Field name="arrival">
            {(f) => <TextField field={f} label="到达地" />}
          </form.Field>
          <form.Field name="transportType">
            {(f) => <TextField field={f} label="交通工具类型" />}
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
