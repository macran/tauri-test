import {
  type ColumnDef,
  createSortedRowModel,
  rowSortingFeature,
  sortFns,
  tableFeatures,
  useTable,
} from "@tanstack/react-table";
import { Ban, CircleCheck } from "lucide-react";
import { useMemo } from "react";

import type { TaxMatchRuleRecord } from "@/bindings";

import { Badge } from "@/components/ui/badge";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

import { matchTypeLabel, scopeLabel } from "../rules.types";

type TaxRulesTableProps = {
  data: TaxMatchRuleRecord[];
  isLoading: boolean;
  onDisable: (rule: TaxMatchRuleRecord) => void;
};

function formatDateTime(value: string | null): string {
  if (!value) {
    return "—";
  }

  const date = new Date(value);
  if (Number.isNaN(date.getTime())) {
    return value;
  }

  const pad = (n: number) => String(n).padStart(2, "0");

  return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())} ${pad(date.getHours())}:${pad(date.getMinutes())}`;
}

function conditionsSummary(rule: TaxMatchRuleRecord): string {
  const parts: string[] = [];

  const dimensions: Array<{
    label: string;
    matchType: string | null;
    values: string | null;
  }> = [
    {
      label: "销售方税号",
      matchType: rule.sellerTaxIdMatchType,
      values: rule.sellerTaxIdValues,
    },
    {
      label: "税收分类编码",
      matchType: rule.taxCodeMatchType,
      values: rule.taxCodeValues,
    },
    {
      label: "税收分类编码简称",
      matchType: rule.taxNameMatchType,
      values: rule.taxNameValues,
    },
  ];

  for (const dimension of dimensions) {
    if (dimension.matchType && dimension.values) {
      parts.push(
        `${dimension.label}：${matchTypeLabel(dimension.matchType)} ${dimension.values}`
      );
    }
  }

  return parts.join("；");
}

// 每列的宽度设计：匹配条件、备注内容通常较长，给更多空间；其他列内容固定且短，够用就好。
const COLUMN_WIDTH_CLASS: Record<string, string> = {
  scope: "w-24",
  ruleCategory: "w-28",
  conditions: "w-[300px]",
  needsManualReview: "w-20",
  note: "w-[220px]",
  createdAt: "w-36",
  disabledAt: "w-36",
  enabled: "w-24",
};

const features = tableFeatures({
  rowSortingFeature,
  sortedRowModel: createSortedRowModel(),
  sortFns,
});

export function TaxRulesTable({
  data,
  isLoading,
  onDisable,
}: TaxRulesTableProps) {
  const columns = useMemo<
    Array<ColumnDef<typeof features, TaxMatchRuleRecord>>
  >(
    () => [
      {
        accessorKey: "scope",
        header: "规则所属",
        cell: ({ row }) => scopeLabel(row.original.scope),
      },
      {
        accessorKey: "ruleCategory",
        header: "规则类别",
      },
      {
        id: "conditions",
        header: "匹配条件",
        enableSorting: false,
        cell: ({ row }) => {
          const summary = conditionsSummary(row.original);

          return (
            <span
              className="block max-w-[300px] truncate text-sm"
              title={summary}
            >
              {summary}
            </span>
          );
        },
      },
      {
        accessorKey: "needsManualReview",
        header: "是否核对",
        cell: ({ row }) => (row.original.needsManualReview ? "是" : "否"),
      },
      {
        accessorKey: "note",
        header: "备注",
        enableSorting: false,
        cell: ({ row }) =>
          row.original.note ? (
            <span
              className="block max-w-[220px] truncate text-sm"
              title={row.original.note}
            >
              {row.original.note}
            </span>
          ) : (
            <span className="text-sm text-muted-foreground">—</span>
          ),
      },
      {
        accessorKey: "createdAt",
        header: "生效时间",
        cell: ({ row }) => formatDateTime(row.original.createdAt),
      },
      {
        accessorKey: "disabledAt",
        header: "禁用时间",
        cell: ({ row }) => formatDateTime(row.original.disabledAt),
      },
      {
        accessorKey: "enabled",
        header: "状态",
        cell: ({ row }) =>
          row.original.enabled ? (
            <Badge
              render={<button type="button" />}
              onClick={() => onDisable(row.original)}
              className="cursor-pointer gap-1 border-emerald-600/30 bg-emerald-100 text-emerald-700 hover:bg-emerald-200 dark:bg-emerald-500/15 dark:text-emerald-400 dark:hover:bg-emerald-500/25"
              title="点击禁用该规则"
            >
              <CircleCheck className="size-3" />
              已启用
            </Badge>
          ) : (
            <Badge
              className="gap-1 border-rose-600/30 bg-rose-100 text-rose-700 dark:bg-rose-500/15 dark:text-rose-400"
              title="已禁用，无法重新启用"
            >
              <Ban className="size-3" />
              已禁用
            </Badge>
          ),
      },
    ],
    [onDisable]
  );

  const table = useTable({
    key: "tax-rules",
    features,
    data,
    columns,
  });

  return (
    <div className="rounded-md border">
      <Table>
        <TableHeader>
          {table.getHeaderGroups().map((headerGroup) => (
            <TableRow key={headerGroup.id}>
              {headerGroup.headers.map((header) => (
                <TableHead
                  key={header.id}
                  className={COLUMN_WIDTH_CLASS[header.column.id]}
                >
                  {header.isPlaceholder ? null : (
                    <button
                      type="button"
                      className="font-medium"
                      onClick={header.column.getToggleSortingHandler()}
                      disabled={!header.column.getCanSort()}
                    >
                      <table.FlexRender header={header} />

                      {header.column.getIsSorted() === "asc" && " ↑"}

                      {header.column.getIsSorted() === "desc" && " ↓"}
                    </button>
                  )}
                </TableHead>
              ))}
            </TableRow>
          ))}
        </TableHeader>

        <TableBody>
          {isLoading ? (
            <TableRow>
              <TableCell colSpan={columns.length} className="h-24 text-center">
                正在读取规则……
              </TableCell>
            </TableRow>
          ) : table.getRowModel().rows.length === 0 ? (
            <TableRow>
              <TableCell
                colSpan={columns.length}
                className="h-24 text-center text-muted-foreground"
              >
                没有找到规则
              </TableCell>
            </TableRow>
          ) : (
            table.getRowModel().rows.map((row) => (
              <TableRow
                key={row.id}
                className={
                  row.original.enabled
                    ? undefined
                    : "bg-muted/30 text-muted-foreground"
                }
              >
                {row.getAllCells().map((cell) => (
                  <TableCell
                    key={cell.id}
                    className={COLUMN_WIDTH_CLASS[cell.column.id]}
                  >
                    <table.FlexRender cell={cell} />
                  </TableCell>
                ))}
              </TableRow>
            ))
          )}
        </TableBody>
      </Table>
    </div>
  );
}
