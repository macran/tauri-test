// src/components/query/pages/RealEstateDetailQuery.tsx
// 不动产经营租赁服务发票明细查询

import { useForm } from "@tanstack/react-form";
import type { ColumnDef } from "@tanstack/react-table";
import { useMemo } from "react";
import { z } from "zod";
import { commands, type RealEstateDetailRow } from "@/bindings";
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
    certNo: z.string(),
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
  plateNo: "",
  certNo: "",
  sellerName: "",
  issueDateFrom: dateRange.from,
  issueDateTo: dateRange.to,
};

export default function RealEstateDetailQuery() {
  const { rows, total, truncated, loading, error, search } =
    useQueryPage<FilterValues, RealEstateDetailRow>({
      queryFn: commands.queryRealEstateDetails,
    });

  const form = useForm({
    defaultValues,
    validators: { onChange: schema },
    onSubmit: ({ value }) => search(value),
  });

  const layouts = useTableLayouts("real-estate-detail");

  const columns = useMemo<ColumnDef<typeof dataTableFeatures, RealEstateDetailRow>[]>(
    () => [
      ...headColumns<RealEstateDetailRow>((r) => r.head),
      textCol<RealEstateDetailRow>("certNo", "产权证书/不动产权证号", 200),
      textCol<RealEstateDetailRow>("plateNo", "车牌号"),
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
          <form.Field name="plateNo">
            {(f) => <TextField field={f} label="车牌号" />}
          </form.Field>
          <form.Field name="certNo">
            {(f) => <TextField field={f} label="产权证书/不动产权证号" />}
          </form.Field>
        </>
      }
      advancedFields={
        <form.Field name="sellerName">
          {(f) => <TextField field={f} label="销售方名称" />}
        </form.Field>
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
