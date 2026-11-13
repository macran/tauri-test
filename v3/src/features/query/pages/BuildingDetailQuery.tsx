// src/components/query/pages/BuildingDetailQuery.tsx
// 建筑服务发票明细查询

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";
import { z } from "zod";
import { commands, type BuildingDetailRow } from "@/bindings";
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
    projectName: z.string(),
    serviceLocation: z.string(),
    crossRegionNo: z.string(),
    sellerName: z.string(),
    issueDateFrom: z.string(),
    issueDateTo: z.string(),
  })
  .refine(
    (v) => !v.issueDateFrom || !v.issueDateTo || v.issueDateFrom <= v.issueDateTo,
    { message: "开始日期不能晚于结束日期", path: ["issueDateTo"] },
  );

type FilterValues = z.infer<typeof schema>;

const dateRange = defaultDateRange();

const defaultValues: FilterValues = {
  invoiceNumber: "",
  projectName: "",
  serviceLocation: "",
  crossRegionNo: "",
  sellerName: "",
  issueDateFrom: dateRange.from,
  issueDateTo: dateRange.to,
};

export default function BuildingDetailQuery() {
  const { rows, total, truncated, loading, error, search } =
    useQueryPage<FilterValues, BuildingDetailRow>({
      queryFn: commands.queryBuildingDetails,
    });

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search(value),
  });

  const layouts = useTableLayouts("building-detail");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, BuildingDetailRow>[]>(
    () => [
      ...headColumns<BuildingDetailRow>((r) => r.head),
      textCol<BuildingDetailRow>("crossRegionNo", "跨区域涉税事项报验管理编号", 230),
      textCol<BuildingDetailRow>("serviceLocation", "建筑服务发生地", 160),
      textCol<BuildingDetailRow>("projectName", "建筑项目名称", 200),
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
          <form.Field name="projectName">
            {(f) => <TextField field={f} label="建筑项目名称" />}
          </form.Field>
          <form.Field name="serviceLocation">
            {(f) => <TextField field={f} label="建筑服务发生地" />}
          </form.Field>
        </>
      }
      advancedFields={
        <>
          <form.Field name="crossRegionNo">
            {(f) => <TextField field={f} label="跨区域涉税事项报验管理编号" />}
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
