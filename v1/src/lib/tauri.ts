import { invoke } from "@tauri-apps/api/core";
import type { ImportResultDto } from "@/bindings";

// export async function getInvoiceDbStatus() {
//   return await invoke<DbStatusDto>("get_invoice_db_status");
// }

// export async function initInvoiceDb() {
//   return await invoke<DbStatusDto>("init_invoice_db");
// }

export async function importInvoiceExcel(filePath: string) {
  return await invoke<ImportResultDto>("import_invoice_excel", {
    filePath,
  });
}

export type StartupStatus = {
  ready: boolean;
  kind: "localLocked" | "sharedUnreachable" | "localNewer" | "other" | null;
  message: string;
  files: string[];
};

export const retryStartup = () => invoke<StartupStatus>("retry_startup");
export const uploadLocalToShared = () =>
  invoke<StartupStatus>("upload_local_to_shared");
