//! Agent loop: turn → tool calls → approval → tool results → próximo turn
//! (spec/04). Puro de domínio: provider/tools/aprovação são ports injetadas.

use futures::StreamExt;

use crate::events::Event;
use crate::message::{ContentBlock, Message};
use crate::provider_port::{ChatRequest, LlmProvider, ProviderError, StreamChunk};
use crate::tool_port::{ApprovalPort, ToolCall, ToolDecision, ToolOutcome, ToolPort};
use crate::{AgentId, TokenUsage};

/// Erros do agent loop.
#[derive(Debug, thiserror::Error)]
pub enum LoopError {
    #[error("provider error: {0}")]
    Provider(#[from] ProviderError),
    #[error("max turns ({0}) reached without final reply")]
    MaxTurns(usize),
}

/// Resultado final do loop.
#[derive(Debug, Default, Clone)]
pub struct LoopOutcome {
    pub text: String,
    pub usage: TokenUsage,
}

/// Retorno completo de `AgentLoop::run`.
#[derive(Debug)]
pub struct LoopRun {
    pub outcome: LoopOutcome,
    /// Histórico estendido (inclui tool_use/tool_result).
    pub messages: Vec<Message>,
    /// Trilha de auditoria do loop (spec/04).
    pub events: Vec<Event>,
}

/// Loop de um agente com tools e aprovação injetada.
pub struct AgentLoop<'a> {
    provider: &'a dyn LlmProvider,
    tools: &'a dyn ToolPort,
    approver: &'a dyn ApprovalPort,
    agent: AgentId,
    max_turns: usize,
}

impl<'a> AgentLoop<'a> {
    pub fn new(
        provider: &'a dyn LlmProvider,
        tools: &'a dyn ToolPort,
        approver: &'a dyn ApprovalPort,
    ) -> Self {
        Self {
            provider,
            tools,
            approver,
            agent: AgentId::new("root"),
            max_turns: 8,
        }
    }

    pub fn with_max_turns(mut self, max_turns: usize) -> Self {
        self.max_turns = max_turns;
        self
    }

    pub fn with_agent_id(mut self, agent: AgentId) -> Self {
        self.agent = agent;
        self
    }

    /// Executa o loop até resposta final (ou MaxTurns).
    pub async fn run(&self, req: ChatRequest) -> Result<LoopRun, LoopError> {
        self.run_with_sink(req, &mut |_| {}).await
    }

    /// Como `run`, mas cada evento também é entregue a `sink` em tempo real
    /// (mesmo quando o loop retorna Err no meio do caminho).
    pub async fn run_with_sink(
        &self,
        req: ChatRequest,
        sink: &mut dyn FnMut(&Event),
    ) -> Result<LoopRun, LoopError> {
        let mut messages = req.messages.clone();
        let mut events: Vec<Event> = Vec::new();
        macro_rules! emit {
            ($e:expr) => {{
                let e = $e;
                sink(&e);
                events.push(e);
            }};
        }
        let mut outcome = LoopOutcome::default();

        for _turn in 0..self.max_turns {
            let request = ChatRequest {
                model: req.model.clone(),
                messages: messages.clone(),
                max_tokens: req.max_tokens,
                system: req.system.clone(),
                tools: self.tools.specs(),
            };

            let stream = self.provider.stream(request).await?;
            let mut text = String::new();
            let mut tool_calls: Vec<ToolCall> = Vec::new();
            let mut stream = stream;
            while let Some(chunk) = stream.next().await {
                match chunk? {
                    StreamChunk::TextDelta(t) => {
                        text.push_str(&t);
                        emit!(Event::AssistantDelta {
                            agent: self.agent.clone(),
                            text: t,
                        });
                    }
                    StreamChunk::ToolUse(call) => tool_calls.push(call),
                    StreamChunk::Usage { input, output } => {
                        outcome.usage = TokenUsage { input, output };
                    }
                    StreamChunk::MessageStart | StreamChunk::MessageStop => {}
                }
            }

            if tool_calls.is_empty() {
                outcome.text = text;
                return Ok(LoopRun {
                    outcome,
                    messages,
                    events,
                });
            }

            // mensagem do assistente com os blocos tool_use
            let mut assistant_blocks = Vec::new();
            if !text.is_empty() {
                assistant_blocks.push(ContentBlock::Text { text });
            }
            for call in &tool_calls {
                assistant_blocks.push(ContentBlock::ToolUse {
                    id: call.id.clone(),
                    name: call.name.clone(),
                    input: call.args.clone(),
                });
            }
            messages.push(Message {
                role: crate::Role::Assistant,
                content: assistant_blocks,
            });

            // decide + executa cada chamada, gerando tool_results
            let mut results = Vec::new();
            for call in &tool_calls {
                emit!(Event::ToolCallRequested {
                    agent: self.agent.clone(),
                    id: call.id.clone(),
                    name: call.name.clone(),
                    args: call.args.clone(),
                });
                let outcome = match self.tools.decide(call) {
                    ToolDecision::Deny { reason } => {
                        emit!(Event::ToolCallDenied {
                            agent: self.agent.clone(),
                            id: call.id.clone(),
                            reason: reason.clone(),
                        });
                        ToolOutcome {
                            content: format!("tool call denied by sandbox policy: {reason}"),
                            is_error: true,
                        }
                    }
                    ToolDecision::NeedsApproval { reason } => {
                        match self.approver.decide(call, &reason).await {
                            crate::tool_port::ApprovalDecision::Approve
                            | crate::tool_port::ApprovalDecision::ApproveAndAllowlist => {
                                emit!(Event::ToolCallApproved {
                                    agent: self.agent.clone(),
                                    id: call.id.clone(),
                                });
                                let outcome = self.tools.execute(call).await;
                                emit!(Event::ToolCallCompleted {
                                    agent: self.agent.clone(),
                                    id: call.id.clone(),
                                    is_error: outcome.is_error,
                                });
                                outcome
                            }
                            crate::tool_port::ApprovalDecision::Deny { reason } => {
                                emit!(Event::ToolCallDenied {
                                    agent: self.agent.clone(),
                                    id: call.id.clone(),
                                    reason: reason.clone(),
                                });
                                ToolOutcome {
                                    content: format!("tool call denied by user: {reason}"),
                                    is_error: true,
                                }
                            }
                        }
                    }
                    ToolDecision::Allow => {
                        emit!(Event::ToolCallApproved {
                            agent: self.agent.clone(),
                            id: call.id.clone(),
                        });
                        let outcome = self.tools.execute(call).await;
                        emit!(Event::ToolCallCompleted {
                            agent: self.agent.clone(),
                            id: call.id.clone(),
                            is_error: outcome.is_error,
                        });
                        outcome
                    }
                };
                results.push(ContentBlock::ToolResult {
                    tool_use_id: call.id.clone(),
                    content: outcome.content,
                    is_error: outcome.is_error,
                });
            }
            messages.push(Message {
                role: crate::Role::User,
                content: results,
            });
        }
        Err(LoopError::MaxTurns(self.max_turns))
    }
}
