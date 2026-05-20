use rusqlite::{params, Connection};
use crate::tui::app::Message;

// ──────────────────────────────────────────────
// Characters
// ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct CharacterRow {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub personality: String,
    pub speech_style: String,
    pub first_message: String,
    pub background: String,
}

pub fn list_characters(conn: &Connection) -> anyhow::Result<Vec<CharacterRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, slug, name, personality, speech_style, first_message, background FROM characters ORDER BY slug"
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(CharacterRow {
            id: row.get(0)?,
            slug: row.get(1)?,
            name: row.get(2)?,
            personality: row.get(3)?,
            speech_style: row.get(4)?,
            first_message: row.get(5)?,
            background: row.get(6)?,
        })
    })?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

pub fn get_character(conn: &Connection, slug: &str) -> anyhow::Result<Option<CharacterRow>> {
    let mut stmt = conn.prepare(
        "SELECT id, slug, name, personality, speech_style, first_message, background FROM characters WHERE slug=?1"
    )?;
    let row = stmt.query_row(params![slug], |row| {
        Ok(CharacterRow {
            id: row.get(0)?,
            slug: row.get(1)?,
            name: row.get(2)?,
            personality: row.get(3)?,
            speech_style: row.get(4)?,
            first_message: row.get(5)?,
            background: row.get(6)?,
        })
    }).ok();
    Ok(row)
}

pub fn create_character(conn: &Connection, row: &CharacterRow) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO characters (slug, name, personality, speech_style, first_message, background) VALUES (?1,?2,?3,?4,?5,?6)",
        params![row.slug, row.name, row.personality, row.speech_style, row.first_message, row.background],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn delete_character(conn: &Connection, id: i64) -> anyhow::Result<()> {
    conn.execute("DELETE FROM characters WHERE id=?1", params![id])?;
    Ok(())
}

// ──────────────────────────────────────────────
// Worlds
// ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct WorldRow {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub overview: String,
}

pub fn list_worlds(conn: &Connection) -> anyhow::Result<Vec<WorldRow>> {
    let mut stmt = conn.prepare("SELECT id, slug, name, description, overview FROM worlds ORDER BY slug")?;
    let rows = stmt.query_map([], |row| {
        Ok(WorldRow { id: row.get(0)?, slug: row.get(1)?, name: row.get(2)?, description: row.get(3)?, overview: row.get(4)? })
    })?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

pub fn get_world(conn: &Connection, slug: &str) -> anyhow::Result<Option<WorldRow>> {
    let mut stmt = conn.prepare("SELECT id, slug, name, description, overview FROM worlds WHERE slug=?1")?;
    Ok(stmt.query_row(params![slug], |row| {
        Ok(WorldRow { id: row.get(0)?, slug: row.get(1)?, name: row.get(2)?, description: row.get(3)?, overview: row.get(4)? })
    }).ok())
}

pub fn create_world(conn: &Connection, row: &WorldRow) -> anyhow::Result<i64> {
    conn.execute("INSERT INTO worlds (slug, name, description, overview) VALUES (?1,?2,?3,?4)",
        params![row.slug, row.name, row.description, row.overview])?;
    Ok(conn.last_insert_rowid())
}

// ──────────────────────────────────────────────
// Character ⇄ World
// ──────────────────────────────────────────────

pub fn link_character_world(conn: &Connection, char_id: i64, world_id: i64, role: &str) -> anyhow::Result<()> {
    conn.execute("INSERT OR REPLACE INTO character_worlds (character_id, world_id, role) VALUES (?1,?2,?3)",
        params![char_id, world_id, role])?;
    Ok(())
}

pub fn get_character_worlds(conn: &Connection, char_id: i64) -> anyhow::Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT world_id FROM character_worlds WHERE character_id=?1")?;
    let ids: Vec<i64> = stmt.query_map(params![char_id], |row| row.get(0))?.filter_map(|r| r.ok()).collect();
    Ok(ids)
}

