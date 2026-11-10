// 与 src-tauri/src/tax_workflow 的 serde 结构一一对应（camelCase）。
// 若已用 tauri-specta 生成 bindings.ts，可直接改为从 bindings 导入这些类型。

export type TaxModule = "input_tax_transfer" | "stamp_tax";
export type RunStatus = "open" | "confirmed" | "reopened";
export type MatchType = "autoHit" | "noHit" | "reviewed" | "manual";
export type GroupLevel = "child" | "parent";

export interface PeriodRange {
  from: string; // YYYYMM
  to: string;
}

export interface RunBatch {
  id: number;
  batchNo: number;
  createdAt: string;
  exportedAt: string | null;
  exportedBy: string | null;
  exportLineCount: number | null;
  exportTaxTotal: number | null;
  acctVoucherNo: string | null;
  acctPostedDate: string | null;
  acctPreparer: string | null;
  confirmedBy: string | null;
  confirmedAt: string | null;
}

export interface TaxRun {
  id: number;
  module: TaxModule;
  /** 当前批次下匹配过的认证期间/凭证期间范围 */
  srcPeriods: PeriodRange[];
  /** 创建匹配批次时确定，确认后保持不变 */
  filingPeriod: PeriodRange | null;
  status: RunStatus;
  createdBy: string;
  createdAt: string;
  lastMatchedAt: string | null;
  lastModifiedAt: string | null;
  batches: RunBatch[];
}

export interface RunEvent {
  id: number;
  event: string;
  detail: string;
  actor: string;
  at: string;
}

export interface LineStats {
  totalLines: number;
  autoHitCount: number;
  includedCount: number;
  includedTax: number;
  pendingCount: number;
  pendingTax: number;
  needsReviewCount: number;
  invalidCount: number;
  manualCount: number;
  confirmedCount: number;
  confirmedTax: number;
  exported: boolean;
  exportStale: boolean;
}

export interface MatchSummary {
  run: TaxRun;
  ruleCount: number;
  candidateCount: number;
  hitCount: number;
  skippedManual: number;
  skippedConfirmed: number;
  skippedOtherRun: number;
}

export interface ApplyResult {
  applied: number;
  skippedLocked: number;
}

export interface ExportResult {
  path: string;
  lineCount: number;
  taxTotal: number;
}

export interface ConfirmInput {
  runId: number;
  acctVoucherNo: string | null;
  acctPostedDate: string | null;
  acctPreparer: string | null;
}

/** 两个模块明细行的公共字段（复核表、批量操作用） */
export interface BaseLine {
  resultId: number | null;
  matchedAt: string | null;
  detailId: number;
  headerId: number;
  invoiceCode: string;
  invoiceNo: string;
  issueDate: string;
  sellerName: string;
  sellerTaxNo: string;
  lineNo: number;
  taxCode: string | null;
  goodsCategory: string | null;
  goodsName: string;
  specModel: string | null;
  amount: number;
  taxRate: string | null;
  voucherNos: string | null;
  voucherDates: string | null;
  autoHit: boolean;
  matchedInOtherPeriod: boolean;
  hitRuleIds: number[];
  hitRules: string | null;
  remark: string | null;
  updatedBy: string | null;
  updatedAt: string | null;
  finalIncluded: boolean;
  matchType: MatchType;
  needsReview: boolean;
  confirmedBatchNo: number | null;
  locked: boolean;
  pending: boolean;
}

// ───────── 进项税转出 ─────────
export interface TransferLine extends BaseLine {
  certPeriod: string;
  tax: number;
  hasVoucher: boolean;
  hitCategories: string[];
  overrideIncluded: boolean | null;
  overrideTax: number | null;
  finalTax: number;
}

export interface TransferLinesPayload {
  run: TaxRun;
  lines: TransferLine[];
  stats: LineStats;
}

export type TransferAction =
  | { type: "include" }
  | { type: "exclude" }
  | { type: "setTax"; tax: number }
  | { type: "acceptSystem" }
  | { type: "restore" }
  | { type: "setRemark" };

