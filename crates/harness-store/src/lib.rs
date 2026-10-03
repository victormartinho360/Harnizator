//! harness-store: adapter SQLite da port `SessionStore` (spec/07).

use std::path::Path;
use std::sync::Mutex;

use harness_core::events::Event;
use harness_core::store_port::{SessionMeta, SessionStore, StoreError, StoredEvent};
use harness_core::{AgentId, TokenUsage};
use rusqlite::{Connection, params};

const MIGRATIONS: &[&str] = &[
    // m0001: schema inicial
    r#"
    CREATE TABLE IF NOT EXISTS schema_migrations(version INTEGER PRIMARY KEY);
    CREATE TABLE IF NOT EXISTS sessions(
        id TEXT PRIMARY KEY,
        title TEXT NOT NULL,
        model TEXT NOT NULL,
        sandbox_mode TEXT NOT NULL,
        created_at INTEGER NOT NULL,
        updated_at INTEGER NOT NULL,
        input_tokens INTEGER NOT NULL DEFAULT 0,
        output_tokens INTEGER NOT NULL DEFAULT 0
    );
    CREATE TABLE IF NOT EXISTS events(
        id INTEGER PRIMARY KEY AUTOINCREMENT,
        session_id TEXT NOT NULL REFERENCES sessions(id),
        agent_id TEXT NOT NULL,
        seq INTEGER NOT NULL,
        ts INTEGER NOT NULL,
        payload TEXT NOT NULL
    );
    CREATE INDEX IF NOT EXISTS idx_events_session_agent ON events(session_id, agent_id, seq);
    CREATE TABLE IF NOT EXISTS agents(
        session_id TEXT NOT NULL,
        agent_id TEXT NOT NULL,
        status TEXT NOT NULL,
        PRIMARY KEY(session_id, agent_id)
    );
    "#,
];

/// Store SQLite (WAL) com event log append-only.
pub struct SqliteStore {
    conn: Mutex<Connection>,
}

fn now_unix() -> u64 {
    use std::time::{SystemTime, UNIX_EPOCH};
    // monotonicidade do relógio do SO é suficiente para metadata de sessão
    #[allow(clippy::unwrap_used)]
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

impl SqliteStore {
    pub fn open(path: &Path) -> Result<Self, StoreError> {
        let conn = Connection::open(path).map_err(|e| StoreError::Backend(e.to_string()))?;
        let store = Self {
            conn: Mutex::new(conn),
        };
        store.migrate()?;
        Ok(store)
    }

    fn migrate(&self) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.pragma_update(None, "journal_mode", "WAL")
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        let version: u32 = conn
            .query_row(
                "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
                [],
                |r| r.get(0),
            )
            .unwrap_or(0);
        for (i, m) in MIGRATIONS.iter().enumerate() {
            let v = i as u32 + 1;
            if v > version {
                conn.execute_batch(m)
                    .map_err(|e| StoreError::Backend(e.to_string()))?;
                conn.execute(
                    "INSERT OR IGNORE INTO schema_migrations(version) VALUES (?1)",
                    params![v],
                )
                .map_err(|e| StoreError::Backend(e.to_string()))?;
            }
        }
        Ok(())
    }

    pub fn schema_version(&self) -> Result<u32, StoreError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.query_row(
            "SELECT COALESCE(MAX(version), 0) FROM schema_migrations",
            [],
            |r| r.get(0),
        )
        .map_err(|e| StoreError::Backend(e.to_string()))
    }

    fn set_agent_status(&self, session: &str, agent: &str, status: &str) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO agents(session_id, agent_id, status) VALUES (?1, ?2, ?3)
             ON CONFLICT(session_id, agent_id) DO UPDATE SET status = excluded.status",
            params![session, agent, status],
        )
        .map_err(|e| StoreError::Backend(e.to_string()))?;
        Ok(())
    }
}

impl SqliteStore {}

