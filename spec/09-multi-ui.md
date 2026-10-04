# 09 — Arquitetura Limpa e Múltiplas UIs

> **Princípio supremo do projeto**: nenhuma funcionalidade pode existir apenas dentro de uma UI. Se um usuário do TUI consegue fazer X, o core expõe X via port e qualquer outra UI consegue fazer X.

## Contrato de UI (port de entrada)

O core expõe um application service único, orientado a **intents** (comandos) e **eventos** (saída):

```rust
/// Handle da aplicação — único ponto de contato de qualquer UI.
pub struct HarnessHandle { /* sender de intents + subscriber de eventos */ }

impl HarnessHandle {
    pub fn dispatch(&self, intent: Intent) -> Result<(), DispatchError>;
    pub fn subscribe(&self) -> broadcast::Receiver<UiUpdate>;
    pub fn snapshot(&self) -> AppStateView; // estado renderizável, imutável
}

/// Tudo que uma UI pode pedir ao sistema.
pub enum Intent {
    SendMessage { agent: AgentId, text: String },
    CancelGeneration { agent: AgentId },
    ResolveApproval { call: ToolCallId, decision: ApprovalDecision },
    SpawnAgent { parent: AgentId, label: String, prompt: String },
    InterruptAgent { agent: AgentId },
    InjectContext { agent: AgentId, text: String },
    ConfigureProvider { config: ProviderConfig },
    SetSandboxMode(SandboxMode),
    SwitchModel(ModelAlias),
    ResumeSession(SessionId),
    // ...
}

/// Tudo que o sistema publica para as UIs (dados, não widgets).
pub enum UiUpdate {
    AssistantDelta { agent: AgentId, text: String },
    ToolApprovalNeeded { call: PendingToolCall },  // UI DECIDE como perguntar
    AgentStatusChanged { agent: AgentId, status: AgentStatus },
    GraphChanged,                                  // UI reconsulta snapshot
    ModelChanged(ModelAlias),
    Error(UiError),
    // ...
}
```

Regras do contrato:
1. **`UiUpdate` carrega apenas dados de domínio** — nenhum tipo ratatui, nenhuma cor, nenhum layout. Renderização é problema da UI.
2. **Aprovação de tool é push, não pull**: o core emite `ToolApprovalNeeded` e bloqueia até chegar `ResolveApproval`. Uma UI headless pode auto-responder; o TUI abre modal. O core não sabe a diferença.
3. **`AppStateView`** é o modelo de leitura imutável (conversas, grafo de agentes, providers, sessões) — serve para render inicial e replays.
4. UIs novas **não adicionam variants** a `Intent`/`UiUpdate` sem atualizar o contrato + contract tests (ver abaixo).

## UIs planejadas

| UI | Status | Notas |
|---|---|---|
| `harnizator-cli` (headless, stdout scriptável) | Wave 1+ | Primeira prova do desacoplamento; usada em todos os testes de integração |
| `harnizator-tui` (ratatui) | Wave 3+ | UI principal |
| GUI/web (ex.: egui, Leptos, ou servidor WS + frontend) | backlog | Só pluga no contrato; zero mudança no core |

## Contract tests (obrigatório por UI)

Existe uma **suite de contrato compartilhada** (`harnizator-core/tests/ui_contract.rs`, exportada como harness reutilizável): qualquer crate de UI deve rodá-la contra si mesma.

```rust
// Toda UI deve passar:
contract_test_envia_mensagem_e_recebe_deltas(ui);
contract_test_aprovacao_de_tool_bloqueia_ate_decisao(ui);
contract_test_spawn_interrupt_inject_context(ui);
contract_test_todo_intent_tem_cobertura(ui); // reflexão sobre o enum
```

Métrica contínua: **feature parity** — um teste de reflexão garante que cada `Intent` tem pelo menos um binding na UI registrada; o resultado vai para um relatório de paridade (`just parity`).

## Anti-regressões de acoplamento (CI)

1. `harnizator-core` com **zero** deps de tokio/reqwest/ratatui/rusqlite/crossterm (checado por script no `just ci`).
2. `harnizator-tui` e `harnizator-cli` não importam `harnizator-providers`/`harnizator-tools`/`harnizator-store` diretamente — só `harnizator-core` (wiring fica em `harnizator-app`).
3. Nenhum `println!`/stdout fora dos adapters de UI.
4. Teste arquitetural: para qualquer feature nova, deve existir teste headless equivalente — features "só-TUI" são rejeitadas em review.

## Impacto nas waves

- Wave 1 entrega `harnizator-cli` junto (era "headless"; agora é adapter formal).
- Wave 3: TUI implementa a suite de contrato; paridade TUI↔CLI entra no `just ci`.
- Wave 4 em diante: qualquer Intent novo exige binding no CLI **antes** do binding no TUI (TDD: o teste headless é o "red" mais barato).
