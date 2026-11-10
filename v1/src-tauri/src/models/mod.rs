// src-tauri/src/models/mod.rs

pub mod ap_voucher;
pub mod db; // 改名后的 db.rs（原 db_row.rs）
pub mod dto; // 前端交互用 DTO
pub mod enums; // 所有枚举类型
pub mod query_dto; // 查询页面用的筛选 / 行 DTO
pub mod raw_row; // 原始行结构体（直接映射 Excel 列）