pub fn get_world_characters(conn: &Connection, world_id: i64) -> anyhow::Result<Vec<i64>> {
    let mut stmt = conn.prepare("SELECT character_id FROM character_worlds WHERE world_id=?1")?;
    let ids: Vec<i64> = stmt.query_map(params![world_id], |row| row.get(0))?.filter_map(|r| r.ok()).collect();
    Ok(ids)
}

// ──────────────────────────────────────────────
// Locations
// ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LocationRow {
    pub id: i64,
    pub slug: String,
    pub name: String,
    pub description: String,
    pub connects_to: String,
    pub world_id: i64,
}

pub fn list_locations(conn: &Connection, world_id: i64) -> anyhow::Result<Vec<LocationRow>> {
    let mut stmt = conn.prepare("SELECT id, slug, name, description, connects_to, world_id FROM locations WHERE world_id=?1 ORDER BY slug")?;
    let rows = stmt.query_map(params![world_id], |row| {
        Ok(LocationRow { id: row.get(0)?, slug: row.get(1)?, name: row.get(2)?, description: row.get(3)?, connects_to: row.get(4)?, world_id: row.get(5)? })
    })?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

pub fn create_location(conn: &Connection, row: &LocationRow) -> anyhow::Result<i64> {
    conn.execute("INSERT INTO locations (slug, name, description, connects_to, world_id) VALUES (?1,?2,?3,?4,?5)",
        params![row.slug, row.name, row.description, row.connects_to, row.world_id])?;
    Ok(conn.last_insert_rowid())
}

// ──────────────────────────────────────────────
// Lore entries
// ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct LoreRow {
    pub id: i64,
    pub key: String,
    pub triggers: Vec<String>,
    pub content: String,
    pub priority: i32,
    pub selective: bool,
    pub enabled: bool,
}

pub fn list_lore(conn: &Connection) -> anyhow::Result<Vec<LoreRow>> {
    let mut stmt = conn.prepare("SELECT id, key, triggers, content, priority, selective, enabled FROM lore_entries WHERE enabled=1 ORDER BY priority DESC")?;
    let rows = stmt.query_map([], |row| {
        let triggers_str: String = row.get(2)?;
        let triggers: Vec<String> = serde_json::from_str(&triggers_str).unwrap_or_default();
        Ok(LoreRow {
            id: row.get(0)?, key: row.get(1)?, triggers,
            content: row.get(3)?, priority: row.get(4)?,
            selective: row.get::<_, i32>(5)? != 0,
            enabled: row.get::<_, i32>(6)? != 0,
        })
    })?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

pub fn get_lore(conn: &Connection, key: &str) -> anyhow::Result<Option<LoreRow>> {
    let mut stmt = conn.prepare("SELECT id, key, triggers, content, priority, selective, enabled FROM lore_entries WHERE key=?1")?;
    Ok(stmt.query_row(params![key], |row| {
        let triggers_str: String = row.get(2)?;
        let triggers: Vec<String> = serde_json::from_str(&triggers_str).unwrap_or_default();
        Ok(LoreRow {
            id: row.get(0)?, key: row.get(1)?, triggers,
            content: row.get(3)?, priority: row.get(4)?,
            selective: row.get::<_, i32>(5)? != 0,
            enabled: row.get::<_, i32>(6)? != 0,
        })
    }).ok())
}

pub fn create_lore(conn: &Connection, key: &str, triggers: &[String], content: &str, priority: i32, selective: bool) -> anyhow::Result<i64> {
    let triggers_json = serde_json::to_string(triggers)?;
    conn.execute(
        "INSERT INTO lore_entries (key, triggers, content, priority, selective) VALUES (?1,?2,?3,?4,?5)",
        params![key, triggers_json, content, priority, selective as i32],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn link_lore_world(conn: &Connection, lore_id: i64, world_id: i64) -> anyhow::Result<()> {
    conn.execute("INSERT OR IGNORE INTO lore_worlds (lore_id, world_id) VALUES (?1,?2)", params![lore_id, world_id])?;
    Ok(())
}

pub fn link_lore_character(conn: &Connection, lore_id: i64, char_id: i64) -> anyhow::Result<()> {
    conn.execute("INSERT OR IGNORE INTO lore_characters (lore_id, character_id) VALUES (?1,?2)", params![lore_id, char_id])?;
    Ok(())
}

// ──────────────────────────────────────────────
// States (generic for character/world/location)
// ──────────────────────────────────────────────

pub fn list_states(conn: &Connection, table: &str, owner_id: i64) -> anyhow::Result<Vec<(String, String, String)>> {
    let sql = format!("SELECT category, key, data FROM {} WHERE {}_id=?1 ORDER BY seq", table, table.strip_suffix("_states").unwrap_or(table));
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![owner_id], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?, row.get::<_, String>(2)?))
    })?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

