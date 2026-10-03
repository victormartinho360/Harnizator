//! harness-cli: adapter de UI headless (spec/09-multi-ui.md).
//!
//! Consome o port `LlmProvider` escrevendo deltas em um `Write` qualquer.
//! É a primeira prova de que a funcionalidade vive fora da UI.

use std::io::Write;

use futures::StreamExt;
use harness_core::TokenUsage;
use harness_core::agent_loop::{AgentLoop, LoopError};
use harness_core::events::Event;
use harness_core::provider_port::{ChatRequest, LlmProvider, ProviderError, StreamChunk};
use harness_core::tool_port::{ApprovalDecision, ApprovalPort, ToolCall, ToolPort};

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("provider error: {0}")]
    Provider(#[from] ProviderError),
    #[error("agent loop error: {0}")]
    Loop(#[from] LoopError),
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),
}

/// Resultado de um chat headless.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct HeadlessSummary {
    pub text: String,
    pub usage: Option<TokenUsage>,
}

/// Roda um turno de chat: deltas vão para `out`, erros propagam.
pub async fn run_headless(
    provider: &dyn LlmProvider,
    req: ChatRequest,
    out: &mut dyn Write,
) -> Result<HeadlessSummary, CliError> {
    let mut stream = provider.stream(req).await?;
    let mut summary = HeadlessSummary::default();
    while let Some(chunk) = stream.next().await {
        match chunk? {
            StreamChunk::TextDelta(t) => {
                out.write_all(t.as_bytes())?;
                out.flush()?;
                summary.text.push_str(&t);
            }
            StreamChunk::Usage { input, output } => {
                summary.usage = Some(TokenUsage { input, output });
            }
            StreamChunk::MessageStart | StreamChunk::MessageStop | StreamChunk::ToolUse(_) => {}
        }
    }
    out.write_all(b"\n")?;
    Ok(summary)
}

/// Aprovador headless: aprova tudo (modo yolo programático).
pub struct AutoApprove;

#[async_trait::async_trait]
impl ApprovalPort for AutoApprove {
    async fn decide(&self, _call: &ToolCall, _reason: &str) -> ApprovalDecision {
        ApprovalDecision::Approve
    }
}

/// Turno de agente completo no headless: tools + aprovação programática.
///
/// Eventos de auditoria (tool calls aprovadas/negadas) vão para `err`;
/// o texto final do assistente vai para `out`.
pub async fn run_agent_headless(
    provider: &dyn LlmProvider,
    tools: &dyn ToolPort,
    approver: &dyn ApprovalPort,
    req: ChatRequest,
    out: &mut dyn Write,
    err: &mut dyn Write,
) -> Result<HeadlessSummary, CliError> {
    let mut audit = |event: &Event| match event {
        Event::ToolCallRequested { name, .. } => {
            let _ = writeln!(err, "[tool] {name} requested");
        }
        Event::ToolCallApproved { id, .. } => {
            let _ = writeln!(err, "[tool] {id} approved");
        }
        Event::ToolCallDenied { id, reason, .. } => {
            let _ = writeln!(err, "[tool] {id} denied: {reason}");
        }
        Event::ToolCallCompleted { id, is_error, .. } => {
            let _ = writeln!(err, "[tool] {id} completed (error={is_error})");
        }
        _ => {}
    };
    let run = AgentLoop::new(provider, tools, approver)
        .run_with_sink(req, &mut audit)
        .await?;
    out.write_all(run.outcome.text.as_bytes())?;
    out.write_all(b"\n")?;
    Ok(HeadlessSummary {
        text: run.outcome.text.clone(),
        usage: Some(run.outcome.usage),
    })
}
