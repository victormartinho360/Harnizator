# 01 — Arquitetura

## Regra nº 1: Arquitetura Limpa / Ports & Adapters (obrigatória)

**Toda funcionalidade é independente de interface.** O sistema é organizado em anéis concêntricos, e dependências só apontam para dentro:

```
┌────────────────────────────────────────────────────────┐
│ ADAPTERS DE ENTRADA (UIs — plugáveis, N implementações)│
│   harness-tui (ratatui) · harness-cli (headless) ·     │
│   futura: GUI/web/IDE                                  │
├────────────────────────────────────────────────────────┤
│ ADAPTERS DE SAÍDA (infra)                              │
│   harness-providers (HTTP/SSE, vault) ·                │
│   harness-tools (fs/shell) · harness-store (SQLite)    │
├────────────────────────────────────────────────────────┤
│ USE CASES + PORTS        (harness-core)                │
│   AgentLoop, dispatch(Intent) → Vec<Effect>,           │
│   traits: LlmProvider, Tool, SessionStore, UiPort...   │
├────────────────────────────────────────────────────────┤
│ DOMAIN (harness-core, puro)                            │
│   Message, Event, AgentRegistry, SandboxPolicy...      │
└────────────────────────────────────────────────────────┘
```

- **Ports** são traits definidas no core. Nenhuma struct de infra (reqwest, rusqlite, ratatui) vaza para dentro do core.
- **UIs são adapters de entrada**: consomem o mesmo "application service" (`HarnessHandle`) e implementam o mesmo contrato (`UiPort`). Adicionar uma UI nova = adicionar um crate adapter, **zero mudanças** nos demais (ver 09-multi-ui.md).
- TUI atual e o modo `--headless` são apenas duas instâncias desse contrato — o que também prova a separação desde a Wave 3: toda feature do TUI deve funcionar headless.
- Violações são bloqueadas em CI: o core não pode declarar dependência de tokio/reqwest/ratatui/rusqlite (verificado com `cargo deny`/check de deps, ver 02-tdd-strategy.md).

## Workspace (D6)

```
harnessrs/
├── Cargo.toml                # workspace
├── crates/
│   ├── harness-core/         # domínio + use cases + ports. ZERO deps de IO/TUI/HTTP
│   ├── harness-providers/    # adapter: trait LlmProvider impls + vault
│   ├── harness-tools/        # adapter: trait Tool impls + execução sandbox
│   ├── harness-store/        # adapter: SQLite (rusqlite) implementando SessionStore
│   ├── harness-cli/          # adapter de entrada: UI headless (stdout/scriptável)
│   ├── harness-tui/          # adapter de entrada: ratatui (telas, widgets, render puro)
│   └── harness-app/          # binário: wiring, tokio runtime, CLI args (clap), config (figment)
├── spec/
└── tests/                    # testes E2E que sobem o binário contra MockProvider
```

Princípio-guia: **harness-core não conhece tokio, reqwest, crossterm ou ratatui.** Todo efeito colateral entra por traits injetadas. Isso é o que torna o TDD real: o agent loop inteiro roda em teste unitário com clocks e providers falsos.

## Crates em detalhe

### harness-core
- Tipos: `Message`, `Role`, `ContentBlock` (text/tool_use/tool_result), `TokenUsage`, `ModelAlias`, `SessionId`, `AgentId`.
- `Event` enum (bus interno): `AssistantDelta`, `ToolCallRequested`, `ToolCallApproved/Denied`, `ToolCallCompleted`, `AgentSpawned`, `AgentFinished`, `AgentInterrupted`, `ContextInjected`, `Error`.
- `AgentLoop`: máquina de estados pura `turn(state, input) -> (state, Vec<Effect>)`. `Effect` é um enum (`CallLlm`, `RunTool`, `SpawnAgent`, ...) executado por um runtime fora do core (**pattern "effects as data"**).
- Agent registry: `petgraph::DiGraph<AgentId, Relation>` para hierarquia.

### harness-providers
- `trait LlmProvider`: 
  ```rust
  #[async_trait]
  pub trait LlmProvider: Send + Sync {
      fn id(&self) -> &str;
      async fn stream(&self, req: ChatRequest) -> Result<BoxStream<'static, Result<StreamChunk, ProviderError>>, ProviderError>;
      async fn models(&self) -> Result<Vec<ModelInfo>, ProviderError>;
  }
  ```
- Roteador `ProviderRouter`: resolve alias `provider/model` → provider + api_key (via vault) + base_url (default ou override).
- `Vault`: criptografia age, chave do OS keyring ou `HARNESSRS_VAULT_KEY`.
- `MockProvider`: respostas scriptadas por scenario file — **peça central do TDD** e dos testes E2E.

### harness-tools
- `trait Tool`: `name()`, `schema() -> JsonSchema`, `requires_approval(&self, mode) -> Approval`, `execute(args, ctx) -> ToolOutput`.
- Builtins Wave 2: `read_file`, `write_file`, `edit_file`, `bash`, `glob`, `grep`.
- `SandboxPolicy` (D8): enum `ReadOnly | WriteOnly | OnlyFlagged(allowlist) | FullAccess`, resolvida por `(tool, args)`.

### harness-tui
- Puro de render: componentes recebem `&AppState` e desenham em `Frame`. Toda mutação de estado acontece em reducers testáveis (`fn reduce(state, Event) -> State diff`).
- Telas (ver 05-tui.md): Chat, ProviderSetup, Grafo, SessionPicker, Help.

### harness-app
- Binário `harnessrs`. clap: `harnessrs [--session ID] [--model alias] [--sandbox MODE] [--mock scenario]`.
- Wiring: cria bus `broadcast<Event>`, spawns de tasks, loop do crossterm → keymap → intents → core.

## Fluxo de um turn (async)

```
TUI key event → Intent::Send(text)
  → core::dispatch → Effect::CallLlm(req)
  → runtime: router.resolve(alias) → provider.stream(req)
  → chunks → Event::AssistantDelta (broadcast)
  → tool_use block → Effect::RunTool
  → sandbox.check() → se aprovação: Event::ToolCallRequested → TUI dialog
  → approve → execute → Event::ToolCallCompleted → resultado entra no próximo turn
```

## Cancelamento e concorrência (D16)
- Cada agente = `JoinSet` + `CancellationToken` próprio; interromper no grafo = `token.cancel()`.
- Broadcast com lag_tolerance configurável; persistência consome um subscriber dedicado.
