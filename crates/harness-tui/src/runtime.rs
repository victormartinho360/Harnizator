//! Runtime do TUI: loop de eventos crossterm + tasks de agente (spec/05).
//!
//! Toda funcionalidade de negócio vive no core; aqui há apenas wiring de IO.

use std::io::stdout;
use std::sync::{Arc, Mutex};

use crossterm::event::{Event as CrosstermEvent, EventStream, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode,
};
use futures::StreamExt;
use harness_core::agent_loop::AgentLoop;
use harness_core::events::Event as CoreEvent;
use harness_core::provider_port::{ChatRequest, LlmProvider};
use harness_core::tool_port::{ApprovalDecision, ApprovalPort, ToolCall, ToolPort};
use harness_core::{Message, Role};
use ratatui::Terminal;
use ratatui::backend::CrosstermBackend;
use tokio::sync::{mpsc, oneshot};
use tokio::task::JoinHandle;

use crate::state::{Action, AppState};
use crate::ui_msg::UiMsg;

#[derive(Debug, thiserror::Error)]
pub enum TuiError {
    #[error("terminal io: {0}")]
    Io(#[from] std::io::Error),
}

/// ToolPort vazio para sessões sem tools (`specs()` vazio, tudo negado).
struct NoTools;

#[async_trait::async_trait]
impl ToolPort for NoTools {
    fn decide(&self, _call: &ToolCall) -> harness_core::tool_port::ToolDecision {
        harness_core::tool_port::ToolDecision::Deny {
            reason: "no tools enabled".into(),
        }
    }
    async fn execute(&self, call: &ToolCall) -> harness_core::tool_port::ToolOutcome {
        harness_core::tool_port::ToolOutcome {
            content: format!("error: no tools enabled ({} )", call.name),
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
) -> Result<(), TuiError> {
    let _guard = TerminalGuard::enter()?;
    let backend = CrosstermBackend::new(stdout());
    let mut terminal = Terminal::new(backend)?;

    let mut state = AppState::new(&model, &sandbox);
    let (tx, mut rx) = mpsc::unbounded_channel::<UiMsg>();
    let reply_slot: Arc<Mutex<Option<oneshot::Sender<ApprovalDecision>>>> =
        Arc::new(Mutex::new(None));

    let tools: Arc<dyn ToolPort> = tools.unwrap_or_else(|| Arc::new(NoTools));
    let model_name = model.clone();
    let mut history: Vec<Message> = Vec::new();
    let mut task: Option<JoinHandle<()>> = None;
    let mut keys = EventStream::new();

    loop {
        terminal.draw(|f| crate::ui::draw(f, &state))?;

        tokio::select! {
            maybe_msg = rx.recv() => {
                match maybe_msg {
                    Some(UiMsg::SyncHistory(messages, usage)) => {
                        history = messages;
                        state.apply_ui_msg(UiMsg::SyncHistory(Vec::new(), usage));
                    }
                    Some(UiMsg::TurnFinished(Ok(()))) => {
                        state.apply_ui_msg(UiMsg::TurnFinished(Ok(())));
                    }
                    Some(msg @ UiMsg::TurnFinished(Err(_))) => state.apply_ui_msg(msg),
                    Some(msg) => state.apply_ui_msg(msg),
                    None => {}
                }
            }
            maybe_key = keys.next() => {
                let Some(Ok(CrosstermEvent::Key(key))) = maybe_key else { continue };
                if key.kind != KeyEventKind::Press { continue; }
                match state.handle_key(key) {
                    Action::None => {}
                    Action::Quit => break,
                    Action::Cancel => {
                        if let Some(h) = task.take() {
                            h.abort();
                        }
                        state.apply_ui_msg(UiMsg::TurnFinished(Ok(())));
                    }
                    Action::Approve(decision) => {
                        let sender = {
                            let mut slot = match reply_slot.lock() {
                                Ok(s) => s,
                                Err(poisoned) => poisoned.into_inner(),
                            };
                            slot.take()
                        };
                        if let Some(tx_reply) = sender {
                            let _ = tx_reply.send(decision);
                        }
                        state.apply_ui_msg(UiMsg::ApprovalResolved);
                    }
                    Action::Send(text) => {
                        state.push_user_message(text.clone());
                        history.push(Message { role: Role::User, content: vec![harness_core::ContentBlock::Text { text }] });

                        let tx2 = tx.clone();
                        let slot2 = reply_slot.clone();
                        let model_name = model_name.clone();
                        let provider2 = provider.clone();
                        let tools2 = tools.clone();
                        let history_now = history.clone();
                        task = Some(tokio::spawn(async move {
                            let sink_tx = tx2.clone();
                            let mut sink = move |ev: &CoreEvent| {
                                let _ = sink_tx.send(UiMsg::Core(ev.clone()));
                            };
                            let approval = TuiApproval { tx: tx2.clone(), reply_slot: slot2 };
                            let req = ChatRequest {
                                model: model_name.clone(),
                                messages: history_now,
                                max_tokens: 4096,
                                system: None,
                                tools: vec![],
                            };
                            let looper = AgentLoop::new(provider2.as_ref(), tools2.as_ref(), &approval);
                            match looper.run_with_sink(req, &mut sink).await {
                                Ok(run) => {
                                    let _ = tx2.send(UiMsg::SyncHistory(run.messages, run.outcome.usage));
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
    }
    Ok(())
}
