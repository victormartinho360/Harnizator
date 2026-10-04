//! harness-rs: binário — wiring de adapters (spec/01).
//!
//! Wave 1-2: modo headless com provider real (config+vault) ou `--mock`,
//! tools de workspace com `--tools` e modo de sandbox via `--sandbox`.

use std::path::PathBuf;
use std::process::ExitCode;
use std::sync::Arc;

use anyhow::Context;
use clap::{Parser, Subcommand, ValueEnum};
use harness_core::provider_port::{ChatRequest, LlmProvider};
use harness_core::replay::replay_agent;
use harness_core::store_port::SessionStore;
use harness_core::tool_port::ToolPort;
use harness_core::{Message, ModelAlias};
use harness_providers::{MockProvider, ProviderConfig, ProviderRouter, Vault};
use harness_store::SqliteStore;
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

#[derive(Debug, Subcommand)]
enum Cmd {
    /// Lista sessões persistidas.
    Sessions,
    /// Imprime o transcript de uma sessão.
    Resume { id: String },
}

#[derive(Debug, Parser)]
#[command(name = "harness-rs", about = "HarnessRS — harness de agentes AI")]
struct Cli {
    #[command(subcommand)]
    command: Option<Cmd>,

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

    /// Não persiste sessão no SQLite local.
    #[arg(long)]
    no_store: bool,

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
    // StdoutLock/StderrLock não são Send: bufferizamos e imprimimos ao final.
    let mut out: Vec<u8> = Vec::new();
    let mut err: Vec<u8> = Vec::new();
    match tools {
        Some(belt) => {
            let summary = harness_cli::run_agent_headless(
                provider,
                &belt,
                &harness_cli::AutoApprove,
                req,
                &mut out,
                &mut err,
            )
            .await?;
            use std::io::Write;
            std::io::stdout().write_all(&out)?;
            std::io::stderr().write_all(&err)?;
            if let Some(u) = summary.usage {
                eprintln!("[usage] input={} output={}", u.input, u.output);
            }
        }
        None => {
            let summary = harness_cli::run_headless(provider, req, &mut out).await?;
            use std::io::Write;
            std::io::stdout().write_all(&out)?;
            if let Some(u) = summary.usage {
                eprintln!("[usage] input={} output={}", u.input, u.output);
            }
        }
    }
    Ok(())
}

fn open_store() -> anyhow::Result<SqliteStore> {
    std::fs::create_dir_all(dirs_config())?;
    Ok(SqliteStore::open(&dirs_config().join("harness.db"))?)
}

async fn real_main() -> anyhow::Result<()> {
    let cli = Cli::parse();
    match &cli.command {
        Some(Cmd::Sessions) => {
            for m in open_store()?.sessions()? {
                println!(
                    "{} │ {} │ {} · {}in/{}out",
                    m.id, m.title, m.model, m.usage.input, m.usage.output
                );
            }
            return Ok(());
        }
        Some(Cmd::Resume { id }) => {
            let store = open_store()?;
            let events = store.events(id)?;
            let msgs = replay_agent(&events, &harness_core::AgentId::new("root"));
            for m in msgs {
                let role = match m.role {
                    harness_core::Role::User => "you",
                    harness_core::Role::Assistant => "assistant",
                    harness_core::Role::System => "sys",
                };
                println!("[{role}] {}", m.text());
            }
            return Ok(());
        }
        None => {}
    }
    let Some(prompt) = cli.prompt.clone() else {
        return run_tui(&cli).await;
    };
    let prompt = prompt.as_str();

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
        dispatch(&provider, "mock/test-model".into(), prompt, tools_of(None)?).await?;
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
        prompt,
        tools_of(Some(&config))?,
    )
    .await?;
    Ok(())
}

/// Modo TUI (default quando não há prompt posicional).
async fn run_tui(cli: &Cli) -> anyhow::Result<()> {
    // Admin de providers (config + vault) — usado para resolver modelos sob demanda.
    let admin: Arc<dyn harness_core::provider_admin::ProviderAdmin> = {
        let vault = Vault::open(&dirs_config().join("vault.age")).ok();
        Arc::new(harness_providers::ProviderAdminService::new(
            dirs_config().join("config.toml"),
            vault,
        ))
    };

    // provider inicial: mock > --model > último modelo usado > placeholder vazio.
    // Sem provider inicial, o chat avisa para escolher um modelo (Ctrl+P/Ctrl+5).
    let initial: Option<(Arc<dyn LlmProvider>, String)> = if let Some(scenario) = &cli.mock {
        Some((
            Arc::new(MockProvider::from_scenario_file(scenario)?),
            "mock/test-model".to_string(),
        ))
    } else {
        let alias_raw = cli.model.clone().or_else(|| admin.last_model());
        match alias_raw {
            Some(raw) => match ModelAlias::parse(&raw)
                .map_err(|e| e.to_string())
                .and_then(|alias| {
                    admin
                        .resolve(&alias)
                        .map(|(p, m)| (p, m))
                        .map_err(|e| format!("{raw}: {e}"))
                }) {
                Ok(pair) => Some(pair),
                Err(e) => {
                    eprintln!("warn: {e} — inicie e escolha um modelo via Ctrl+P/Ctrl+5");
                    None
                }
            },
            None => None,
        }
    };

    let model_str = match &initial {
        Some((_, m)) => cli
            .model
            .clone()
            .or_else(|| admin.last_model())
            .unwrap_or_else(|| format!("{m}")),
        None => String::new(),
    };
    let provider: Arc<dyn LlmProvider> = match initial {
        Some((p, _)) => p,
        None => Arc::new(harness_providers::MockProvider::empty()),
    };

    let tools: Option<Arc<dyn ToolPort>> = if cli.tools {
        let cwd = std::env::current_dir()?;
        let belt = Toolbelt::new(ToolCtx::new(&cwd)?, sandbox_mode(cli.sandbox, None))
            .map_err(|e| anyhow::anyhow!(e))?;
        Some(Arc::new(belt))
    } else {
        None
    };

    let store: Option<Arc<dyn SessionStore>> = if cli.no_store {
        None
    } else {
        open_store().ok().map(|s| Arc::new(s) as _)
    };
    harness_tui::runtime::run(
        provider,
        tools,
        model_str,
        format!("{:?}", cli.sandbox),
        store,
        Some(admin),
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
