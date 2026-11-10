// src-tauri/src/db/layout_operations.rs
//
// 查询结果表格列布局（user_layouts 表）的读写。layout 内容是前端的 JSON，
// 后端不解析，只做存取。
//
// 可见性：当前用户可见 = 自己的全部布局 + 他人的公开（is_private = 0）布局。
// 修改/删除/设为默认只允许布局的所有者。
// 默认布局：每个 (scope, owner) 至多一个 is_default = 1，查询页面打开时自动选中；
// 没有设置时用系统默认（由前端按列定义生成，不落库）。

use rusqlite::{params, Connection, Result as SqlResult};
use serde::{Deserialize, Serialize};
use specta::Type;

#[derive(Debug, Clone, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LayoutRecord {
    pub id: u32,
    pub name: String,
    pub owner: String,
    pub layout: String,
    pub is_default: bool,
    pub is_private: bool,
    /// 是否属于当前用户（前端据此决定能否编辑/删除/设默认）
    pub is_mine: bool,
}

pub fn list_layouts(conn: &Connection, scope: &str, user: &str) -> SqlResult<Vec<LayoutRecord>> {
    let mut stmt = conn.prepare(
        "SELECT id, name, owner, layout, is_default, is_private
         FROM user_layouts
         WHERE scope = ?1 AND (owner = ?2 OR is_private = 0)
         ORDER BY (owner = ?2) DESC, name",
    )?;
    let rows = stmt.query_map(params![scope, user], |r| {
        let owner: String = r.get(2)?;
        Ok(LayoutRecord {
            id: r.get::<_, i64>(0)? as u32,
            name: r.get(1)?,
            is_mine: owner == user,
            owner,
            layout: r.get(3)?,
            is_default: r.get::<_, i64>(4)? != 0,
            is_private: r.get::<_, i64>(5)? != 0,
        })
    })?;
    rows.collect()
}

/// 同 scope + owner + name 覆盖（不改变 is_default）。返回布局 id。
pub fn save_layout(
    conn: &Connection,
    scope: &str,
    user: &str,
    name: &str,
    layout: &str,
    is_private: bool,
) -> SqlResult<u32> {
    let id: i64 = conn.query_row(
        "INSERT INTO user_layouts (scope, owner, name, layout, is_private)
         VALUES (?1, ?2, ?3, ?4, ?5)
         ON CONFLICT(scope, owner, name) DO UPDATE SET
            layout = excluded.layout,
            is_private = excluded.is_private,
            updated_at = datetime('now', 'localtime')
         RETURNING id",
        params![scope, user, name, layout, is_private as i64],
        |r| r.get(0),
    )?;
    Ok(id as u32)
}

/// 把自己的某个布局设为该 scope 的默认；id 为 None 表示取消默认（回到系统默认）。
pub fn set_default_layout(
    conn: &mut Connection,
    scope: &str,
    user: &str,
    id: Option<u32>,
) -> Result<(), String> {
    let tx = conn.transaction().map_err(|e| e.to_string())?;
    tx.execute(
        "UPDATE user_layouts SET is_default = 0 WHERE scope = ?1 AND owner = ?2 AND is_default = 1",
        params![scope, user],
    )
    .map_err(|e| e.to_string())?;
    if let Some(id) = id {
        let n = tx
            .execute(
                "UPDATE user_layouts SET is_default = 1
                 WHERE id = ?1 AND scope = ?2 AND owner = ?3",
                params![id, scope, user],
            )
            .map_err(|e| e.to_string())?;
        if n == 0 {
            return Err("只能把自己的布局设为默认".to_string());
        }
    }
    tx.commit().map_err(|e| e.to_string())
}

pub fn delete_layout(conn: &Connection, user: &str, id: u32) -> Result<(), String> {
    let n = conn
        .execute(
            "DELETE FROM user_layouts WHERE id = ?1 AND owner = ?2",
            params![id, user],
        )
        .map_err(|e| e.to_string())?;
    if n == 0 {
        return Err("只能删除自己的布局".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(crate::db::schema::CREATE_USER_LAYOUTS).unwrap();
        conn
    }

    #[test]
    fn private_layouts_are_hidden_from_others_and_only_owner_can_change() {
        let mut conn = setup();
        let a_pub = save_layout(&conn, "s", "A", "pub", "1", false).unwrap();
        let a_priv = save_layout(&conn, "s", "A", "priv", "2", true).unwrap();

        let seen_by_b: Vec<String> = list_layouts(&conn, "s", "B")
            .unwrap()
            .into_iter()
            .map(|l| l.name)
            .collect();
        assert_eq!(seen_by_b, vec!["pub"]);
        assert_eq!(list_layouts(&conn, "s", "A").unwrap().len(), 2);

        assert!(delete_layout(&conn, "B", a_pub).is_err());
        assert!(set_default_layout(&mut conn, "s", "B", Some(a_pub)).is_err());
        assert!(delete_layout(&conn, "A", a_priv).is_ok());
    }

    #[test]
    fn one_default_per_owner_and_scope() {
        let mut conn = setup();
        let x = save_layout(&conn, "s", "A", "x", "1", false).unwrap();
        let y = save_layout(&conn, "s", "A", "y", "2", false).unwrap();
        set_default_layout(&mut conn, "s", "A", Some(x)).unwrap();
        set_default_layout(&mut conn, "s", "A", Some(y)).unwrap();
        let defaults: Vec<String> = list_layouts(&conn, "s", "A")
            .unwrap()
            .into_iter()
            .filter(|l| l.is_default)
            .map(|l| l.name)
            .collect();
        assert_eq!(defaults, vec!["y"]);
        set_default_layout(&mut conn, "s", "A", None).unwrap();
        assert!(list_layouts(&conn, "s", "A").unwrap().iter().all(|l| !l.is_default));
    }

    #[test]
    fn save_overwrites_same_owner_name_only() {
        let conn = setup();
        save_layout(&conn, "s", "A", "n", "1", false).unwrap();
        save_layout(&conn, "s", "A", "n", "2", true).unwrap();
        save_layout(&conn, "s", "B", "n", "3", false).unwrap();
        let a = list_layouts(&conn, "s", "A").unwrap();
        assert_eq!(a.len(), 2); // A 自己的 n + B 公开的 n
        assert_eq!(a.iter().find(|l| l.is_mine).unwrap().layout, "2");
    }
}
