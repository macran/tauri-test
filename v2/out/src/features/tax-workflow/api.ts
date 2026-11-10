import { invoke } from "@tauri-apps/api/core";
import { save } from "@tauri-apps/plugin-dialog";
import type {
  ApplyResult,
  ConfirmedQuery,
  ConfirmedStampRow,
  ConfirmedTransferRow,
  ConfirmInput,
  ExportResult,
  GroupLevel,
  MatchSummary,
  PeriodRange,
  RunEvent,
  StampAction,
  StampCategory,
  StampConfirmedQuery,
  StampLinesPayload,
  StampSummaryRow,
  StampTemplateSettings,
  TaxModule,
  TaxRun,
  TransferAction,
  TransferLinesPayload,
} from "./types";

// 后端错误为 String；统一转成 Error 便于 toast
async function call<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  try {
    return await invoke<T>(cmd, args);
  } catch (e) {
    throw new Error(typeof e === "string" ? e : e instanceof Error ? e.message : JSON.stringify(e));
  }
}

export const twApi = {
  listRuns: (module: TaxModule) => call<TaxRun[]>("tw_list_runs", { module }),
  findRun: (module: TaxModule, filingPeriod: PeriodRange) => call<TaxRun | null>("tw_find_run", { module, filingPeriod }),
  listEvents: (runId: number) => call<RunEvent[]>("tw_list_events", { runId }),
  isAdmin: () => call<boolean>("tw_is_admin"),
  unlockRun: (runId: number, reason: string) => call<TaxRun>("tw_unlock_run", { runId, reason }),
  relockRun: (runId: number, reason: string) => call<TaxRun>("tw_relock_run", { runId, reason }),
  deleteRun: (runId: number) => call<void>("tw_delete_run", { runId }),
};

export const ittApi = {
  match: (filingPeriod: PeriodRange, sourcePeriod: PeriodRange) => call<MatchSummary>("itt_match", { filingPeriod, sourcePeriod }),
  listLines: (runId: number) => call<TransferLinesPayload>("itt_list_lines", { runId }),
  applyOverride: (runId: number, detailIds: number[], action: TransferAction, remark?: string | null) =>
    call<ApplyResult>("itt_apply_override", { runId, detailIds, action, remark: remark ?? null }),
  exportPending: (runId: number, path: string) => call<ExportResult>("itt_export_pending", { runId, path }),
  exportConfirmedBatch: (runId: number, batchNo: number, path: string) =>
    call<ExportResult>("itt_export_confirmed_batch", { runId, batchNo, path }),
  confirm: (input: ConfirmInput) => call<TaxRun>("itt_confirm", { input }),
  queryConfirmed: (query: ConfirmedQuery) => call<ConfirmedTransferRow[]>("itt_query_confirmed", { query }),
  exportConfirmedQuery: (query: ConfirmedQuery, path: string) =>
    call<ExportResult>("itt_export_confirmed_query", { query, path }),
};

export const stApi = {
  listCategories: () => call<StampCategory[]>("st_list_categories"),
  match: (filingPeriod: PeriodRange, sourcePeriod: PeriodRange) => call<MatchSummary>("st_match", { filingPeriod, sourcePeriod }),
  listLines: (runId: number) => call<StampLinesPayload>("st_list_lines", { runId }),
  applyOverride: (runId: number, detailIds: number[], action: StampAction, remark?: string | null) =>
    call<ApplyResult>("st_apply_override", { runId, detailIds, action, remark: remark ?? null }),
  exportPending: (runId: number, path: string) => call<ExportResult>("st_export_pending", { runId, path }),
  exportConfirmedBatch: (runId: number, batchNo: number, path: string) =>
    call<ExportResult>("st_export_confirmed_batch", { runId, batchNo, path }),
  confirm: (input: ConfirmInput) => call<TaxRun>("st_confirm", { input }),
  summary: (runId: number, level: GroupLevel, batchNo: number | null, includePending: boolean) =>
    call<StampSummaryRow[]>("st_summary", { runId, level, batchNo, includePending }),
  getTemplateSettings: () => call<StampTemplateSettings>("st_get_template_settings"),
  saveTemplateSettings: (settings: StampTemplateSettings) => call<void>("st_save_template_settings", { settings }),
  exportCollectionForm: (runId: number, batchNo: number | null, path: string) =>
    call<ExportResult>("st_export_collection_form", { runId, batchNo, path }),
  queryConfirmed: (query: StampConfirmedQuery) => call<ConfirmedStampRow[]>("st_query_confirmed", { query }),
  exportConfirmedQuery: (query: StampConfirmedQuery, path: string) =>
    call<ExportResult>("st_export_confirmed_query", { query, path }),
};

/** 弹出保存对话框，返回路径；用户取消返回 null */
export async function pickSavePath(defaultName: string): Promise<string | null> {
  const p = await save({ defaultPath: defaultName, filters: [{ name: "Excel", extensions: ["xlsx"] }] });
  return p ?? null;
}
