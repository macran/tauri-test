// src-tauri/src/db/admin_operations.rs
//
// admin.db（app_users / legal_entities）的管理操作。所有函数接收调用方已开启的事务，
// 校验失败返回中文错误，由调用方整体回滚。
//
// 守住的不变量（启动流程 load_admin_context 依赖它们，破坏了会让用户进不了程序）：
//   - 至少有一名有效的 leader（否则没人能再管理用户）
//   - 至少有一个有效的 LE
//   - 有效用户的默认 LE 必须存在且有效（否则该用户启动时报错）

use rusqlite::{params, Connection, OptionalExtension};
use serde::{Deserialize, Serialize};
use specta::Type;

type R<T> = Result<T, String>;

fn db(e: rusqlite::Error) -> String {
    e.to_string()
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppUserRow {
    pub id: u32,
    pub username: String,
    pub full_name: String,
    pub default_le_code: Option<String>,
    pub is_active: bool,
    pub is_leader: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LegalEntityRow {
    pub le_code: String,
    pub is_active: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AdminOverview {
    pub users: Vec<AppUserRow>,
    pub legal_entities: Vec<LegalEntityRow>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NewUser {
    pub username: String,
    pub full_name: String,
    /// None = 全 LE 汇总（dashboard）
    pub default_le_code: Option<String>,
    pub is_leader: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UserPatch {
    pub id: u32,
    pub full_name: String,
    pub default_le_code: Option<String>,
    pub is_active: bool,
    pub is_leader: bool,
}

/// 调用者必须是有效的 leader；返回其 id。username 取自 Windows 登录名，不由前端传入。
pub fn require_leader(conn: &Connection, username: &str) -> R<i64> {
    conn.query_row(
        "SELECT id FROM app_users WHERE username = ?1 COLLATE NOCASE AND is_active = 1 AND is_leader = 1",
        [username],
        |r| r.get(0),
    )
    .optional()
    .map_err(db)?
    .ok_or_else(|| "只有 leader 才能管理用户".to_string())
}

pub fn overview(conn: &Connection) -> R<AdminOverview> {
    let users = conn
        .prepare("SELECT id, username, full_name, default_le_code, is_active, is_leader FROM app_users ORDER BY username")
        .map_err(db)?
        .query_map([], |r| {
            Ok(AppUserRow {
                id: r.get::<_, i64>(0)? as u32,
                username: r.get(1)?,
                full_name: r.get(2)?,
                default_le_code: r.get(3)?,
                is_active: r.get::<_, i64>(4)? == 1,
                is_leader: r.get::<_, i64>(5)? == 1,
            })
        })
        .map_err(db)?
        .collect::<Result<_, _>>()
        .map_err(db)?;
    let legal_entities = conn
        .prepare("SELECT le_code, is_active FROM legal_entities ORDER BY le_code")
        .map_err(db)?
        .query_map([], |r| {
            Ok(LegalEntityRow { le_code: r.get(0)?, is_active: r.get::<_, i64>(1)? == 1 })
        })
        .map_err(db)?
        .collect::<Result<_, _>>()
        .map_err(db)?;
    Ok(AdminOverview { users, legal_entities })
}

fn count(conn: &Connection, sql: &str) -> R<i64> {
    conn.query_row(sql, [], |r| r.get(0)).map_err(db)
}

fn ensure_active_leader(conn: &Connection) -> R<()> {
    if count(conn, "SELECT COUNT(*) FROM app_users WHERE is_active = 1 AND is_leader = 1")? == 0 {
        return Err("至少需要保留一名有效的 leader".into());
    }
    Ok(())
}

fn check_default_le(conn: &Connection, le: &Option<String>) -> R<()> {
    let Some(le) = le else { return Ok(()) };
    let ok: bool = conn
        .query_row("SELECT EXISTS (SELECT 1 FROM legal_entities WHERE le_code = ?1 AND is_active = 1)", [le], |r| r.get(0))
        .map_err(db)?;
    if ok { Ok(()) } else { Err(format!("LE {le} 不存在或已停用")) }
}

/// LE 代码会成为数据库文件名（{LE}_{年}.db）和共享盘目录名，只允许字母数字。
fn valid_le(code: &str) -> R<String> {
    let t = code.trim();
    if t.is_empty() || t.len() > 16 || !t.chars().all(|c| c.is_ascii_alphanumeric()) {
        return Err("LE 代码只能由 1–16 位字母或数字组成".into());
    }
    Ok(t.to_string())
}

pub fn add_user(conn: &Connection, u: &NewUser) -> R<()> {
    let username = u.username.trim().to_uppercase();
    let full_name = u.full_name.trim();
    if username.is_empty() || full_name.is_empty() {
        return Err("用户名和姓名不能为空".into());
    }
    check_default_le(conn, &u.default_le_code)?;
    let exists: bool = conn
        .query_row("SELECT EXISTS (SELECT 1 FROM app_users WHERE username = ?1 COLLATE NOCASE)", [&username], |r| r.get(0))
        .map_err(db)?;
    if exists {
        return Err(format!("用户 {username} 已存在"));
    }
    conn.execute(
        "INSERT INTO app_users (username, full_name, default_le_code, is_active, is_leader) VALUES (?1, ?2, ?3, 1, ?4)",
        params![username, full_name, u.default_le_code, u.is_leader],
    )
    .map_err(db)?;
    Ok(())
}

pub fn update_user(conn: &Connection, caller_id: i64, p: &UserPatch) -> R<()> {
    let full_name = p.full_name.trim();
    if full_name.is_empty() {
        return Err("姓名不能为空".into());
    }
    if i64::from(p.id) == caller_id && !p.is_active {
        return Err("不能停用自己".into());
    }
    if p.is_active {
        check_default_le(conn, &p.default_le_code)?;
    }
    let n = conn
        .execute(
            "UPDATE app_users SET full_name = ?2, default_le_code = ?3, is_active = ?4, is_leader = ?5 WHERE id = ?1",
            params![p.id, full_name, p.default_le_code, p.is_active, p.is_leader],
        )
        .map_err(db)?;
    if n == 0 {
        return Err("用户不存在".into());
    }
    ensure_active_leader(conn)
}

pub fn add_le(conn: &Connection, code: &str) -> R<()> {
    let code = valid_le(code)?;
    let exists: bool = conn
        .query_row("SELECT EXISTS (SELECT 1 FROM legal_entities WHERE le_code = ?1)", [&code], |r| r.get(0))
        .map_err(db)?;
    if exists {
        return Err(format!("LE {code} 已存在"));
    }
    conn.execute("INSERT INTO legal_entities (le_code, is_active) VALUES (?1, 1)", [&code]).map_err(db)?;
    Ok(())
}

pub fn set_le_active(conn: &Connection, code: &str, active: bool) -> R<()> {
    if !active {
        let users: Vec<String> = conn
            .prepare("SELECT username FROM app_users WHERE is_active = 1 AND default_le_code = ?1 ORDER BY username")
            .map_err(db)?
            .query_map([code], |r| r.get(0))
            .map_err(db)?
            .collect::<Result<_, _>>()
            .map_err(db)?;
        if !users.is_empty() {
            return Err(format!("有效用户 {} 的默认 LE 是 {code}，请先修改他们的默认 LE", users.join("、")));
        }
    }
    let n = conn
        .execute("UPDATE legal_entities SET is_active = ?2 WHERE le_code = ?1", params![code, active])
        .map_err(db)?;
    if n == 0 {
        return Err(format!("LE {code} 不存在"));
    }
    if count(conn, "SELECT COUNT(*) FROM legal_entities WHERE is_active = 1")? == 0 {
        return Err("至少需要保留一个有效的 LE".into());
    }
    Ok(())
}

/// 改名只改 admin.db（含引用它的用户）；LE 已有数据库文件时是否允许由调用方判断。
/// FK 没有 ON UPDATE CASCADE，所以按「新增 → 迁移引用 → 删除旧的」的顺序。
pub fn rename_le(conn: &Connection, old: &str, new: &str) -> R<()> {
    let new = valid_le(new)?;
    if new == old {
        return Err("新旧 LE 代码相同".into());
    }
    let exists = |c: &str| -> R<bool> {
        conn.query_row("SELECT EXISTS (SELECT 1 FROM legal_entities WHERE le_code = ?1)", [c], |r| r.get(0))
            .map_err(db)
    };
    if !exists(old)? {
        return Err(format!("LE {old} 不存在"));
    }
    if exists(&new)? {
        return Err(format!("LE {new} 已存在"));
    }
    conn.execute(
        "INSERT INTO legal_entities (le_code, is_active) SELECT ?2, is_active FROM legal_entities WHERE le_code = ?1",
        params![old, new],
    )
    .map_err(db)?;
    conn.execute("UPDATE app_users SET default_le_code = ?2 WHERE default_le_code = ?1", params![old, new]).map_err(db)?;
    conn.execute("DELETE FROM legal_entities WHERE le_code = ?1", [old]).map_err(db)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(
            "PRAGMA foreign_keys = ON;
             CREATE TABLE legal_entities (le_code TEXT PRIMARY KEY, is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)));
             CREATE TABLE app_users (id INTEGER PRIMARY KEY, username TEXT NOT NULL COLLATE NOCASE UNIQUE, full_name TEXT NOT NULL,
               default_le_code TEXT, is_active INTEGER NOT NULL DEFAULT 1 CHECK (is_active IN (0, 1)),
               is_leader INTEGER NOT NULL DEFAULT 0 CHECK (is_leader IN (0, 1)),
               FOREIGN KEY (default_le_code) REFERENCES legal_entities(le_code));
             INSERT INTO legal_entities VALUES ('2118', 1), ('1618', 1);
             INSERT INTO app_users (username, full_name, default_le_code, is_leader) VALUES ('BOSS', 'Boss', '2118', 1), ('U1', 'User', '1618', 0);",
        )
        .unwrap();
        conn
    }

    fn patch(id: u32, le: Option<&str>, active: bool, leader: bool) -> UserPatch {
        UserPatch { id, full_name: "x".into(), default_le_code: le.map(Into::into), is_active: active, is_leader: leader }
    }

    #[test]
    fn leader_guards() {
        let c = setup();
        assert!(require_leader(&c, "boss").is_ok());
        assert!(require_leader(&c, "U1").is_err());
        // 不能停用自己，也不能把唯一的 leader 降级
        assert!(update_user(&c, 1, &patch(1, Some("2118"), false, true)).is_err());
        assert!(update_user(&c, 1, &patch(1, Some("2118"), true, false)).is_err());
        assert!(update_user(&c, 1, &patch(2, Some("1618"), true, true)).is_ok());
        assert!(update_user(&c, 1, &patch(1, Some("2118"), true, false)).is_ok());
    }

    #[test]
    fn le_rules() {
        let c = setup();
        assert!(add_le(&c, "bad_code").is_err());
        assert!(add_le(&c, "1784").is_ok());
        assert!(add_user(&c, &NewUser { username: " m1 ".into(), full_name: "N".into(), default_le_code: Some("1784".into()), is_leader: false }).is_ok());
        assert!(add_user(&c, &NewUser { username: "M1".into(), full_name: "N".into(), default_le_code: None, is_leader: false }).is_err());
        // 有效用户的默认 LE 不能停用
        assert!(set_le_active(&c, "1618", false).is_err());
        // 改名同步用户引用
        rename_le(&c, "1618", "1619").unwrap();
        let o = overview(&c).unwrap();
        assert_eq!(o.users.iter().find(|u| u.username == "U1").unwrap().default_le_code.as_deref(), Some("1619"));
        assert!(o.legal_entities.iter().all(|l| l.le_code != "1618"));
    }
}
