//! Runtime do TUI: loop de eventos crossterm + tasks de agentes (spec/05, 06).
//!
//! Toda funcionalidade de negócio vive no core; aqui há apenas wiring de IO.

use std::collections::HashMap;
use std::io::stdout;
use std::sync::{Arc, Mutex};

use crossterm::event::{Event as CrosstermEvent, EventStream, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures::StreamExt;
use futures::channel::mpsc as fmpsc;
use futures::future::{AbortHandle, Abortable};
use harness_core::agent_loop::AgentLoop;
use harness_core::agents::AgentManager;
use harness_core::events::Event as CoreEvent;
use harness_core::provider_port::{ChatRequest, LlmProvider};
use harness_core::service::{ServiceCtx, run_agent_service};
use harness_core::subagent_tool::SpawnToolPort;
use harness_core::tool_port::{ApprovalDecision, ApprovalPort, ToolCall, ToolDecision, ToolPort};
use harness_core::{AgentId, ContentBlock, Message, Role};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::{mpsc, oneshot};

use crate::state::{Action, AppState, ChatMessage, DisplayRole};
use crate::ui_msg::UiMsg;
use harness_core::replay::replay_agent;

#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("terminal io: {0}")]
    Io(#[from] std::io::Error),
}

/// ToolPort vazio para sessões sem tools (`specs()` vazio, tudo negado).
struct NoTools;

#[async_trait::async_trait]
impl ToolPort for NoTools {
    fn decide(&self, _call: &ToolCall) -> ToolDecision {
        ToolDecision::Deny {
            reason: "no tools enabled".into(),
        }
    }
    async fn execute(&self, call: &ToolCall) -> harness_core::tool_port::ToolOutcome {
        harness_core::tool_port::ToolOutcome {
            content: format!("error: no tools enabled ({})", call.name),
            is_error: true,
        }
    }
}

/// Approver do TUI: empurra NeedsApproval para a UI e espera a resposta.
pub struct TuiApproval {
    tx: mpsc::UnboundedSender<UiMsg>,
    reply_slot: Arc<Mutex<Option<oneshot::Sender<ApprovalDecision>>>>,
}

#[async_trait::async_trait]
impl ApprovalPort for TuiApproval {
    async fn decide(&self, call: &ToolCall, reason: &str) -> ApprovalDecision {
        let (tx, rx) = oneshot::channel();
        {
            let mut slot = match self.reply_slot.lock() {
                Ok(s) => s,
                Err(poisoned) => poisoned.into_inner(),
            };
            *slot = Some(tx);
        }
        let _ = self.tx.send(UiMsg::NeedsApproval {
            call: call.clone(),
            reason: reason.to_string(),
        });
        rx.await.unwrap_or(ApprovalDecision::Deny {
            reason: "approval channel closed".into(),
        })
    }
}

/// Guard que restaura o terminal ao sair.
struct TerminalGuard;

impl TerminalGuard {
    fn enter() -> Result<Self, TuiError> {
        enable_raw_mode()?;
        execute!(stdout(), EnterAlternateScreen)?;
        Ok(Self)
    }
}

impl Drop for TerminalGuard {
    fn drop(&mut self) {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
    }
}

/// Roda o TUI interativo até o usuário sair (Ctrl+C).
pub async fn run(
    provider: Arc<dyn LlmProvider>,
    tools: Option<Arc<dyn ToolPort>>,
    model: String,
    sandbox: String,
    store: Option<Arc<dyn harness_core::store_port::SessionStore>>,
    admin: Option<Arc<dyn harness_core::provider_admin::ProviderAdmin>>,
) -> Result<(), TuiError> {
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new(&model, &sandbox);
    let (tx, mut rx) = mpsc::unbounded_channel::<UiMsg>();
    let reply_slot: Arc<Mutex<Option<oneshot::Sender<ApprovalDecision>>>> =
        Arc::new(Mutex::new(None));

    let manager = Arc::new(Mutex::new(AgentManager::new(8, 4)));
    let root_id = {
        let m = manager.lock().unwrap_or_else(|p| p.into_inner());
        m.root_id()
    };

    // tools do root ganham spawn_agent via wrapper (spec/06)
    let base_tools: Arc<dyn ToolPort> = tools.unwrap_or_else(|| Arc::new(NoTools));
    let root_tools: Arc<dyn ToolPort> = Arc::new(SpawnToolPort::new(
        base_tools.clone(),
        manager.clone(),
        root_id.clone(),
    ));

    let mut history: Vec<Message> = Vec::new();
    let mut root_task: Option<tokio::task::JoinHandle<()>> = None;
    let mut agent_tasks: HashMap<AgentId, AbortHandle> = HashMap::new();
    let mut agent_inboxes: HashMap<AgentId, fmpsc::UnboundedSender<String>> = HashMap::new();
    let mut keys = EventStream::new();
    let mut graph_dirty = true;

    // persiste sessão se houver store (spec/07)
    let session_id = store.as_ref().map(|s| {
        s.create_session("tui session", &model, &sandbox)
            .unwrap_or_else(|_| "unknown".to_string())
    });
    let seq_counters: Arc<Mutex<HashMap<String, u64>>> = Arc::new(Mutex::new(HashMap::new()));

    loop {
        // sincroniza grafo e promove agentes enfileirados
        if graph_dirty {
            let snapshot = {
                let m = manager.lock().unwrap_or_else(|p| p.into_inner());
                m.snapshot()
            };
            state.apply_ui_msg(UiMsg::GraphSync(snapshot));
            graph_dirty = false;
        }
        let startable = {
            let mut m = manager.lock().unwrap_or_else(|p| p.into_inner());
            m.drain_startable()
        };
        for agent_id in startable {
            if agent_id == root_id {
                continue; // root roda via chat
            }
            let prompt = {
                let m = manager.lock().unwrap_or_else(|p| p.into_inner());
                m.prompt_of(&agent_id).unwrap_or_default()
            };
            let (inbox_tx, mut inbox_rx) = fmpsc::unbounded::<String>();
            agent_inboxes.insert(agent_id.clone(), inbox_tx);

            let tx2 = tx.clone();
            let slot2 = reply_slot.clone();
            let provider2 = provider.clone();
            let manager2 = manager.clone();
            let base2 = base_tools.clone();
            let model2 = model.clone();
            let agent2 = agent_id.clone();

            let (abort, abort_reg) = AbortHandle::new_pair();
            agent_tasks.insert(agent_id.clone(), abort);

            tokio::spawn(Abortable::new(
                async move {
                    let sub_tools: Arc<dyn ToolPort> =
                        Arc::new(SpawnToolPort::new(base2, manager2.clone(), agent2.clone()));
                    let sink_tx = tx2.clone();
                    let mut sink = move |ev: &CoreEvent| {
                        let _ = sink_tx.send(UiMsg::Core(ev.clone()));
                    };
                    let approval = TuiApproval {
                        tx: tx2.clone(),
                        reply_slot: slot2,
                    };
                    let history = vec![Message {
                        role: Role::User,
                        content: vec![ContentBlock::Text { text: prompt }],
                    }];
                    let tx_done = tx2.clone();
                    let id_done = agent2.clone();
                    let result = run_agent_service(
                        ServiceCtx {
                            provider: provider2.as_ref(),
                            tools: sub_tools.as_ref(),
                            approver: &approval,
                            manager: manager2,
                            agent: agent2,
                            model: model2,
                        },
                        history,
                        &mut inbox_rx,
                        &mut sink,
                    )
                    .await;
                    if let Err(e) = result {
                        let _ = tx_done.send(UiMsg::Core(CoreEvent::Error {
                            message: format!("agent {id_done}: {e}"),
                        }));
                    }
                },
                abort_reg,
            ));
            let _ = tx.send(UiMsg::Core(CoreEvent::AgentSpawned {
                agent: agent_id.clone(),
                parent: None,
                label: String::new(),
            }));
        }

        terminal.draw(|f| crate::ui::draw(f, &state))?;

        tokio::select! {
            maybe_msg = rx.recv() => {
                match maybe_msg {
                    Some(UiMsg::SyncHistory(messages, usage)) => {
                        history = messages;
                        state.apply_ui_msg(UiMsg::SyncHistory(Vec::new(), usage));
                    }
                    Some(msg) => state.apply_ui_msg(msg),
                    None => {}
                }
                graph_dirty = true;
            }
            maybe_key = keys.next() => {
                let Some(Ok(CrosstermEvent::Key(key))) = maybe_key else { continue };
                if key.kind != KeyEventKind::Press { continue; }
                match state.handle_key(key) {
                    Action::None => {}
                    Action::Quit => break,
                    Action::ResumeSession(id) => {
                        if let Some(st) = &store {
                            if let Ok(events) = st.events(&id) {
                                let msgs = replay_agent(&events, &root_id);
                                history = msgs.clone();
                                state.messages = msgs
                                    .iter()
                                    .map(|m| ChatMessage {
                                        role: match m.role {
                                            Role::User => DisplayRole::User,
                                            Role::Assistant => DisplayRole::Assistant,
                                            Role::System => DisplayRole::System,
                                        },
                                        content: m.text(),
                                        streaming: false,
                                    })
                                    .collect();
                            }
                        }
                    }
                    Action::Cancel => {
                        if let Some(h) = root_task.take() {
                            h.abort();
                        }
                        state.apply_ui_msg(UiMsg::TurnFinished(Ok(())));
                    }
                    Action::Approve(decision) => {
                        let sender = {
                            let mut slot = match reply_slot.lock() {
                                Ok(s) => s,
                                Err(p) => p.into_inner(),
                            };
                            slot.take()
                        };
                        if let Some(tx_reply) = sender {
                            let _ = tx_reply.send(decision);
                        }
                        state.apply_ui_msg(UiMsg::ApprovalResolved);
                    }
                    Action::Interrupt(id) => {
                        if let Some(t) = agent_tasks.remove(&id) {
                            t.abort();
                        }
                        agent_inboxes.remove(&id);
                        if let Ok(mut m) = manager.lock() {
                            m.interrupt(&id);
                        }
                        let _ = tx.send(UiMsg::Core(CoreEvent::AgentInterrupted {
                            agent: id,
                        }));
                        graph_dirty = true;
                    }
                    Action::SendToAgent(id, text) | Action::InjectContext(id, text) => {
                        if let Some(inbox) = agent_inboxes.get(&id) {
                            let _ = inbox.unbounded_send(text);
                            if let Ok(mut m) = manager.lock() {
                                m.set_running(&id);
                            }
                            graph_dirty = true;
                        }
                    }
                    Action::SetProviderKey(id, key) => {
                        if let Some(a) = &admin {
                            let msg = match a.set_key(&id, &key) {
                                Ok(()) => format!("{id}: chave salva"),
                                Err(e) => format!("{id}: {e}"),
                            };
                            state.providers_status = msg;
                            let list = a.views();
                            let _ = tx.send(UiMsg::ProvidersSync(list));
                        }
                    }
                    Action::AddProvider {
                        name,
                        kind,
                        base_url,
                    } => {
                        if let Some(a) = &admin {
                            let msg = match a.add_provider(&name, &kind, &base_url) {
                                Ok(()) => format!("{name}: adicionado"),
                                Err(e) => e,
                            };
                            state.providers_status = msg;
                            let list = a.views();
                            let _ = tx.send(UiMsg::ProvidersSync(list));
                        }
                    }
                    Action::RemoveProvider(id) => {
                        if let Some(a) = &admin {
                            let msg = match a.remove_provider(&id) {
                                Ok(()) => format!("{id}: removido"),
                                Err(e) => e,
                            };
                            state.providers_status = msg;
                            let list = a.views();
                            let _ = tx.send(UiMsg::ProvidersSync(list));
                        }
                    }
                    Action::TestConnection(id) => {
                        if let Some(a) = &admin {
                            let a2 = a.clone();
                            let tx2 = tx.clone();
                            let id2 = id.clone();
                            tokio::spawn(async move {
                                let result = a2.test_connection(&id2).await;
                                let _ = tx2.send(UiMsg::ProviderTestResult {
                                    id: id2,
                                    result,
                                });
                            });
                        }
                    }
                    Action::Send(text) => {
                        state.push_user_message(text.clone());
                        history.push(Message {
                            role: Role::User,
                            content: vec![ContentBlock::Text { text }],
                        });

                        let tx2 = tx.clone();
                        let slot2 = reply_slot.clone();
                        let provider2 = provider.clone();
                        let tools2 = root_tools.clone();
                        let history_now = history.clone();
                        let model_name = model.clone();
                        let store3 = store.clone();
                        let session3 = session_id.clone();
                        let seq_counters3 = seq_counters.clone();
                        root_task = Some(tokio::spawn(async move {
                            let sink_tx = tx2.clone();
                            let mut sink = move |ev: &CoreEvent| {
                                if let (Some(st), Some(sess)) = (&store3, &session3) {
                                    let agent = ev.agent().cloned()
                                        .unwrap_or_else(|| AgentId::new("root"));
                                    let mut counters = match seq_counters3.lock() {
                                        Ok(c) => c,
                                        Err(p) => p.into_inner(),
                                    };
                                    let seq = counters.entry(agent.as_str().to_string()).or_insert(0);
                                    *seq += 1;
                                    let ts = std::time::SystemTime::now()
                                        .duration_since(std::time::UNIX_EPOCH)
                                        .map(|d| d.as_secs())
                                        .unwrap_or_default();
                                    let _ = st.append_event(sess, &agent, *seq, ts, ev);
                                }
                                let _ = sink_tx.send(UiMsg::Core(ev.clone()));
                            };
                            let approval = TuiApproval {
                                tx: tx2.clone(),
                                reply_slot: slot2,
                            };
                            let req = ChatRequest {
                                model: model_name,
                                messages: history_now,
                                max_tokens: 4096,
                                system: None,
                                tools: vec![],
                            };
                            let looper = AgentLoop::new(provider2.as_ref(), tools2.as_ref(), &approval);
                            match looper.run_with_sink(req, &mut sink).await {
                                Ok(run) => {
                                    let _ = tx2.send(UiMsg::SyncHistory(
                                        run.messages,
                                        run.outcome.usage,
                                    ));
                                    let _ = tx2.send(UiMsg::TurnFinished(Ok(())));
                                }
                                Err(e) => {
                                    let _ = tx2.send(UiMsg::TurnFinished(Err(e.to_string())));
                                }
                            }
                        }));
                    }
                }
            }
        }

        // sync de telas dedicadas conforme foco
        match state.screen {
            crate::state::Screen::Sessions => {
                if let Some(st) = &store {
                    if let Ok(list) = st.sessions() {
                        state.apply_ui_msg(UiMsg::SessionsSync(list));
                    }
                }
            }
            crate::state::Screen::Providers => {
                if let Some(a) = &admin {
                    state.apply_ui_msg(UiMsg::ProvidersSync(a.views()));
                }
            }
            _ => {}
        }
    }
    Ok(())
}
