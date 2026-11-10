// src/components/query/invoice-columns.tsx
//
// 发票头 + 各明细表共用的列定义：每张表展示的前 13 列都来自发票头。
// 发票头表的行本身就带这些字段（getHead = 行本身），明细表的行把它们放在 row.head 里。

import type { ColumnDef, RowData } from "@tanstack/react-table";
import type { InvoiceHead } from "@/bindings";
import type { dataTableFeatures } from "@/components/query/DataTable";
import {
  enumLabel,
  INV_SOURCE_OPTIONS,
  INV_STATE_OPTIONS,
  INV_TYPE_OPTIONS,
  IS_POSITIVE_OPTIONS,
  RISK_LEVEL_OPTIONS,
} from "@/lib/enum-options";

type Features = typeof dataTableFeatures;

/** 列定义里 accessorKey 的类型约束对运行时无意义，统一在这里放宽 */
export function textCol<T extends RowData>(key: string, header: string, size?: number): ColumnDef<Features, T> {
  return { accessorKey: key, header, size } as ColumnDef<Features, T>;
}

export function headColumns<T extends RowData>(getHead: (row: T) => InvoiceHead): ColumnDef<Features, T>[] {
  const text = (id: keyof InvoiceHead, header: string, size: number): ColumnDef<Features, T> => ({
    id,
    header,
    size,
    accessorFn: (r) => getHead(r)[id],
  });
  const enumCol = (
    id: "source" | "invType" | "state" | "isPos" | "risk",
    header: string,
    options: { value: number; label: string }[],
    size: number,
  ): ColumnDef<Features, T> => ({
    id,
    header,
    size,
    accessorFn: (r) => getHead(r)[id],
    cell: ({ row }) => enumLabel(options, getHead(row.original)[id]),
  });

  return [
    text("code", "发票代码", 110),
    text("num", "发票号码", 100),
    text("eNum", "数电发票号码", 170),
    text("sellerTaxNo", "销方识别号", 160),
    text("sellerName", "销方名称", 220),
    text("issueDate", "开票日期", 100),
    enumCol("source", "发票来源", INV_SOURCE_OPTIONS, 90),
    enumCol("invType", "发票票种", INV_TYPE_OPTIONS, 200),
    enumCol("state", "发票状态", INV_STATE_OPTIONS, 100),
    enumCol("isPos", "是否正数发票", IS_POSITIVE_OPTIONS, 110),
    enumCol("risk", "发票风险等级", RISK_LEVEL_OPTIONS, 110),
    text("issuer", "开票人", 90),
    text("remark", "备注", 160),
  ];
}

/** 发票头那几列里适合做列筛选的（低基数）：枚举列 + 开票人 */
export const HEAD_FILTERS = {
  source: INV_SOURCE_OPTIONS,
  invType: INV_TYPE_OPTIONS,
  state: INV_STATE_OPTIONS,
  isPos: IS_POSITIVE_OPTIONS,
  risk: RISK_LEVEL_OPTIONS,
  issuer: "auto",
} as const;
