//! harness-rs: binário — wiring de adapters (spec/01).
//!
//! Wave 1-2: modo headless com provider real (config+vault) ou `--mock`,
//! tools de workspace com `--tools` e modo de sandbox via `--sandbox`.

use std::path::PathBuf;
use std::process::ExitCode;

use anyhow::Context;
use clap::{Parser, ValueEnum};
use harness_core::provider_port::{ChatRequest, LlmProvider};
use harness_core::{Message, ModelAlias};
use harness_providers::{MockProvider, ProviderConfig, ProviderRouter, Vault};
use harness_tools::{FlagPolicy, SandboxMode, ToolCtx, Toolbelt};

#[derive(Debug, Clone, Copy, ValueEnum)]
enum SandboxFlag {
    /// Nega write/exec.
    ReadOnly,
    /// Permite read+write, nega exec.
    WriteOnly,
    /// Exec só para comandos da flaglist ([sandbox.flags] no config).
    OnlyFlagged,
    /// Tudo com aprovação (headless: aprovador programático).
    FullAccess,
}

#[derive(Debug, Parser)]
#[command(name = "harness-rs", about = "HarnessRS — harness de agentes AI")]
struct Cli {
    /// Cenário TOML do MockProvider (desenvolvimento/testes, offline).
    #[arg(long)]
    mock: Option<PathBuf>,

    /// Alias `provider/model` (ex.: anthropic/claude-sonnet-4-5).
    #[arg(long)]
    model: Option<String>,

    /// Habilita tools de workspace no agent loop.
    #[arg(long)]
    tools: bool,

    /// Modo de sandbox das tools.
    #[arg(long, value_enum, default_value_t = SandboxFlag::OnlyFlagged)]
    sandbox: SandboxFlag,

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

fn sandbox_mode(flag: SandboxFlag, config: Option<&ProviderConfig>) -> SandboxMode {
    match flag {
        SandboxFlag::ReadOnly => SandboxMode::ReadOnly,
        SandboxFlag::WriteOnly => SandboxMode::WriteOnly,
        SandboxFlag::FullAccess => SandboxMode::FullAccess,
        SandboxFlag::OnlyFlagged => {
            let mut policy = FlagPolicy::new();
            if let Some(_cfg) = config {
                // grupos de flags do config entram na Wave 6 (tela); por ora, defaults seguros
                policy.add_group("rust", &["cargo *"]);
                policy.add_group("safe-git", &["git status", "git diff *", "git log *"]);
            } else {
                policy.add_group("rust", &["cargo *"]);
                policy.add_group("safe-git", &["git status", "git diff *", "git log *"]);
            }
            SandboxMode::OnlyFlagged(policy)
        }
    }
}

async fn dispatch(
    provider: &dyn LlmProvider,
    model: String,
    prompt: &str,
    tools: Option<Toolbelt>,
) -> anyhow::Result<()> {
    let req = ChatRequest {
        model,
        messages: vec![Message::user(prompt)],
        max_tokens: 4096,
        system: None,
        tools: vec![],
    };
    let mut out = std::io::stdout().lock();
    match tools {
        Some(belt) => {
            let mut err = std::io::stderr().lock();
            let summary = harness_cli::run_agent_headless(
                provider,
                &belt,
                &harness_cli::AutoApprove,
                req,
                &mut out,
                &mut err,
            )
            .await?;
            if let Some(u) = summary.usage {
                eprintln!("[usage] input={} output={}", u.input, u.output);
            }
        }
        None => {
            let summary = harness_cli::run_headless(provider, req, &mut out).await?;
            if let Some(u) = summary.usage {
                eprintln!("[usage] input={} output={}", u.input, u.output);
            }
        }
    }
    Ok(())
}

async fn real_main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    let prompt = cli.prompt.context("missing prompt (TUI chega na Wave 3)")?;

    let tools_of = |config: Option<&ProviderConfig>| -> anyhow::Result<Option<Toolbelt>> {
        if !cli.tools {
            return Ok(None);
        }
        let cwd = std::env::current_dir()?;
        let belt = Toolbelt::new(ToolCtx::new(&cwd)?, sandbox_mode(cli.sandbox, config))
            .map_err(|e| anyhow::anyhow!(e))?;
        Ok(Some(belt))
    };

    if let Some(scenario) = cli.mock {
        let provider =
            MockProvider::from_scenario_file(&scenario).context("failed to load scenario")?;
        dispatch(
            &provider,
            "mock/test-model".into(),
            &prompt,
            tools_of(None)?,
        )
        .await?;
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

    dispatch(
        resolved.provider.as_ref(),
        resolved.model,
        &prompt,
        tools_of(Some(&config))?,
    )
    .await?;
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
