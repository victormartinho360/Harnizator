//! Serviço de agente persistente: roda turns até a inbox fechar (spec/06).
//!
//! Entre turns, drena a inbox (injeção de contexto entra como mensagem
//! de usuário efêmera). O cancelamento é feito abortando a task fora.

use std::sync::{Arc, Mutex};

use futures::StreamExt;
use futures::channel::mpsc::UnboundedReceiver;

use crate::agent_loop::{AgentLoop, LoopError};
use crate::agents::AgentManager;
use crate::events::Event;
use crate::message::Message;
use crate::provider_port::{ChatRequest, LlmProvider};
use crate::tool_port::{ApprovalPort, ToolPort};
use crate::{AgentId, TokenUsage};

/// Resultado do serviço quando a inbox fecha (agente vai a Done).
pub struct ServiceResult {
    pub final_text: String,
    pub history: Vec<Message>,
    pub usage: TokenUsage,
}

/// Parâmetros do serviço de agente (agrupados para clareza).
pub struct ServiceCtx<'a> {
    pub provider: &'a dyn LlmProvider,
    pub tools: &'a dyn ToolPort,
    pub approver: &'a dyn ApprovalPort,
    pub manager: Arc<Mutex<AgentManager>>,
    pub agent: AgentId,
    pub model: String,
}

/// Roda o ciclo: turn → drena inbox → turn … até a inbox fechar.
pub async fn run_agent_service(
    cx: ServiceCtx<'_>,
    initial_history: Vec<Message>,
    inbox: &mut UnboundedReceiver<String>,
    sink: &mut (dyn FnMut(&Event) + Send),
) -> Result<ServiceResult, LoopError> {
    let ServiceCtx {
        provider,
        tools,
        approver,
        manager,
        agent,
        model,
    } = cx;
    let model = model.as_str();
    let mut history = initial_history;
    let mut total_usage = TokenUsage::default();

    loop {
        let req = ChatRequest {
            model: model.to_string(),
            messages: history.clone(),
            max_tokens: 4096,
            system: None,
            tools: tools.specs(),
        };
        let run = AgentLoop::new(provider, tools, approver)
            .with_agent_id(agent.clone())
            .run_with_sink(req, sink)
            .await?;
        history = run.messages;
        total_usage.input += run.outcome.usage.input;
        total_usage.output += run.outcome.usage.output;
        {
            let mut m = match manager.lock() {
                Ok(g) => g,
                Err(p) => p.into_inner(),
            };
            m.mark_idle(&agent);
        }

        // drena injeções já disponíveis; senão espera
        let mut buffered: Vec<String> = Vec::new();
        while let Ok(text) = inbox.try_recv() {
            buffered.push(text);
        }
        if buffered.is_empty() {
            match inbox.next().await {
                Some(text) => buffered.push(text),
                None => {
                    break; // canal fechado: serviço termina
                }
            }
        }
        for text in buffered {
            sink(&Event::ContextInjected {
                agent: agent.clone(),
                text: text.clone(),
            });
            history.push(Message::user(text));
            {
                let mut m = match manager.lock() {
                    Ok(g) => g,
                    Err(p) => p.into_inner(),
                };
                m.set_running(&agent);
            }
        }
    }

    {
        let mut m = match manager.lock() {
            Ok(g) => g,
            Err(p) => p.into_inner(),
        };
        m.mark_finished(&agent);
    }
    sink(&Event::AgentFinished {
        agent: agent.clone(),
    });
    // texto final = última mensagem do assistente no histórico
    let final_text = history
        .iter()
        .rev()
        .find(|m| matches!(m.role, crate::Role::Assistant))
        .map(Message::text)
        .unwrap_or_default();
    Ok(ServiceResult {
        final_text,
        history,
        usage: total_usage,
    })
}
