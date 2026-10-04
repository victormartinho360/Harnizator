# 06 — Subagentes e Grafo de Agentes

## Modelo

- Cada sessão tem um agente **root** (o chat principal). Subagentes são criados pela tool `spawn_agent` (disponível ao LLM) ou pelo usuário (`Ctrl+N` no grafo).
- `AgentNode { id, parent, label, status: Running|WaitingApproval|Idle|Done|Failed|Interrupted, model, token_usage }`.
- Arestas = parent/child. Filhos herdam: permissões de sandbox (nunca mais permissivos), modelo (override possível), e recebem contexto inicial = instrução do pai (**não** o histórico completo, por padrão).

## Concorrência (D16)
- `AgentRegistry` no core; cada agente roda numa task com `CancellationToken` e um `mpsc` de inbox.
- Limite de paralelismo: `max_concurrent_agents` (default 8); excedente fica `Queued`.
- Subagente pode spawna seus próprios filhos (profundidade máx `max_depth`, default 4).

## Tela Graph (D7)

- Layout: camadas por profundidade (root no topo), nós como boxes `[label ●status]`, arestas ASCII/unicode; cores por status.
- Navegação: setas/hjkl move seleção; nó selecionado mostra painel lateral com últimas N linhas de atividade.
- **Ações por nó**:
  | Tecla | Ação |
  |---|---|
  | `Enter` | focar chat do agente (abre Chat screen com a sessão dele) |
  | `m` | enviar mensagem/instrução ao agente (inbox) |
  | `i` | **context injection**: insere `Event::ContextInjected { text }` — entra como mensagem de sistema efêmera no próximo turn do agente |
  | `x` | interromper (cancel token; confirma se está em tool call) |
  | `Ctrl+N` | spawn manual de filho a partir do nó selecionado |
  | `r` | retry de agente `Failed` (mesmo prompt, novo turno) |
- Pan/zoom quando o grafo excede a área (offset + zoom factor; zoom muda tamanho do box).

## Context injection — semântica
- Injeção é **append-only** e auditada: aparece no transcript como `> [context-injected by user @ t]`.
- Não edita histórico passado; afeta apenas turns futuros.

## Testes exigidos (antes da impl — Wave 4)
1. Registry: spawn/finish/interrupt mantêm invariantes do grafo (proptest sobre sequências de Event).
2. Herança de sandbox: filho não escala permissão (unitário).
3. Render do grafo: snapshots para 1, 5 e 50 nós (com pan/zoom); grafo degenerado (cadeia linear, estrela).
4. Reducer das ações por nó, incl. `x` com confirmação.
5. Context injection: aparece no próximo `ChatRequest` do agente (integração core+MockProvider).
6. Concorrência: `max_concurrent_agents` respeitado; `Queued → Running` ao terminar outro.
