//! Tauri 命令：进项税转出（itt_*）、印花税（st_*）、共用纳税所属期结果集（tw_*）
//!
//! ⚠️ 适配点只有下面两个函数：按你项目里实际的数据库 State / 用户名来源修改即可。

use rusqlite::Connection;
use tauri::State;

use crate::tax_workflow::common::{self, *};
use crate::tax_workflow::input_transfer::{self as itt, ConfirmedQuery, ConfirmedTransferRow, TransferAction, TransferLinesPayload};
use crate::tax_workflow::stamp_tax::{
    self as st, ConfirmedStampRow, GroupLevel, StampAction, StampCategory, StampConfirmedQuery, StampLinesPayload,
    StampSummaryRow, StampTemplateSettings,
};

// ───────────────────────── 适配层 ─────────────────────────
// 假设：pub struct DbState(pub parking_lot::Mutex<rusqlite::Connection>);
//       pub struct AppRuntimeState { pub username: String, .. }
use crate::db::DbState;
use crate::state::AppRuntimeState;

fn with_conn<T>(db: &DbState, f: impl FnOnce(&mut Connection) -> anyhow::Result<T>) -> Result<T, String> {
    let mut guard = db.0.lock();
    f(&mut guard).map_err(|e| format!("{e:#}"))
}

fn current_user(rt: &AppRuntimeState) -> String {
    rt.username.clone()
}
// ──────────────────────────────────────────────────────────

type Db<'a> = State<'a, DbState>;
type Rt<'a> = State<'a, AppRuntimeState>;

// ───────────── 纳税所属期结果集（共用） ─────────────

#[tauri::command]
#[specta::specta]
pub async fn tw_list_runs(db: Db<'_>, module: TaxModule) -> Result<Vec<TaxRun>, String> {
    with_conn(&db, |c| common::list_runs(c, module))
}

#[tauri::command]
#[specta::specta]
pub async fn tw_find_run(db: Db<'_>, module: TaxModule, filing_period: PeriodRange) -> Result<Option<TaxRun>, String> {
    with_conn(&db, |c| common::find_run(c, module, &filing_period))
}

#[tauri::command]
#[specta::specta]
pub async fn tw_list_events(db: Db<'_>, run_id: i64) -> Result<Vec<RunEvent>, String> {
    with_conn(&db, |c| common::list_events(c, run_id))
}

#[tauri::command]
#[specta::specta]
pub async fn tw_is_admin(db: Db<'_>, rt: Rt<'_>) -> Result<bool, String> {
    let u = current_user(&rt);
    with_conn(&db, |c| common::is_admin(c, &u))
}

#[tauri::command]
#[specta::specta]
pub async fn tw_unlock_run(db: Db<'_>, rt: Rt<'_>, run_id: i64, reason: String) -> Result<TaxRun, String> {
    let u = current_user(&rt);
    with_conn(&db, |c| {
        let tx = c.transaction()?;
        let run = common::unlock_run(&tx, run_id, &reason, &u)?;
        tx.commit()?;
        Ok(run)
    })
}

#[tauri::command]
#[specta::specta]
pub async fn tw_relock_run(db: Db<'_>, rt: Rt<'_>, run_id: i64, reason: String) -> Result<TaxRun, String> {
    let u = current_user(&rt);
    with_conn(&db, |c| {
        let tx = c.transaction()?;
        let run = common::load_run(&tx, run_id)?;
        let pending = match run.module {
            TaxModule::InputTaxTransfer => itt::pending_count(&tx, run_id)?,
            TaxModule::StampTax => st::pending_count(&tx, run_id)?,
        };
        let run = common::relock_run(&tx, run_id, &reason, &u, pending)?;
        tx.commit()?;
        Ok(run)
    })
}

#[tauri::command]
#[specta::specta]
pub async fn tw_delete_run(db: Db<'_>, run_id: i64) -> Result<(), String> {
    with_conn(&db, |c| {
        let tx = c.transaction()?;
        let run = common::load_run(&tx, run_id)?;
        common::delete_open_run(&tx, run_id, |conn| match run.module {
            TaxModule::InputTaxTransfer => itt::delete_run_data(conn, run_id),
            TaxModule::StampTax => st::delete_run_data(conn, run_id),
        })?;
        tx.commit()?;
        Ok(())
    })
}

