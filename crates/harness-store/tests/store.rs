//! Red tests: harness-store SQLite (spec/07).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harness_core::events::Event;
use harness_core::replay::replay_agent;
use harness_core::store_port::SessionStore;
use harness_core::{AgentId, Role};
use harness_store::SqliteStore;

fn store() -> (tempfile::TempDir, SqliteStore) {
    let dir = tempfile::tempdir().unwrap();
    let s = SqliteStore::open(&dir.path().join("harness.db")).unwrap();
    (dir, s)
}

#[test]
fn migrations_are_idempotent() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("harness.db");
    let _ = SqliteStore::open(&db).unwrap();
    let _ = SqliteStore::open(&db).unwrap(); // segunda abertura: sem erro
    let s = SqliteStore::open(&db).unwrap();
    assert_eq!(s.schema_version().unwrap(), 1);
}

#[test]
fn create_and_list_sessions() {
    let (_d, s) = store();
    let a = s
        .create_session("first chat", "anthropic/x", "only-flagged")
        .unwrap();
    let b = s
        .create_session("second chat", "openai/y", "full-access")
        .unwrap();
    let sessions = s.sessions().unwrap();
    let mut titles: Vec<_> = sessions.iter().map(|m| m.title.clone()).collect();
    titles.sort();
    assert_eq!(titles, vec!["first chat", "second chat"]);
    assert_ne!(a, b);
}

#[test]
fn append_and_replay_roundtrip_with_tools() {
    let (_d, s) = store();
    let sid = s.create_session("t", "m", "only-flagged").unwrap();
    let root = AgentId::new("root");
    let events = [
        Event::AssistantDelta {
            agent: root.clone(),
            text: "Let me read. ".into(),
        },
        Event::ToolCallRequested {
            agent: root.clone(),
            id: "c1".into(),
            name: "read_file".into(),
            args: serde_json::json!({"path": "a.txt"}),
        },
        Event::ToolCallApproved {
            agent: root.clone(),
            id: "c1".into(),
        },
        Event::ToolCallCompleted {
            agent: root.clone(),
            id: "c1".into(),
            is_error: false,
            output: "file contents".into(),
        },
        Event::AssistantDelta {
            agent: root.clone(),
            text: "It says: file contents".into(),
        },
    ];
    for (i, e) in events.iter().enumerate() {
        s.append_event(&sid, &root, i as u64, 100 + i as u64, e)
            .unwrap();
    }

    let stored = s.events(&sid).unwrap();
    assert_eq!(stored.len(), 5);
    let msgs = replay_agent(&stored, &root);
    assert_eq!(msgs.len(), 3); // assistant(text+tool_use), user(tool_result), assistant(text)
    assert_eq!(msgs[2].role, Role::Assistant);
    assert_eq!(msgs[2].text(), "It says: file contents");
    match &msgs[1].content[0] {
        harness_core::ContentBlock::ToolResult {
            content, is_error, ..
        } => {
            assert_eq!(content, "file contents");
            assert!(!is_error);
        }
        other => panic!("expected tool_result, got {other:?}"),
    }
}

#[test]
fn crash_recovery_marks_running_agents_interrupted() {
    let dir = tempfile::tempdir().unwrap();
    let db = dir.path().join("harness.db");
    let root = AgentId::new("root");
    let sid;
    {
        let s = SqliteStore::open(&db).unwrap();
        sid = s.create_session("crash", "m", "only-flagged").unwrap();
        s.append_event(
            &sid,
            &root,
            0,
            1,
            &Event::AgentSpawned {
                agent: root.clone(),
                parent: None,
                label: "root".into(),
            },
        )
        .unwrap();
        // sem close limpo: simula crash largando a conexão sem finalizar agente
    }
    let s = SqliteStore::open(&db).unwrap();
    let recovered = s.recover(&sid).unwrap();
    assert!(recovered >= 1, "pelo menos o agente (root) é marcado");
    let evs = s.events(&sid).unwrap();
    assert!(
        evs.iter()
            .any(|e| matches!(e.event, Event::AgentInterrupted { .. }))
    );
}

#[test]
fn burst_of_events_is_not_lost() {
    let (_d, s) = store();
    let sid = s.create_session("burst", "m", "only-flagged").unwrap();
    let root = AgentId::new("root");
    for i in 0..10_000u64 {
        s.append_event(
            &sid,
            &root,
            i,
            i,
            &Event::AssistantDelta {
                agent: root.clone(),
                text: "x".into(),
            },
        )
        .unwrap();
    }
    let evs = s.events(&sid).unwrap();
    assert_eq!(evs.len(), 10_000);
    assert_eq!(evs.last().unwrap().seq, 9_999);
}

#[test]
fn usage_accumulates() {
    let (_d, s) = store();
    let sid = s.create_session("t", "m", "m").unwrap();
    s.accumulate_usage(&sid, 10, 3).unwrap();
    s.accumulate_usage(&sid, 5, 2).unwrap();
    let meta = s.sessions().unwrap().into_iter().next().unwrap();
    assert_eq!(meta.usage.input, 15);
    assert_eq!(meta.usage.output, 5);
}