pub fn manage_state(conn: &Connection, table: &str, owner_id: i64, action: &str, category: &str, key: &str, data: &str) -> anyhow::Result<String> {
    let id_col = format!("{}_id", table.strip_suffix("_states").unwrap_or(table));
    match action {
        "add" => {
            conn.execute(
                &format!("INSERT INTO {} ({}, category, key, data) VALUES (?1,?2,?3,?4)", table, id_col),
                params![owner_id, category, key, data],
            )?;
            Ok(format!("已添加 {}: {}", category, key))
        }
        "update" => {
            let updated = conn.execute(
                &format!("UPDATE {} SET data=?1 WHERE {}=?2 AND category=?3 AND key=?4", table, id_col),
                params![data, owner_id, category, key],
            )?;
            if updated == 0 {
                conn.execute(
                    &format!("INSERT INTO {} ({}, category, key, data) VALUES (?1,?2,?3,?4)", table, id_col),
                    params![owner_id, category, key, data],
                )?;
            }
            Ok(format!("已更新 {}: {}", category, key))
        }
        "delete" => {
            conn.execute(
                &format!("DELETE FROM {} WHERE {}=?1 AND category=?2 AND key=?3", table, id_col),
                params![owner_id, category, key],
            )?;
            Ok(format!("已删除 {}: {}", category, key))
        }
        _ => {
            let mut stmt = conn.prepare(
                &format!("SELECT category, key, data FROM {} WHERE {}=?1 AND category=?2", table, id_col)
            )?;
            let rows: Vec<String> = stmt.query_map(params![owner_id, category], |row| {
                let k: String = row.get(1)?;
                let d: String = row.get(2)?;
                if key.is_empty() || k.to_lowercase().contains(&key.to_lowercase()) {
                    Ok(Some(format!("- {} | {}", k, d)))
                } else {
                    Ok(None)
                }
            })?.filter_map(|r| r.ok().flatten()).collect();
            if rows.is_empty() {
                Ok(format!("未找到匹配的 {}", category))
            } else {
                Ok(rows.join("\n"))
            }
        }
    }
}

// ──────────────────────────────────────────────
// Timeline
// ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct TimelineEntry {
    pub id: i64,
    pub world_id: i64,
    pub time_label: String,
    pub description: String,
    pub created_at: String,
}

pub fn advance_timeline(conn: &Connection, world_id: i64, time_label: &str, description: &str) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO timeline (world_id, time_label, description) VALUES (?1, ?2, ?3)",
        params![world_id, time_label, description],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn current_timeline(conn: &Connection, world_id: i64) -> anyhow::Result<Option<TimelineEntry>> {
    let mut stmt = conn.prepare("SELECT id, world_id, time_label, description, created_at FROM timeline WHERE world_id=?1 ORDER BY id DESC LIMIT 1")?;
    Ok(stmt.query_row(params![world_id], |row| {
        Ok(TimelineEntry { id: row.get(0)?, world_id: row.get(1)?, time_label: row.get(2)?, description: row.get(3)?, created_at: row.get(4)? })
    }).ok())
}

