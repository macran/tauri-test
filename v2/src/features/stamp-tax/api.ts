import { useMutation } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";

/** 申报类型：11 正常申报 / 21 纳税人自行补正 */
export type DeclareType = "11" | "21";
/** 申报期限类型：00 按期申报 / 01 按次申报 */
export type PeriodType = "00" | "01";

export const DECLARE_TYPES = [
  { code: "11", label: "正常申报" },
  { code: "21", label: "纳税人自行补正" },
] as const satisfies ReadonlyArray<{ code: DeclareType; label: string }>;

export const PERIOD_TYPES = [
  { code: "00", label: "按期申报" },
  { code: "01", label: "按次申报" },
] as const satisfies ReadonlyArray<{ code: PeriodType; label: string }>;

/** 一行 = 采集表一行，已按 (申报期限类型, 税目, 子目) 汇总 */
export interface ReportLine {
  periodType: PeriodType;
  itemCode: string;
  itemName: string;
  subItemCode: string | null;
  subItemName: string | null;
  /** 百万分之一：0.0003 => 300 */
  ratePpm: number;
  /** 计税金额（不含税），单位：分 */
  taxableFen: number;
  /** 可选；不传则取子目名称，否则取税目名称 */
  voucherName?: string | null;
}

export interface StampTaxReport {
  /** "YYYY-MM-DD" */
  periodStart: string;
  periodEnd: string;
  declareType: DeclareType;
  lines: ReportLine[];
}

export interface ReportSummary {
  rowCount: number;
  totalTaxableFen: number;
  totalTaxFen: number;
}

export interface ExportResult {
  outputPath: string;
  summary: ReportSummary;
}

/** 弹出“另存为”对话框并生成采集表；用户取消时返回 null */
export async function exportStampTaxReport(report: StampTaxReport): Promise<ExportResult | null> {
  const outputPath = await save({
    title: "保存印花税采集表",
    defaultPath: `印花税采集表_${report.periodStart}_${report.periodEnd}.xlsx`,
    filters: [{ name: "Excel 工作簿", extensions: ["xlsx"] }],
  });
  if (!outputPath) return null;

  // Rust 端参数 output_path / report，Tauri 默认按 camelCase 传参
  const summary = await invoke<ReportSummary>("generate_stamp_tax_report", { outputPath, report });
  return { outputPath, summary };
}

export function useExportStampTaxReport() {
  return useMutation({ mutationFn: exportStampTaxReport });
}

export const fenToYuan = (fen: number) => (fen / 100).toFixed(2);
