// src-tauri/src/config.rs
//
// 所有路径和阈值集中在这里，方便以后改共享盘位置而不用翻其他文件。

/// %LOCALAPPDATA% 下的目录名。
pub const LOCAL_DB_DIR: &str = "invoiceManage";

/// 共享盘根目录。admin.db 在根目录，每个 LE 一个子目录。
pub const SHARED_ROOT: &str = r"\\cnbj1file01\Finance\01. Accounting\InputTax";

/// 用户与 LE 配置数据库的文件名。
pub const ADMIN_DB_FILE: &str = "admin.db";

/// 记录“共享盘上每个文件的版本”的台账文件名，放在本地缓存目录。
pub const SYNC_LEDGER_FILE: &str = "sync-ledger.json";

/// 记录“还没有备份到共享盘的文件”的台账文件名，放在本地缓存目录。
pub const BACKUP_LEDGER_FILE: &str = "backup-pending.json";

/// 单个年度数据库的容量上限。标题栏的容量电池以它为 100%，同时也是次年一月轮换的阈值。
/// 整库要在本机与共享盘之间整文件拷贝，库越大启动/退出越慢，定得太大用户会忍不住强制结束进程；
/// 2 GB 是折中，不是实测值，拿到网络盘的实际拷贝速度后再调。
pub const DB_SIZE_LIMIT_BYTES: u64 = 2 * 1024 * 1024 * 1024;

/// 记录“最近一次全部备份成功的时间”的文件名，放在本地缓存目录。
pub const LAST_BACKUP_FILE: &str = "last-backup.json";

/// 后台补备份的重试间隔（秒）。
pub const BACKUP_RETRY_INTERVAL_SECONDS: u64 = 300;
