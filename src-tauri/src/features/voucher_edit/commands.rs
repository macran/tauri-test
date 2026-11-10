// src-tauri/src/commands/voucher_edit.rs
//
// bank / AP / IC 凭证的修改与批量删除命令。规则见 db::voucher_edit_operations。

use super::repository as ops;
use crate::features::query::{ApVoucherEdit, BankVoucherEdit, IcVoucherEdit};
use crate::DbState;

macro_rules! update_command {
    ($fn_name:ident, $edit:ty, $op:path) => {
        #[tauri::command]
        #[specta::specta]
        pub async fn $fn_name(edit: $edit, db: tauri::State<'_, DbState>) -> Result<(), String> {
            let mut conn = db.conn.lock().unwrap();
            $op(&mut conn, &edit)
        }
    };
}

macro_rules! delete_command {
    ($fn_name:ident, $op:path) => {
        #[tauri::command]
        #[specta::specta]
        pub async fn $fn_name(
            ids: Vec<u32>,
            db: tauri::State<'_, DbState>,
        ) -> Result<u32, String> {
            let mut conn = db.conn.lock().unwrap();
            $op(&mut conn, &ids)
        }
    };
}

update_command!(update_bank_voucher, BankVoucherEdit, ops::update_bank_voucher);
update_command!(update_ap_voucher, ApVoucherEdit, ops::update_ap_voucher);
update_command!(update_ic_voucher, IcVoucherEdit, ops::update_ic_voucher);

delete_command!(delete_bank_vouchers, ops::delete_bank_vouchers);
delete_command!(delete_ap_vouchers, ops::delete_ap_vouchers);
delete_command!(delete_ic_vouchers, ops::delete_ic_vouchers);
