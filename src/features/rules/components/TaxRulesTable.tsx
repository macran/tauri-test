import type { ColumnDef } from "@tanstack/react-table";
import { Ban, CircleCheck } from "lucide-react";
import { useMemo, useState } from "react";

import type { TaxMatchRuleRecord } from "@/bindings";

import { Badge } from "@/components/ui/badge";
import { OptionCombobox } from "@/components/ui/option-combobox";
import { DataTable, type dataTableFeatures } from "@/features/query/DataTable";

import { matchTypeLabel, scopeLabel } from "../rules.types";

type Column = ColumnDef<typeof dataTableFeatures, TaxMatchRuleRecord>;

type TaxRulesTableProps = {
  data: TaxMatchRuleRecord[];
  isLoading: boolean;
  onDisable: (rule: TaxMatchRuleRecord) => void;
  /** 表格工具栏里的表级操作（导出）。导出的是库里的启用 / 全部规则，不受表格上的筛选影响 */
  toolbarSlot?: React.ReactNode;
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

const SCOPE_OPTIONS = [
  { value: "input_tax_transfer", label: scopeLabel("input_tax_transfer") },
  { value: "stamp_tax", label: scopeLabel("stamp_tax") },
];
const REVIEW_OPTIONS = [
  { value: "true", label: "是" },
  { value: "false", label: "否" },
];
const STATUS_OPTIONS = [
  { value: "enabled", label: "已启用" },
  { value: "disabled", label: "已禁用" },
];

export function TaxRulesTable({ data, isLoading, onDisable, toolbarSlot }: TaxRulesTableProps) {
  const [scopes, setScopes] = useState<string[]>([]);
  const [reviews, setReviews] = useState<string[]>([]);
  const [statuses, setStatuses] = useState<string[]>([]);

  // 空数组 = 不限
  const filtered = useMemo(
    () =>
      data.filter(
        (r) =>
          (scopes.length === 0 || scopes.includes(r.scope)) &&
          (reviews.length === 0 || reviews.includes(String(r.needsManualReview))) &&
          (statuses.length === 0 || statuses.includes(r.enabled ? "enabled" : "disabled")),
      ),
    [data, scopes, reviews, statuses],
  );

  // accessorFn 返回界面上显示的文字，全局搜索才能搜到"已禁用"、"印花税"这类词
  const columns = useMemo<Column[]>(
    () => [
      { id: "scope", header: "规则所属", size: 110, accessorFn: (r) => scopeLabel(r.scope) },
      { id: "ruleCategory", header: "规则类别", size: 130, accessorFn: (r) => r.ruleCategory },
      { id: "conditions", header: "匹配条件", size: 380, accessorFn: conditionsSummary },
      {
        id: "needsManualReview",
        header: "是否核对",
        size: 90,
        accessorFn: (r) => (r.needsManualReview ? "是" : "否"),
      },
      { id: "note", header: "备注", size: 240, accessorFn: (r) => r.note },
      { id: "createdAt", header: "生效时间", size: 140, accessorFn: (r) => formatDateTime(r.createdAt) },
      { id: "disabledAt", header: "禁用时间", size: 140, accessorFn: (r) => formatDateTime(r.disabledAt) },
      {
        id: "enabled",
        header: "状态",
        size: 100,
        accessorFn: (r) => (r.enabled ? "已启用" : "已禁用"),
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
    [onDisable],
  );

  return (
    <DataTable
      toolbarSlot={toolbarSlot}
      columns={columns}
      data={filtered}
      loading={isLoading}
      total={filtered.length}
      filterSlot={
        <>
          <OptionCombobox className="w-40" placeholder="规则所属" options={SCOPE_OPTIONS} value={scopes} onChange={setScopes} />
          <OptionCombobox className="w-32" placeholder="是否核对" options={REVIEW_OPTIONS} value={reviews} onChange={setReviews} />
          <OptionCombobox className="w-32" placeholder="状态" options={STATUS_OPTIONS} value={statuses} onChange={setStatuses} />
        </>
      }
      rowKey={(r) => r.id}
      rowClassName={(r) => (r.enabled ? undefined : "bg-muted/30 text-muted-foreground")}
    />
  );
}