#[allow(clippy::unwrap_used)]
impl SessionStore for SqliteStore {
    fn create_session(
        &self,
        title: &str,
        model: &str,
        sandbox: &str,
    ) -> Result<String, StoreError> {
        let id = format!("s-{}", ulid::Ulid::new().to_string().to_lowercase());
        let now = now_unix();
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "INSERT INTO sessions(id, title, model, sandbox_mode, created_at, updated_at)
             VALUES (?1, ?2, ?3, ?4, ?5, ?5)",
            params![id, title, model, sandbox, now],
        )
        .map_err(|e| StoreError::Backend(e.to_string()))?;
        Ok(id)
    }

    fn append_event(
        &self,
        session: &str,
        agent: &AgentId,
        seq: u64,
        ts: u64,
        event: &Event,
    ) -> Result<(), StoreError> {
        let payload =
            serde_json::to_string(event).map_err(|e| StoreError::Backend(e.to_string()))?;
        {
            let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
            conn.execute(
                "INSERT INTO events(session_id, agent_id, seq, ts, payload)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![session, agent.as_str(), seq, ts, payload],
            )
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        }
        // rastreia status do agente para crash recovery
        match event {
            Event::AgentSpawned { agent: a, .. } => {
                self.set_agent_status(session, a.as_str(), "running")?;
            }
            Event::AgentFinished { agent: a } => {
                self.set_agent_status(session, a.as_str(), "done")?;
            }
            Event::AgentInterrupted { agent: a } => {
                self.set_agent_status(session, a.as_str(), "interrupted")?;
            }
            _ => {}
        }
        Ok(())
    }

    fn events(&self, session: &str) -> Result<Vec<StoredEvent>, StoreError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn
            .prepare(
                "SELECT agent_id, seq, ts, payload FROM events
                 WHERE session_id = ?1 ORDER BY agent_id, seq",
            )
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        let rows = stmt
            .query_map(params![session], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, u64>(1)?,
                    r.get::<_, u64>(2)?,
                    r.get::<_, String>(3)?,
                ))
            })
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            let (agent, seq, ts, payload) = row.map_err(|e| StoreError::Backend(e.to_string()))?;
            let event: Event =
                serde_json::from_str(&payload).map_err(|e| StoreError::Backend(e.to_string()))?;
            out.push(StoredEvent {
                agent: AgentId::new(agent),
                seq,
                ts,
                event,
            });
        }
        Ok(out)
    }

    fn sessions(&self) -> Result<Vec<SessionMeta>, StoreError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        let mut stmt = conn
            .prepare(
                "SELECT id, title, model, sandbox_mode, created_at, updated_at, input_tokens, output_tokens
                 FROM sessions ORDER BY updated_at DESC",
            )
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        let rows = stmt
            .query_map([], |r| {
                Ok((
                    r.get::<_, String>(0)?,
                    r.get::<_, String>(1)?,
                    r.get::<_, String>(2)?,
                    r.get::<_, String>(3)?,
                    r.get::<_, u64>(4)?,
                    r.get::<_, u64>(5)?,
                    r.get::<_, u64>(6)?,
                    r.get::<_, u64>(7)?,
                ))
            })
            .map_err(|e| StoreError::Backend(e.to_string()))?;
        let mut out = Vec::new();
        for row in rows {
            let (id, title, model, sandbox, created, updated, input, output) =
                row.map_err(|e| StoreError::Backend(e.to_string()))?;
            out.push(SessionMeta {
                id,
                title,
                model,
                sandbox_mode: sandbox,
                created_at: created,
                updated_at: updated,
                usage: TokenUsage { input, output },
            });
        }
        Ok(out)
    }

    fn recover(&self, session: &str) -> Result<u64, StoreError> {
        // agentes ainda running/idle viram interrupted + evento auditável
        let agents: Vec<String> = {
            let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
            let mut stmt = conn
                .prepare(
                    "SELECT agent_id FROM agents
                     WHERE session_id = ?1 AND status IN ('running', 'idle', 'queued')",
                )
                .map_err(|e| StoreError::Backend(e.to_string()))?;
            let rows = stmt
                .query_map(params![session], |r| r.get(0))
                .map_err(|e| StoreError::Backend(e.to_string()))?;
            let mut v = Vec::new();
            for r in rows {
                v.push(r.map_err(|e| StoreError::Backend(e.to_string()))?);
            }
            v
        };
        let count = agents.len() as u64;
        let max_seq: u64 = {
            let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
            conn.query_row(
                "SELECT COALESCE(MAX(seq), 0) FROM events WHERE session_id = ?1",
                params![session],
                |r| r.get(0),
            )
            .unwrap_or(0)
        };
        for (i, agent) in agents.iter().enumerate() {
            let ev = Event::AgentInterrupted {
                agent: AgentId::new(agent.clone()),
            };
            self.append_event(
                session,
                &AgentId::new(agent.clone()),
                max_seq + 1 + i as u64,
                now_unix(),
                &ev,
            )?;
        }
        Ok(count)
    }

    fn accumulate_usage(&self, session: &str, input: u64, output: u64) -> Result<(), StoreError> {
        let conn = self.conn.lock().unwrap_or_else(|p| p.into_inner());
        conn.execute(
            "UPDATE sessions SET input_tokens = input_tokens + ?2,
             output_tokens = output_tokens + ?3, updated_at = ?4 WHERE id = ?1",
            params![session, input, output, now_unix()],
        )
        .map_err(|e| StoreError::Backend(e.to_string()))?;
        Ok(())
    }
}
