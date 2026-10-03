//! harness-cli: adapter de UI headless (spec/09-multi-ui.md).
//!
//! Consome o port `LlmProvider` escrevendo deltas em um `Write` qualquer.
//! É a primeira prova de que a funcionalidade vive fora da UI.

use std::io::Write;

use futures::StreamExt;
use harness_core::TokenUsage;
use harness_providers::{ChatRequest, LlmProvider, ProviderError, StreamChunk};

#[derive(Debug, thiserror::Error)]
pub enum CliError {
    #[error("provider error: {0}")]
    Provider(#[from] ProviderError),
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
            StreamChunk::MessageStart | StreamChunk::MessageStop => {}
        }
    }
    out.write_all(b"\n")?;
    Ok(summary)
}
