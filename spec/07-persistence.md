# 07 — Persistência (SQLite / rusqlite)

## Schema (migrations numeradas, `m0001_*.sql`)

```sql
sessions(id TEXT PK, title TEXT, model TEXT, sandbox_mode TEXT,
         created_at INTEGER, updated_at INTEGER, total_input_tokens INT, total_output_tokens INT)
agents(id TEXT PK, session_id TEXT FK, parent_id TEXT NULL, label TEXT, status TEXT, model TEXT)
events(id INTEGER PK AUTOINCREMENT, agent_id TEXT FK, seq INTEGER, ts INTEGER,
       kind TEXT, payload JSON)   -- append-only; seq por agente
```

- **Event-sourcing-lite**: estado da conversa é reconstituído replayando `events` ordenados por (agent_id, seq). Mensagens derivadas não são tabelas separadas na Wave 5.
- Writes em task dedicada (subscriber do broadcast) com batch + WAL mode; nunca bloqueia o TUI.
- Crash recovery: ao abrir sessão, replay até último evento; agente `Running` no crash vira `Interrupted`.

## Tela Sessions
Listagem com busca fuzzy (`/`), título auto-gerado pelo primeiro prompt (trunc 60 chars), custo acumulado por sessão.

## Testes exigidos (antes da impl — Wave 5)
1. Migration up/down idempotente; abrir DB vazio cria schema.
2. Roundtrip: sessão com tool calls + subagentes → replay gera estado idêntico (snapshot do estado reduzido).
3. Crash recovery: kill no meio do stream → reopen = `Interrupted`, transcript íntegro.
4. Concorrência: writer task não perde eventos sob burst de 10k deltas (test com canal sob pressão).
5. Snapshots da tela Sessions.
