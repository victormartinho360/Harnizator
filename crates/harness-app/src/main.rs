//! harness-rs: binário — wiring de adapters (spec/01).
//!
//! Wave 1: modo headless com provider real (config+vault) ou `--mock`.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
use clap::Parser;
use harness_core::{Message, ModelAlias};
use harness_providers::{ChatRequest, MockProvider, ProviderConfig, ProviderRouter, Vault};

#[derive(Debug, Parser)]
#[command(name = "harness-rs", about = "HarnessRS — harness de agentes AI")]
struct Cli {
    /// Cenário TOML do MockProvider (desenvolvimento/testes, offline).
    #[arg(long)]
    mock: Option<PathBuf>,

    /// Alias `provider/model` (ex.: anthropic/claude-sonnet-4-5).
    #[arg(long)]
    model: Option<String>,

    /// Mensagem do usuário (modo headless de um turno).
    prompt: Option<String>,
}

#[tokio::main]
async fn main() -> ExitCode {
    match real_main().await {
        Ok(()) => ExitCode::SUCCESS,
        Err(e) => {
            eprintln!("error: {e:#}");
            ExitCode::FAILURE
        }
    }
}

async fn real_main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let prompt = cli.prompt.context("missing prompt (TUI chega na Wave 3)")?;

    let req_of = |model: String| ChatRequest {
        model,
        messages: vec![Message::user(&prompt)],
        max_tokens: 4096,
        system: None,
    };

    let mut out = std::io::stdout().lock();
    if let Some(scenario) = cli.mock {
        let provider =
            MockProvider::from_scenario_file(&scenario).context("failed to load scenario")?;
        harness_cli::run_headless(&provider, req_of("mock/test-model".into()), &mut out).await?;
        return Ok(());
    }

    let alias_raw = cli.model.context("missing --model (or --mock)")?;
    let alias = ModelAlias::parse(&alias_raw)?;
    let config_path = dirs_config().join("config.toml");
    let config_src = std::fs::read_to_string(&config_path)
        .with_context(|| format!("cannot read {}", config_path.display()))?;
    let config: ProviderConfig = toml::from_str(&config_src)?;
    let vault = Vault::open(&dirs_config().join("vault.age"))?;
    let router = ProviderRouter::new(&config, &vault)?;
    let resolved = router.resolve(&alias)?;

    let summary =
        harness_cli::run_headless(resolved.provider.as_ref(), req_of(resolved.model), &mut out)
            .await?;
    if let Some(u) = summary.usage {
        eprintln!("[usage] input={} output={}", u.input, u.output);
    }
    Ok(())
}

fn dirs_config() -> PathBuf {
    std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            PathBuf::from(std::env::var("HOME").unwrap_or_else(|_| ".".to_string())).join(".config")
        })
        .join("harnessrs")
}