export interface ConfirmedQuery {
  filingFrom?: string | null;
  filingTo?: string | null;
  srcFrom?: string | null;
  srcTo?: string | null;
  keyword?: string | null;
}

export interface ConfirmedTransferRow {
  runId: number;
  filingPeriod: PeriodRange;
  certPeriod: string;
  batchNo: number;
  acctVoucherNo: string | null;
  acctPostedDate: string | null;
  acctPreparer: string | null;
  confirmedBy: string;
  confirmedAt: string;
  detailId: number;
  headerId: number;
  invoiceCode: string;
  invoiceNo: string;
  issueDate: string;
  sellerName: string;
  sellerTaxNo: string;
  lineNo: number;
  taxCode: string | null;
  goodsCategory: string | null;
  goodsName: string;
  amount: number;
  taxRate: string | null;
  detailTax: number;
  transferTax: number;
  matchType: MatchType;
  hitRules: string | null;
  remark: string | null;
  voucherNos: string | null;
  voucherDates: string | null;
}

// ───────── 印花税 ─────────
export interface StampCategory {
  code: string;
  name: string;
  parentCode: string | null;
  ratePpm: number | null;
}

export interface StampLine extends BaseLine {
  vatTax: number;
  poNos: string | null;
  costCenters: string | null;
  hitCategories: string[];
  autoCategory: string | null;
  overrideIncluded: boolean | null;
  overrideCategory: string | null;
  finalCategory: string | null;
  itemName: string | null;
  subItemName: string | null;
  ratePpm: number | null;
  baseAmount: number;
  stampTax: number;
  categoryInvalid: boolean;
}

export interface StampLinesPayload {
  run: TaxRun;
  lines: StampLine[];
  stats: LineStats;
}

export type StampAction =
  | { type: "include" }
  | { type: "exclude" }
  | { type: "setCategory"; categoryCode: string }
  | { type: "acceptSystem" }
  | { type: "restore" }
  | { type: "setRemark" };

export interface StampSummaryRow {
  itemCode: string;
  itemName: string;
  subItemCode: string | null;
  subItemName: string | null;
  ratePpm: number | null;
  lineCount: number;
  baseAmount: number;
  taxAmount: number;
}

export interface TemplateColumns {
  seq: string | null;
  itemCode: string | null;
  itemName: string | null;
  subItemCode: string | null;
  subItemName: string | null;
  periodFrom: string | null;
  periodTo: string | null;
  baseAmount: string | null;
  rate: string | null;
  taxAmount: string | null;
  lineCount: string | null;
}

export interface FixedCell {
  cell: string;
  value: string;
}

export interface StampTemplateSettings {
  templatePath: string | null;
  sheetName: string | null;
  startRow: number;
  groupLevel: GroupLevel;
  columns: TemplateColumns;
  fixedCells: FixedCell[];
}

export interface StampConfirmedQuery {
  filingFrom?: string | null;
  filingTo?: string | null;
  categoryCode?: string | null;
  keyword?: string | null;
}

export interface ConfirmedStampRow {
  runId: number;
  filingPeriod: PeriodRange;
  voucherPeriods: PeriodRange[];
  batchNo: number;
  acctVoucherNo: string | null;
  acctPostedDate: string | null;
  acctPreparer: string | null;
  confirmedBy: string;
  confirmedAt: string;
  detailId: number;
  headerId: number;
  invoiceCode: string;
  invoiceNo: string;
  issueDate: string;
  sellerName: string;
  sellerTaxNo: string;
  lineNo: number;
  goodsName: string;
  categoryCode: string;
  itemName: string;
  subItemName: string | null;
  ratePpm: number;
  baseAmount: number;
  stampTax: number;
  matchType: MatchType;
  hitRules: string | null;
  remark: string | null;
  voucherNos: string | null;
  poNos: string | null;
  costCenters: string | null;
}
