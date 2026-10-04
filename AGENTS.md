# AGENTS.md — Guia para agentes de IA trabalhando no Harnizator

> Este arquivo é leitura **obrigatória** antes de qualquer alteração de código.
> Ele descreve como este projeto é construído, testado e versionado.

## 1. O que é o projeto

**Harnizator** — harness de agentes AI full-TUI, escrito 100% em Rust, desenvolvido
**estritamente com TDD** (red → green → refactor). Terminal onde o usuário conversa
com LLMs (Anthropic, OpenAI, Google, NIM, qualquer endpoint OpenAI-compatible),
com tools de workspace, aprovação interativa e subagentes concorrentes.

A especificação completa vive em [`spec/`](spec/README.md). Leia o spec relevante
antes de mexer numa área (ex.: `spec/03-providers.md` antes de tocar em providers).

## 2. Regra nº 1: Arquitetura Limpa (ports & adapters)

Dependências só apontam **para dentro**:

```
UIs:     harnizator-tui (ratatui) · harnizator-cli (headless)
Adapters: harnizator-providers (HTTP/SSE, vault) · harnizator-tools (fs/shell) ·
          harnizator-store (SQLite)
Core:    harnizator-core — domínio + use cases + ports (traits)
```

**Regras duras (CI assegura — `just check-arch`)**:
- `harnizator-core` **não pode** depender de `tokio`, `reqwest`, `ratatui`, `crossterm`,
  `rusqlite`. Nenhuma struct de infra vaza pra dentro.
- Nenhuma funcionalidade existe só numa UI. Se o TUI faz X, o core expõe X via port.
- Lógica de negócio nos use cases do core; adapters são só IO + mapeamento.
- TUI usa **reducers puros** (`state.rs`): `handle_key(evento) -> Action` —
  sem efeitos colaterais. O `runtime.rs` executa as Actions.

## 3. Fluxo TDD (obrigatório)

**Nenhum código de produção sem teste falhando antes.**

1. **Red**: escreva o teste que falha (`cargo test` deve mostrar a falha esperada).
2. **Green**: implementação mínima pra passar.
3. **Refactor**: limpar, sem quebrar testes.

Ferramentas padrão (ver `spec/02-tdd-strategy.md`):

| Ferramenta | Para quê |
|---|---|
| `cargo test` / `cargo nextest run` | runner (`just test`) |
| `insta` + `TestBackend` | snapshots de telas do TUI |
| `wiremock` | testes HTTP de providers sem rede |
| `rexpect` (PTY) | E2E do binário |
| `proptest` | reducers/parsers nunca panicam |

Convenções de teste:
- Test files em `crates/<crate>/tests/`, comentário `//! Red tests: ...` no topo
  e `#![allow(clippy::unwrap_used, ...)]`.
- Fixers YAML/TOML em `crates/<crate>/tests/fixtures/`.
- Snapshots insta em `crates/harnizator-tui/tests/snapshots/`. Se uma mudança visual
  for intencional, atualize com `INSTA_UPDATE=always cargo test -p harnizator-tui` e
  revise o diff.

## 4. Ambiente (Fedora Atomic / toolbox)

O host pode não ter toolchain. Use:

```bash
# se gcc/cargo não estiverem no PATH do host:
toolbox run -c Deepseek_Harness bash -c "cd $PWD && cargo test --workspace"

# task runner (instale `just` com: cargo install just)
just build-release   # cargo build --release -p harnizator-app
just test            # nextest | cargo test
just ci              # fmt + clippy -D warnings + tests + doc + check-arch
```

Binário final: `target/release/harnizator`.

## 5. Config, vault e providers

- Config: `~/.config/harnizator/config.toml` (seções `[providers.*]`, `[ui]`).
- Chaves: `~/.config/harnizator/vault.age` (age + scrypt). Env: `HARNIZATOR_VAULT_KEY`.
- **O vault é decriptado uma vez por sessão e cacheado em memória** (invalida via
  mtime). Não re-introduza decriptação por chamada — foi a causa de uma regressão
  de performance grave na UI.
- Providers com `models()` implementado listam modelos reais da API. Nunca
  hardcode ids de modelo em defaults de UI.
- Alias de modelo é sempre `provider/model` (`ModelAlias`). NIM = kind `nim`,
  endpoint default `https://integrate.api.nvidia.com/v1`, fala protocolo OpenAI.

## 6. Git e workflow

- Trabalhe em branches `feat/<nome-curto>`; faça merge fast-forward quando limpo.
- Commits em português, formato: `<tipo>: <resumo>` (feat, fix, perf, chore, test…)
  com corpo descrevendo o porquê quando não for óbvio.
- Rode `just ci` (ou ao menos `cargo test --workspace` + `cargo clippy --workspace
  --all-targets -- -D warnings`) antes de abrir PR/merge.
- Remote: `origin` = `git@github.com:victormartinho360/Harnizator.git`,
  branch principal: `main`.

## 7. Ao mexer na TUI

- Estado novo no `AppState` → inicialize no construtor `new()` e cubra com teste de reducer.
- Nova `Action` → o `runtime.rs` precisa de um braço no `match` (o compilador cobra).
- Teclas novas: reserve um keybinding documentado na tela de help (`ui.rs`).
- O command palette (Ctrl+P / `:`) é fora do ciclo Tab — não adicione ao `next_screen`.
- Snapshot tests em `TestBackend` cobrem o render; adicione um ao criar tela nova.

## 8. Erros comuns / aprendizados (não repita)

- **Vault scrypt no hot path**: proibido (ver §5).
- **Defaults de modelo hardcoded** quebram quando a API muda (causa de 404s reais) —
  sempre liste modelos via `models()`.
- **`models()`/`list_models` sem tratamento de auth**: 401/403 → `ProviderError::Auth`,
  429 → `RateLimited` (retry), resto → `Stream(msg com corpo)` pra diagnóstico.
- Testes E2E usam `--mock` com cenários TOML; nunca chame rede de verdade em teste.
- Em containers sem toolchain, **não** instale com `sudo` no host (flag no-new-privileges) —
  use a toolbox existente.
