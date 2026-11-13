export type ErrorSeverity = "fatal" | "warning";

export type ErrorCode =
  | "local_dir_unavailable"
  | "admin_db_missing"
  | "admin_db_invalid"
  | "user_not_configured"
  | "user_disabled"
  | "no_active_legal_entity"
  | "default_legal_entity_invalid"
  | "business_db_unavailable"
  | "share_unavailable"
  | "sync_failed"
  | "backup_pending"
  | "backup_failed"
  | "local_newer"
  | "local_db_locked"
  | "internal";

export type AppError = {
  code: ErrorCode;
  severity: ErrorSeverity;
  message: string;
  detail?: string | null;
};

export type AppRuntimeState = {
  username: string;
  full_name: string;
  default_le_code: string | null;
  active_le_codes: string[];
  active_le: string;
  dashboard_mode: boolean;
  is_leader: boolean;
  share_available: boolean;
  admin_db_path: string;
  active_db_path: string;
  local_root: string;
  shared_root: string;
};

export type StartupStatus =
  | { phase: "starting"; progress: { percent: number; message: string } | null }
  | {
      phase: "ready";
      runtime: AppRuntimeState;
      warnings: AppError[];
    }
  | { phase: "failed"; error: AppError };

export function isWarning(error: AppError): boolean {
  return error.severity === "warning";
}
