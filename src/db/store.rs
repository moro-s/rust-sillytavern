use crate::tui::app::Message;
use rusqlite::{params, Connection};

#[derive(Debug, Clone)]
pub struct SessionInfo {
    pub id: i64,
    pub name: String,
    pub character_name: String,
    #[allow(dead_code)]
    pub world_name: Option<String>,
    pub message_count: i64,
    #[allow(dead_code)]
    pub created_at: String,
    pub updated_at: String,
}

/// Create a new session
pub fn create_session(
    conn: &Connection,
    name: &str,
    character_name: &str,
    world_name: Option<&str>,
) -> anyhow::Result<i64> {
    conn.execute(
        "INSERT INTO sessions (name, character_name, world_name) VALUES (?1, ?2, ?3)",
        params![name, character_name, world_name],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Save messages for a session (replaces all messages)
pub fn save_messages(conn: &Connection, session_id: i64, messages: &[Message]) -> anyhow::Result<()> {
    // Wrap in transaction
    conn.execute("DELETE FROM messages WHERE session_id = ?1", params![session_id])?;

    let mut stmt = conn.prepare(
        "INSERT INTO messages (session_id, role, content, seq) VALUES (?1, ?2, ?3, ?4)"
    )?;

    for (seq, msg) in messages.iter().enumerate() {
        stmt.execute(params![session_id, msg.role, msg.content, seq as i64])?;
    }

    conn.execute(
        "UPDATE sessions SET updated_at = datetime('now') WHERE id = ?1",
        params![session_id],
    )?;

    Ok(())
}

/// Load messages for a session
pub fn load_messages(conn: &Connection, session_id: i64) -> anyhow::Result<Vec<Message>> {
    let mut stmt = conn.prepare(
        "SELECT role, content FROM messages WHERE session_id = ?1 ORDER BY seq"
    )?;

    let messages = stmt.query_map(params![session_id], |row| {
        Ok(Message {
            role: row.get(0)?,
            content: row.get(1)?,
        })
    })?
    .filter_map(|r| r.ok())
    .collect();

    Ok(messages)
}

/// List all sessions
pub fn list_sessions(conn: &Connection) -> anyhow::Result<Vec<SessionInfo>> {
    let mut stmt = conn.prepare(
        "SELECT s.id, s.name, s.character_name, s.world_name,
                COUNT(m.id) as msg_count, s.created_at, s.updated_at
         FROM sessions s
         LEFT JOIN messages m ON m.session_id = s.id
         GROUP BY s.id
         ORDER BY s.updated_at DESC"
    )?;

    let sessions = stmt.query_map([], |row| {
        Ok(SessionInfo {
            id: row.get(0)?,
            name: row.get(1)?,
            character_name: row.get(2)?,
            world_name: row.get(3)?,
            message_count: row.get(4)?,
            created_at: row.get(5)?,
            updated_at: row.get(6)?,
        })
    })?
    .filter_map(|r| r.ok())
    .collect();

    Ok(sessions)
}

/// Get a single session by ID
pub fn get_session(conn: &Connection, id: i64) -> anyhow::Result<Option<SessionInfo>> {
    let sessions = list_sessions(conn)?;
    Ok(sessions.into_iter().find(|s| s.id == id))
}
