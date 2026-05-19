use rusqlite::{Connection, Result};

const DB_PATH: &str = "data/tavern.db";

pub fn open() -> Result<Connection> {
    std::fs::create_dir_all("data").ok();
    let conn = Connection::open(DB_PATH)?;
    conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA foreign_keys=ON;")?;
    init(&conn)?;
    seed_defaults(&conn)?;
    Ok(conn)
}

fn init(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS worlds (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL UNIQUE,
            slug        TEXT NOT NULL UNIQUE,
            description TEXT NOT NULL DEFAULT '',
            overview    TEXT NOT NULL DEFAULT '',
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS characters (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            name            TEXT NOT NULL,
            slug            TEXT NOT NULL UNIQUE,
            personality     TEXT NOT NULL DEFAULT '',
            speech_style    TEXT NOT NULL DEFAULT '',
            first_message   TEXT NOT NULL DEFAULT '',
            background      TEXT NOT NULL DEFAULT '',
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS locations (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            name        TEXT NOT NULL,
            slug        TEXT NOT NULL,
            description TEXT NOT NULL DEFAULT '',
            connects_to TEXT NOT NULL DEFAULT '',
            world_id    INTEGER NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now')),
            UNIQUE(slug, world_id)
        );

        CREATE TABLE IF NOT EXISTS lore_entries (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            key         TEXT NOT NULL UNIQUE,
            triggers    TEXT NOT NULL DEFAULT '',
            content     TEXT NOT NULL DEFAULT '',
            priority    INTEGER NOT NULL DEFAULT 0,
            selective   INTEGER NOT NULL DEFAULT 0,
            enabled     INTEGER NOT NULL DEFAULT 1,
            position    TEXT NOT NULL DEFAULT 'system_bottom',
            created_at  TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS character_worlds (
            character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
            world_id     INTEGER NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
            role         TEXT NOT NULL DEFAULT '',
            created_at   TEXT NOT NULL DEFAULT (datetime('now')),
            PRIMARY KEY (character_id, world_id)
        );

        CREATE TABLE IF NOT EXISTS lore_worlds (
            lore_id  INTEGER NOT NULL REFERENCES lore_entries(id) ON DELETE CASCADE,
            world_id INTEGER NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
            PRIMARY KEY (lore_id, world_id)
        );

        CREATE TABLE IF NOT EXISTS lore_characters (
            lore_id      INTEGER NOT NULL REFERENCES lore_entries(id) ON DELETE CASCADE,
            character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
            PRIMARY KEY (lore_id, character_id)
        );

        CREATE TABLE IF NOT EXISTS sessions (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            name            TEXT NOT NULL,
            character_id    INTEGER REFERENCES characters(id) ON DELETE SET NULL,
            world_id        INTEGER REFERENCES worlds(id) ON DELETE SET NULL,
            created_at      TEXT NOT NULL DEFAULT (datetime('now')),
            updated_at      TEXT NOT NULL DEFAULT (datetime('now'))
        );

        CREATE TABLE IF NOT EXISTS messages (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            session_id  INTEGER NOT NULL REFERENCES sessions(id) ON DELETE CASCADE,
            role        TEXT NOT NULL,
            content     TEXT NOT NULL,
            seq         INTEGER NOT NULL,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_messages_session ON messages(session_id, seq);

        CREATE TABLE IF NOT EXISTS character_states (
            id           INTEGER PRIMARY KEY AUTOINCREMENT,
            character_id INTEGER NOT NULL REFERENCES characters(id) ON DELETE CASCADE,
            category     TEXT NOT NULL,
            key          TEXT NOT NULL,
            data         TEXT NOT NULL DEFAULT '{}',
            seq          INTEGER NOT NULL DEFAULT 0,
            created_at   TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_char_states ON character_states(character_id, category);

        CREATE TABLE IF NOT EXISTS world_states (
            id         INTEGER PRIMARY KEY AUTOINCREMENT,
            world_id   INTEGER NOT NULL REFERENCES worlds(id) ON DELETE CASCADE,
            category   TEXT NOT NULL,
            key        TEXT NOT NULL,
            data       TEXT NOT NULL DEFAULT '{}',
            seq        INTEGER NOT NULL DEFAULT 0,
            created_at TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_world_states ON world_states(world_id, category);

        CREATE TABLE IF NOT EXISTS location_states (
            id          INTEGER PRIMARY KEY AUTOINCREMENT,
            location_id INTEGER NOT NULL REFERENCES locations(id) ON DELETE CASCADE,
            category    TEXT NOT NULL,
            key         TEXT NOT NULL,
            data        TEXT NOT NULL DEFAULT '{}',
            seq         INTEGER NOT NULL DEFAULT 0,
            created_at  TEXT NOT NULL DEFAULT (datetime('now'))
        );
        CREATE INDEX IF NOT EXISTS idx_loc_states ON location_states(location_id, category);

        CREATE TABLE IF NOT EXISTS user_persona (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL DEFAULT ''
        );
        "
    )?;
    Ok(())
}

fn seed_defaults(conn: &Connection) -> Result<()> {
    // Ensure default innkeeper character exists
    conn.execute(
        "INSERT OR IGNORE INTO characters (slug, name, personality, speech_style, first_message, background)
         VALUES ('innkeeper', '老酒保',
            '见多识广的退休冒险者，瘸了一条腿但耳目灵通，对客人友善但话里有话',
            '说话慢条斯理，喜欢用动作描写开场，偶尔冒出老派口吻',
            '(继续擦着手中的酒杯，木腿轻轻点地) 嗯。看你这身打扮，是外乡来的吧。',
            '三十年前曾是冒险者，被龙伤了左腿后开了这家酒馆。镇上所有消息都会经过他的吧台。'\n        )",
        [],
    )?;
    // Ensure self persona key exists
    conn.execute(
        "INSERT OR IGNORE INTO user_persona (key, value) VALUES ('self', '')",
        [],
    )?;
    Ok(())
}
