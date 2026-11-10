// src/components/query/pages/RailwayDetailQuery.tsx
// 铁路电子客票明细查询

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";
import { z } from "zod";
import { commands, type RailwayDetailRow } from "@/bindings";
import { HEAD_FILTERS, headColumns, textCol } from "@/components/query/invoice-columns";
import { DataTable, dataTableFeatures } from "@/components/query/DataTable";
import { DateRangeField, TextField } from "@/components/query/fields";
import { LayoutSelect } from "@/components/query/LayoutSelect";
import { QueryShell } from "@/components/query/QueryShell";
import { useTableLayouts } from "@/components/query/table-layout";
import { useQueryPage } from "@/components/query/useQueryPage";

const schema = z
  .object({
    invoiceNumber: z.string(),
    passengerName: z.string(),
    trainNo: z.string(),
    travelDateFrom: z.string(),
    travelDateTo: z.string(),
    depart: z.string(),
    arrival: z.string(),
    idNo: z.string(),
    sellerName: z.string(),
  })
  .refine(
    (v) => !v.travelDateFrom || !v.travelDateTo || v.travelDateFrom <= v.travelDateTo,
    { message: "开始日期不能晚于结束日期", path: ["travelDateTo"] },
  );

type FilterValues = z.infer<typeof schema>;

const defaultValues: FilterValues = {
  invoiceNumber: "",
  passengerName: "",
  trainNo: "",
  travelDateFrom: "",
  travelDateTo: "",
  depart: "",
  arrival: "",
  idNo: "",
  sellerName: "",
};

export default function RailwayDetailQuery() {
  const { rows, total, truncated, loading, error, search } =
    useQueryPage<FilterValues, RailwayDetailRow>({
      queryFn: commands.queryRailwayDetails,
    });

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search(value),
  });

  const layouts = useTableLayouts("railway-detail");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, RailwayDetailRow>[]>(
    () => [
      ...headColumns<RailwayDetailRow>((r) => r.head),
      textCol<RailwayDetailRow>("passengerName", "旅客姓名"),
      textCol<RailwayDetailRow>("idNo", "有效身份证号", 170),
      textCol<RailwayDetailRow>("depart", "出发地"),
      textCol<RailwayDetailRow>("arrival", "到达地"),
      textCol<RailwayDetailRow>("trainNo", "出行车次"),
      textCol<RailwayDetailRow>("travelDate", "日期"),
      textCol<RailwayDetailRow>("travelTime", "时间"),
    ],
    [],
  );

  return (
    <QueryShell
      title="铁路电子客票明细查询"
      description="按旅客姓名、车次、日期等条件查询"
      searching={loading}
      error={error}
      resultCount={total}
      onSubmit={() => form.handleSubmit()}
      layout={<LayoutSelect layouts={layouts} />}
      onReset={() => form.reset()}
      basicFields={
        <>
          <form.Field name="invoiceNumber">
            {(f) => <TextField field={f} label="发票号码" />}
          </form.Field>
          <form.Field name="passengerName">
            {(f) => <TextField field={f} label="旅客姓名" />}
          </form.Field>
          <form.Field name="trainNo">
            {(f) => <TextField field={f} label="车次" />}
          </form.Field>
          <form.Field name="travelDateFrom">
            {(fromF) => (
              <form.Field name="travelDateTo">
                {(toF) => (
                  <DateRangeField label="出行日期" fromField={fromF} toField={toF} />
                )}
              </form.Field>
            )}
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