// ───────────── 进项税转出 ─────────────

#[tauri::command]
#[specta::specta]
pub async fn itt_match(db: Db<'_>, rt: Rt<'_>, filing_period: PeriodRange, source_period: PeriodRange) -> Result<MatchSummary, String> {
    let u = current_user(&rt);
    with_conn(&db, |c| itt::run_match(c, &filing_period, &source_period, &u))
}

#[tauri::command]
#[specta::specta]
pub async fn itt_list_lines(db: Db<'_>, run_id: i64) -> Result<TransferLinesPayload, String> {
    with_conn(&db, |c| itt::list_lines(c, run_id))
}

#[tauri::command]
#[specta::specta]
pub async fn itt_apply_override(
    db: Db<'_>,
    rt: Rt<'_>,
    run_id: i64,
    detail_ids: Vec<i64>,
    action: TransferAction,
    remark: Option<String>,
) -> Result<ApplyResult, String> {
    let u = current_user(&rt);
    with_conn(&db, |c| itt::apply_override(c, run_id, &detail_ids, &action, remark, &u))
}

#[tauri::command]
#[specta::specta]
pub async fn itt_export_pending(db: Db<'_>, rt: Rt<'_>, run_id: i64, path: String) -> Result<ExportResult, String> {
    let u = current_user(&rt);
    with_conn(&db, |c| itt::export_pending_batch(c, run_id, &path, &u))
}

#[tauri::command]
#[specta::specta]
pub async fn itt_export_confirmed_batch(
    db: Db<'_>,
    rt: Rt<'_>,
    run_id: i64,
    batch_no: i64,
    path: String,
) -> Result<ExportResult, String> {
    let u = current_user(&rt);
    with_conn(&db, |c| itt::export_confirmed_batch(c, run_id, batch_no, &path, &u))
}

#[tauri::command]
#[specta::specta]
pub async fn itt_confirm(db: Db<'_>, rt: Rt<'_>, input: ConfirmInput) -> Result<TaxRun, String> {
    let u = current_user(&rt);
    with_conn(&db, |c| itt::confirm(c, &input, &u))
}

#[tauri::command]
#[specta::specta]
pub async fn itt_query_confirmed(db: Db<'_>, query: ConfirmedQuery) -> Result<Vec<ConfirmedTransferRow>, String> {
    with_conn(&db, |c| itt::query_confirmed(c, &query))
}

#[tauri::command]
#[specta::specta]
pub async fn itt_export_confirmed_query(db: Db<'_>, query: ConfirmedQuery, path: String) -> Result<ExportResult, String> {
    with_conn(&db, |c| itt::export_confirmed_query(c, &query, &path))
}

// ───────────── 印花税 ─────────────

#[tauri::command]
#[specta::specta]
pub async fn st_list_categories(db: Db<'_>) -> Result<Vec<StampCategory>, String> {
    with_conn(&db, |c| st::list_categories(c))
}

#[tauri::command]
#[specta::specta]
pub async fn st_match(db: Db<'_>, rt: Rt<'_>, filing_period: PeriodRange, source_period: PeriodRange) -> Result<MatchSummary, String> {
    let u = current_user(&rt);
    with_conn(&db, |c| st::run_match(c, &filing_period, &source_period, &u))
}

#[tauri::command]
#[specta::specta]
pub async fn st_list_lines(db: Db<'_>, run_id: i64) -> Result<StampLinesPayload, String> {
    with_conn(&db, |c| st::list_lines(c, run_id))
}

#[tauri::command]
#[specta::specta]
pub async fn st_apply_override(
    db: Db<'_>,
    rt: Rt<'_>,
    run_id: i64,
    detail_ids: Vec<i64>,
    action: StampAction,
    remark: Option<String>,
) -> Result<ApplyResult, String> {
    let u = current_user(&rt);
    with_conn(&db, |c| st::apply_override(c, run_id, &detail_ids, &action, remark, &u))
}

#[tauri::command]
#[specta::specta]
pub async fn st_export_pending(db: Db<'_>, rt: Rt<'_>, run_id: i64, path: String) -> Result<ExportResult, String> {
    let u = current_user(&rt);
    with_conn(&db, |c| st::export_pending_batch(c, run_id, &path, &u))
}

