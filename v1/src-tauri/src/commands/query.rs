// src-tauri/src/commands/query.rs
//
// 各查询页面的 Tauri 命令。只读查询，直接在锁内同步执行（不经过
// spawn_blocking）——SQLite 单条 SELECT 的耗时远小于跨线程调度开销，
// 且避免与写事务的持锁逻辑产生不必要的差异。

use crate::db::query_operations as q;
use crate::models::query_dto::*;
use crate::DbState;

macro_rules! query_command {
    ($fn_name:ident, $filter:ty, $row:ty, $query_fn:path) => {
        #[tauri::command]
        #[specta::specta]
        pub async fn $fn_name(
            filter: $filter,
            db: tauri::State<'_, DbState>,
        ) -> Result<PageResult<$row>, String> {
            let conn = db.conn.lock().unwrap();
            let (rows, total) = $query_fn(&conn, &filter).map_err(|e| e.to_string())?;
            Ok(PageResult { rows, total })
        }
    };
}

query_command!(
    query_invoice_headers,
    InvoiceHeaderFilter,
    InvoiceHeaderRow,
    q::query_invoice_headers
);
query_command!(
    query_invoice_details,
    InvoiceDetailFilter,
    InvoiceDetailRow,
    q::query_invoice_details
);
query_command!(
    query_building_details,
    BuildingDetailFilter,
    BuildingDetailRow,
    q::query_building_details
);
query_command!(
    query_real_estate_details,
    RealEstateDetailFilter,
    RealEstateDetailRow,
    q::query_real_estate_details
);
query_command!(
    query_passenger_details,
    PassengerDetailFilter,
    PassengerDetailRow,
    q::query_passenger_details
);
query_command!(
    query_cargo_details,
    CargoDetailFilter,
    CargoDetailRow,
    q::query_cargo_details
);
query_command!(
    query_air_details,
    AirDetailFilter,
    AirDetailRow,
    q::query_air_details
);
query_command!(
    query_railway_details,
    RailwayDetailFilter,
    RailwayDetailRow,
    q::query_railway_details
);
query_command!(
    query_toll_details,
    TollDetailFilter,
    TollDetailRow,
    q::query_toll_details
);
query_command!(
    query_certified_list,
    CertifiedListFilter,
    CertifiedListRow,
    q::query_certified_list
);
query_command!(
    query_travel_expense,
    TravelExpenseFilter,
    CertifiedListRow,
    q::query_travel_expense
);
query_command!(
    query_bank_vouchers,
    BankVoucherFilter,
    BankVoucherRow,
    q::query_bank_vouchers
);
query_command!(
    query_ap_vouchers,
    ApVoucherFilter,
    ApVoucherRowDto,
    q::query_ap_vouchers
);
query_command!(
    query_ic_vouchers,
    IcVoucherFilter,
    IcVoucherRow,
    q::query_ic_vouchers
);
query_command!(
    query_item_overview,
    ItemOverviewFilter,
    ItemOverviewRow,
    q::query_item_overview
);