pub fn list_timeline(conn: &Connection, world_id: i64) -> anyhow::Result<Vec<TimelineEntry>> {
    let mut stmt = conn.prepare("SELECT id, world_id, time_label, description, created_at FROM timeline WHERE world_id=?1 ORDER BY id")?;
    let rows = stmt.query_map(params![world_id], |row| {
        Ok(TimelineEntry { id: row.get(0)?, world_id: row.get(1)?, time_label: row.get(2)?, description: row.get(3)?, created_at: row.get(4)? })
    })?.filter_map(|r| r.ok()).collect();
    Ok(rows)
}

// ──────────────────────────────────────────────
// Sessions (updated for new schema)
// ──────────────────────────────────────────────

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: i64,
    pub name: String,
    pub character_name: String,
    pub world_name: Option<String>,
    pub message_count: i64,
    pub updated_at: String,
}

pub fn create_session(conn: &Connection, name: &str, char_id: i64, world_id: Option<i64>) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO sessions (name, character_id, world_id) VALUES (?1, ?2, ?3)",
        params![name, char_id, world_id],
    )?;
    Ok(conn.last_insert_rowid())
}

pub fn save_messages(conn: &Connection, session_id: i64, messages: &[Message]) -> anyhow::Result<()> {
    conn.execute("DELETE FROM messages WHERE session_id=?1", params![session_id])?;
    let mut stmt = conn.prepare("INSERT INTO messages (session_id, role, content, seq) VALUES (?1,?2,?3,?4)")?;
    for (seq, msg) in messages.iter().enumerate() {
        stmt.execute(params![session_id, msg.role, msg.content, seq as i64])?;
    }
    conn.execute("UPDATE sessions SET updated_at=datetime('now') WHERE id=?1", params![session_id])?;
    Ok(())
}

pub fn load_messages(conn: &Connection, session_id: i64) -> anyhow::Result<Vec<Message>> {
    let mut stmt = conn.prepare("SELECT role, content FROM messages WHERE session_id=?1 ORDER BY seq")?;
    let messages = stmt.query_map(params![session_id], |row| {
        Ok(Message { role: row.get(0)?, content: row.get(1)? })
    })?.filter_map(|r| r.ok()).collect();
    Ok(messages)
}

pub fn list_sessions(conn: &Connection) -> anyhow::Result<Vec<SessionInfo>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, COALESCE(c.name,'?'), COALESCE(w.name,''), COUNT(m.id), s.updated_at
         FROM sessions s
         LEFT JOIN characters c ON c.id=s.character_id
         LEFT JOIN worlds w ON w.id=s.world_id
         LEFT JOIN messages m ON m.session_id=s.id
         GROUP BY s.id ORDER BY s.updated_at DESC"
    )?;
    let sessions = stmt.query_map([], |row| {
        Ok(SessionInfo {
            id: row.get(0)?, name: row.get(1)?,
            character_name: row.get(2)?,
            world_name: if row.get::<_, String>(3)?.is_empty() { None } else { Some(row.get(3)?) },
            message_count: row.get(4)?,
            updated_at: row.get(5)?,
        })
    })?.filter_map(|r| r.ok()).collect();
    Ok(sessions)
}

pub fn get_session(conn: &Connection, id: i64) -> anyhow::Result<Option<SessionInfo>> {
    let sessions = list_sessions(conn)?;
    Ok(sessions.into_iter().find(|s| s.id == id))
}

// ──────────────────────────────────────────────
// User persona
// ──────────────────────────────────────────────

pub fn get_persona(conn: &Connection) -> anyhow::Result<String> {
    Ok(conn.query_row("SELECT value FROM user_persona WHERE key='self'", [], |row| row.get(0)).unwrap_or_default())
}

pub fn set_persona(conn: &Connection, text: &str) -> anyhow::Result<()> {
    conn.execute("UPDATE user_persona SET value=?1 WHERE key='self'", params![text])?;
    Ok(())
}
