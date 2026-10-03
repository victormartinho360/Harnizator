# 04 — Tools, Aprovação e Sandbox

## Trait Tool

```rust
#[async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;            // JSON Schema p/ o LLM
    fn risk(&self, args: &serde_json::Value) -> Risk; // Read | Write | Execute | Network
    async fn execute(&self, args: serde_json::Value, ctx: &ToolCtx)
        -> Result<ToolOutput, ToolError>;
}
```

## Modos de sandbox (D8)

```rust
pub enum SandboxMode {
    ReadOnly,                       // nega Write/Execute/Network
    WriteOnly,                      // permite Read+Write, nega Execute
    OnlyFlagged(FlagPolicy),        // Execute só se comando casa allowlist/flaglist
    FullAccess,                     // tudo (ainda assim aprovação interativa, salvo allowlist)
}
```

- `bash` em `OnlyFlagged`: parse do comando → verifica flaglist. Flags organizadas em grupos nomeados no config:
  ```toml
  [sandbox.flags]
  safe-git = ["git status", "git diff *", "git log *"]
  rust = ["cargo *", "rustc *"]
  ```
- Match por glob do argv completo; `&&`, pipes e subshells forçam aprovação (comando composto nunca é auto-aprovado).

## Fluxo de aprovação

```
core emite Effect::RunTool(call)
 → sandbox.decide(call, mode) → Auto | NeedsApproval | Deny(reason)
 → NeedsApproval: Event::ToolCallRequested
 → TUI: dialog com diff/comando + opções:
     [y] aprovar uma vez   [a] aprovar e adicionar à allowlist da sessão
     [n] negar             [e] editar args antes de aprovar
 → decisão volta ao core como Intent; resultado vira tool_result no próximo turn
```

- Allowlist de sessão **nunca** persiste sem o usuário marcar "salvar no config".
- Toda decisão é registrada no event log (auditoria, alimenta 07-persistence).

## Tools builtin (Wave 2)

| Tool | Risk | Notas |
|---|---|---|
| `read_file` | Read | limites: 2k linhas default, offset/limit |
| `glob`, `grep` | Read | glob path-based; grep ripgrep-syntax |
| `write_file` | Write | cria/overwrite; exige path dentro do workspace |
| `edit_file` | Write | literal old→new, match único |
| `bash` | Execute | cwd configurável, timeout, output truncado com tail |

- Workspace root enforcement: syscalls resolvidos com `camino` + canonicalize; path escape ⇒ `Deny`.
- `ToolOutput`: conteúdo + `truncated: bool` + metadados (exit code, duração).

## Testes exigidos (antes da impl — Wave 2)
1. `SandboxPolicy::decide` — tabela completa modo × risk × allowlist (unitário exaustivo).
2. Parser de flaglist com glob (incl. rejeição de comandos compostos).
3. Path escape: `../`, symlink, path absoluto fora do root → Deny.
4. Reducer do dialog de aprovação: todas as transições de teclas (snapshots da UI).
5. Auditoria: decisão aprovada/negada aparece no event log.
6. Proptest: `edit_file` idempotência e erro quando match não é único.
