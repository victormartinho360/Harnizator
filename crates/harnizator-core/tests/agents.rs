//! Red tests: AgentRegistry — hierarquia, cap de concorrência (spec/06).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harnizator_core::agents::{AgentManager, AgentStatus, SpawnError};
use proptest::prelude::*;

fn mgr(max_concurrent: usize, max_depth: usize) -> AgentManager {
    AgentManager::new(max_concurrent, max_depth)
}

#[test]
fn root_agent_exists_and_is_running() {
    let m = mgr(8, 4);
    let nodes = m.snapshot();
    assert_eq!(nodes.len(), 1);
    assert_eq!(nodes[0].parent, None);
    assert_eq!(nodes[0].status, AgentStatus::Running);
}

#[test]
fn spawn_creates_child_with_parent_link() {
    let mut m = mgr(8, 4);
    let child = m.spawn(None, "researcher".into(), "task".into()).unwrap();
    let nodes = m.snapshot();
    assert_eq!(nodes.len(), 2);
    let c = nodes.iter().find(|n| n.id == child).unwrap();
    let root = &nodes[0].id;
    assert_eq!(c.parent.as_ref(), Some(root));
    assert_eq!(c.depth, 1);
}

#[test]
fn cap_enqueued_when_full_then_promoted_on_finish() {
    let mut m = mgr(1, 4);
    // root já ocupa o único slot
    let a = m.spawn(None, "a".into(), "t".into()).unwrap();
    let b = m.spawn(None, "b".into(), "t".into()).unwrap();
    assert_eq!(m.status_of(&a), Some(AgentStatus::Queued));
    assert_eq!(m.status_of(&b), Some(AgentStatus::Queued));
    assert_eq!(m.drain_startable(), vec![]);
    // root termina → libera slot → primeiro da fila promove (FIFO)
    m.mark_finished(&m.root_id());
    assert_eq!(m.drain_startable(), vec![a.clone()]);
    assert_eq!(m.status_of(&a), Some(AgentStatus::Running));
    assert_eq!(m.drain_startable(), vec![]);
    m.mark_finished(&a);
    assert_eq!(m.drain_startable(), vec![b.clone()]);
}

#[test]
fn depth_limit_rejects_spawn() {
    let mut m = mgr(8, 1); // root (0) + filhos (1) no máximo
    let a = m.spawn(None, "a".into(), "t".into()).unwrap();
    let err = m.spawn(Some(&a), "grandchild".into(), "t".into());
    assert!(matches!(err, Err(SpawnError::MaxDepth)));
}

#[test]
fn interrupt_marks_status_and_cascades_to_children() {
    let mut m = mgr(8, 4);
    let a = m.spawn(None, "a".into(), "t".into()).unwrap();
    let b = m.spawn(Some(&a), "b".into(), "t".into()).unwrap();
    m.interrupt(&a);
    assert_eq!(m.status_of(&a), Some(AgentStatus::Interrupted));
    assert_eq!(m.status_of(&b), Some(AgentStatus::Interrupted));
}

#[test]
fn retry_failed_marks_queued() {
    let mut m = mgr(8, 4);
    let a = m.spawn(None, "a".into(), "t".into()).unwrap();
    m.mark_failed(&a, "boom");
    assert_eq!(m.status_of(&a), Some(AgentStatus::Failed));
    m.retry(&a).unwrap();
    assert_eq!(m.status_of(&a), Some(AgentStatus::Queued));
}

#[test]
fn inject_context_only_for_known_agent() {
    let mut m = mgr(8, 4);
    let a = m.spawn(None, "a".into(), "t".into()).unwrap();
    assert!(m.inbox_send(&a, "contexto".into()));
    assert!(!m.inbox_send(&harnizator_core::AgentId::new("nope"), "x".into()));
    assert_eq!(m.inbox_take(&a), vec!["contexto".to_string()]);
}

#[test]
fn snapshot_is_tree() {
    let mut m = mgr(8, 8);
    let a = m.spawn(None, "a".into(), "t".into()).unwrap();
    let b = m.spawn(Some(&a), "b".into(), "t".into()).unwrap();
    let c = m.spawn(Some(&a), "c".into(), "t".into()).unwrap();
    let nodes = m.snapshot();
    assert_eq!(nodes.len(), 4);
    let by_id: std::collections::HashMap<_, _> = nodes.iter().map(|n| (&n.id, n)).collect();
    assert_eq!(by_id[&b].depth, 2);
    assert_eq!(by_id[&c].depth, 2);
    assert_eq!(by_id[&b].parent.as_ref(), Some(&a));
}

proptest! {
    /// Sequências aleatórias de spawn/finish/interrupt mantêm invariantes
    /// da árvore (spec/06): depth ≤ max_depth, parent sempre válido.
    #[test]
    fn registry_invariants_hold(
        ops in proptest::collection::vec((0u8..4, 0usize..8), 1..40),
    ) {
        let mut m = mgr(4, 3);
        let root = m.root_id();
        for (op, sel) in ops {
            let nodes = m.snapshot();
            match op {
                0 => {
                    let parent = &nodes[sel % nodes.len()].id;
                    let _ = m.spawn(Some(parent), "x".into(), "t".into());
                }
                1 => {
                    if let Some(n) = nodes.get(sel % nodes.len()) {
                        m.mark_finished(&n.id);
                    }
                }
                2 => {
                    if let Some(n) = nodes.get(sel % nodes.len()) {
                        m.interrupt(&n.id);
                    }
                }
                _ => {
                    let _ = m.drain_startable();
                }
            }
            // invariantes após cada op
            let nodes = m.snapshot();
            let by_id: std::collections::HashMap<_, _> =
                nodes.iter().map(|n| (n.id.clone(), n)).collect();
            assert!(by_id.contains_key(&root), "root sempre existe");
            for n in &nodes {
                prop_assert!(n.depth <= 3, "depth {} excede limite", n.depth);
                if let Some(p) = &n.parent {
                    prop_assert!(by_id.contains_key(p), "parent ausente");
                    prop_assert_eq!(n.depth, by_id[p].depth + 1);
                }
            }
        }
    }
}
