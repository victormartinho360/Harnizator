//! AgentRegistry + AgentManager: hierarquia de agentes, cap de
//! concorrência, inboxes e interrupção (spec/06). Puro de domínio.

use std::collections::{HashMap, VecDeque};

use crate::{AgentId, TokenUsage};

/// Ciclo de vida de um agente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentStatus {
    Queued,
    Running,
    Idle,
    Done,
    Failed,
    Interrupted,
}

/// View imutável de um nó do grafo (para UIs).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NodeView {
    pub id: AgentId,
    pub parent: Option<AgentId>,
    pub label: String,
    pub status: AgentStatus,
    pub depth: usize,
    pub usage: TokenUsage,
}

#[derive(Debug, Clone)]
struct Node {
    id: AgentId,
    parent: Option<AgentId>,
    label: String,
    prompt: String,
    status: AgentStatus,
    depth: usize,
    usage: TokenUsage,
}

#[derive(Debug, thiserror::Error, PartialEq, Eq)]
pub enum SpawnError {
    #[error("parent not found")]
    UnknownParent,
    #[error("max agent depth reached")]
    MaxDepth,
}

/// Registry de agentes com controle de concorrência (fila FIFO).
///
/// Puro e síncrono: o runtime (tokio) consulta `drain_startable()` para
/// saber o que iniciar, e notifica términos via `mark_finished`.
pub struct AgentManager {
    nodes: HashMap<AgentId, Node>,
    order: Vec<AgentId>, // ordem de criação (root primeiro)
    root: AgentId,
    max_concurrent: usize,
    max_depth: usize,
    inboxes: HashMap<AgentId, VecDeque<String>>,
    seq: u64,
}

impl AgentManager {
    pub fn new(max_concurrent: usize, max_depth: usize) -> Self {
        let root = AgentId::new("root");
        let mut nodes = HashMap::new();
        nodes.insert(
            root.clone(),
            Node {
                id: root.clone(),
                parent: None,
                label: "root".into(),
                prompt: String::new(),
                status: AgentStatus::Running,
                depth: 0,
                usage: TokenUsage::default(),
            },
        );
        Self {
            nodes,
            order: vec![root.clone()],
            root,
            max_concurrent,
            max_depth,
            inboxes: HashMap::new(),
            seq: 0,
        }
    }

    pub fn root_id(&self) -> AgentId {
        self.root.clone()
    }

    fn running_count(&self) -> usize {
        self.nodes
            .values()
            .filter(|n| n.status == AgentStatus::Running)
            .count()
    }

    /// Registra um novo agente (queued). `parent=None` ⇒ filho do root.
    pub fn spawn(
        &mut self,
        parent: Option<&AgentId>,
        label: String,
        prompt: String,
    ) -> Result<AgentId, SpawnError> {
        let parent = parent.cloned().unwrap_or_else(|| self.root.clone());
        let depth = {
            let p = self.nodes.get(&parent).ok_or(SpawnError::UnknownParent)?;
            p.depth + 1
        };
        if depth > self.max_depth {
            return Err(SpawnError::MaxDepth);
        }
        self.seq += 1;
        let id = AgentId::new(format!("agent-{}", self.seq));
        self.nodes.insert(
            id.clone(),
            Node {
                id: id.clone(),
                parent: Some(parent),
                label: label.clone(),
                prompt,
                status: AgentStatus::Queued,
                depth,
                usage: TokenUsage::default(),
            },
        );
        self.order.push(id.clone());
        Ok(id)
    }

    pub fn status_of(&self, id: &AgentId) -> Option<AgentStatus> {
        self.nodes.get(id).map(|n| n.status)
    }

    /// Promove agentes queued até o cap; retorna os ids que devem iniciar.
    pub fn drain_startable(&mut self) -> Vec<AgentId> {
        let mut started = Vec::new();
        let mut running = self.running_count();
        while running < self.max_concurrent {
            let next = self
                .order
                .iter()
                .find(|id| {
                    self.nodes
                        .get(*id)
                        .is_some_and(|n| n.status == AgentStatus::Queued)
                })
                .cloned();
            match next {
                Some(id) => {
                    if let Some(n) = self.nodes.get_mut(&id) {
                        n.status = AgentStatus::Running;
                    }
                    running += 1;
                    started.push(id);
                }
                None => break,
            }
        }
        started
    }

    pub fn set_running(&mut self, id: &AgentId) {
        if let Some(n) = self.nodes.get_mut(id) {
            n.status = AgentStatus::Running;
        }
    }

    pub fn mark_finished(&mut self, id: &AgentId) {
        if let Some(n) = self.nodes.get_mut(id) {
            n.status = AgentStatus::Done;
        }
    }

    pub fn mark_idle(&mut self, id: &AgentId) {
        if let Some(n) = self.nodes.get_mut(id) {
            if n.status == AgentStatus::Running {
                n.status = AgentStatus::Idle;
            }
        }
    }

    pub fn mark_failed(&mut self, id: &AgentId, _reason: &str) {
        if let Some(n) = self.nodes.get_mut(id) {
            n.status = AgentStatus::Failed;
        }
    }

    /// Interrompe o agente e toda a subárvore (spec/06).
    pub fn interrupt(&mut self, id: &AgentId) {
        let mut stack = vec![id.clone()];
        while let Some(cur) = stack.pop() {
            if let Some(n) = self.nodes.get_mut(&cur) {
                if !matches!(n.status, AgentStatus::Done) {
                    n.status = AgentStatus::Interrupted;
                }
            }
            let children: Vec<AgentId> = self
                .nodes
                .values()
                .filter(|n| n.parent.as_ref() == Some(&cur))
                .map(|n| n.id.clone())
                .collect();
            stack.extend(children);
        }
    }

    /// Re-enfileira um agente falho para nova tentativa.
    pub fn retry(&mut self, id: &AgentId) -> Result<(), SpawnError> {
        match self.nodes.get_mut(id) {
            Some(n) if n.status == AgentStatus::Failed => {
                n.status = AgentStatus::Queued;
                Ok(())
            }
            Some(_) => Err(SpawnError::UnknownParent),
            None => Err(SpawnError::UnknownParent),
        }
    }

    /// Envia texto para a inbox de um agente (context injection).
    pub fn inbox_send(&mut self, id: &AgentId, text: String) -> bool {
        match self.inboxes.get_mut(id) {
            Some(q) => {
                q.push_back(text);
                true
            }
            None if self.nodes.contains_key(id) => {
                let mut q = VecDeque::new();
                q.push_back(text);
                self.inboxes.insert(id.clone(), q);
                true
            }
            None => false,
        }
    }

    /// Drena a inbox do agente (consumida no início do próximo turno).
    pub fn inbox_take(&mut self, id: &AgentId) -> Vec<String> {
        self.inboxes
            .get_mut(id)
            .map(|q| q.drain(..).collect())
            .unwrap_or_default()
    }

    /// Prompt inicial do agente (para o runtime iniciar a task).
    pub fn prompt_of(&self, id: &AgentId) -> Option<String> {
        self.nodes.get(id).map(|n| n.prompt.clone())
    }

    /// Snapshot do grafo para renderização (root primeiro).
    pub fn snapshot(&self) -> Vec<NodeView> {
        self.order
            .iter()
            .filter_map(|id| self.nodes.get(id))
            .map(|n| NodeView {
                id: n.id.clone(),
                parent: n.parent.clone(),
                label: n.label.clone(),
                status: n.status,
                depth: n.depth,
                usage: n.usage,
            })
            .collect()
    }
}