#[tauri::command]
#[specta::specta]
pub async fn st_export_confirmed_batch(
    db: Db<'_>,
    rt: Rt<'_>,
    run_id: i64,
    batch_no: i64,
    path: String,
) -> Result<ExportResult, String> {
    let u = current_user(&rt);
    with_conn(&db, |c| st::export_confirmed_batch(c, run_id, batch_no, &path, &u))
}

#[tauri::command]
#[specta::specta]
pub async fn st_confirm(db: Db<'_>, rt: Rt<'_>, input: ConfirmInput) -> Result<TaxRun, String> {
    let u = current_user(&rt);
    with_conn(&db, |c| st::confirm(c, &input, &u))
}

#[tauri::command]
#[specta::specta]
pub async fn st_summary(
    db: Db<'_>,
    run_id: i64,
    level: GroupLevel,
    batch_no: Option<i64>,
    include_pending: bool,
) -> Result<Vec<StampSummaryRow>, String> {
    with_conn(&db, |c| st::summary(c, run_id, level, batch_no, include_pending))
}

#[tauri::command]
#[specta::specta]
pub async fn st_get_template_settings(db: Db<'_>) -> Result<StampTemplateSettings, String> {
    with_conn(&db, |c| st::get_template_settings(c))
}

#[tauri::command]
#[specta::specta]
pub async fn st_save_template_settings(db: Db<'_>, rt: Rt<'_>, settings: StampTemplateSettings) -> Result<(), String> {
    let u = current_user(&rt);
    with_conn(&db, |c| st::save_template_settings(c, &settings, &u))
}

#[tauri::command]
#[specta::specta]
pub async fn st_export_collection_form(
    db: Db<'_>,
    rt: Rt<'_>,
    run_id: i64,
    batch_no: Option<i64>,
    path: String,
) -> Result<ExportResult, String> {
    let u = current_user(&rt);
    with_conn(&db, |c| st::export_collection_form(c, run_id, batch_no, &path, &u))
}

#[tauri::command]
#[specta::specta]
pub async fn st_query_confirmed(db: Db<'_>, query: StampConfirmedQuery) -> Result<Vec<ConfirmedStampRow>, String> {
    with_conn(&db, |c| st::query_confirmed(c, &query))
}

#[tauri::command]
#[specta::specta]
pub async fn st_export_confirmed_query(db: Db<'_>, query: StampConfirmedQuery, path: String) -> Result<ExportResult, String> {
    with_conn(&db, |c| st::export_confirmed_query(c, &query, &path))
}

// 在 lib.rs 的 tauri_specta::Builder 中注册：
//
// let builder = tauri_specta::Builder::<tauri::Wry>::new().commands(tauri_specta::collect_commands![
//     // ...已有命令
//     commands::tax_workflow::tw_list_runs,
//     commands::tax_workflow::tw_find_run,
//     commands::tax_workflow::tw_list_events,
//     commands::tax_workflow::tw_is_admin,
//     commands::tax_workflow::tw_unlock_run,
//     commands::tax_workflow::tw_relock_run,
//     commands::tax_workflow::tw_delete_run,
//     commands::tax_workflow::itt_match,
//     commands::tax_workflow::itt_list_lines,
//     commands::tax_workflow::itt_apply_override,
//     commands::tax_workflow::itt_export_pending,
//     commands::tax_workflow::itt_export_confirmed_batch,
//     commands::tax_workflow::itt_confirm,
//     commands::tax_workflow::itt_query_confirmed,
//     commands::tax_workflow::itt_export_confirmed_query,
//     commands::tax_workflow::st_list_categories,
//     commands::tax_workflow::st_match,
//     commands::tax_workflow::st_list_lines,
//     commands::tax_workflow::st_apply_override,
//     commands::tax_workflow::st_export_pending,
//     commands::tax_workflow::st_export_confirmed_batch,
//     commands::tax_workflow::st_confirm,
//     commands::tax_workflow::st_summary,
//     commands::tax_workflow::st_get_template_settings,
//     commands::tax_workflow::st_save_template_settings,
//     commands::tax_workflow::st_export_collection_form,
//     commands::tax_workflow::st_query_confirmed,
//     commands::tax_workflow::st_export_confirmed_query,
// ]);
// 金额字段为 i64（分）。若 specta 导出报 BigInt 错误，导出配置加
// `.bigint(specta_typescript::BigIntExportBehavior::Number)`（分在 2^53 内是安全的）。
