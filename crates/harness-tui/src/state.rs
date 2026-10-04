//! Estado do TUI e reducers puros (spec/05, 02).

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use harness_core::TokenUsage;
use harness_core::events::Event;
use harness_core::tool_port::{ApprovalDecision, ToolCall};
use harness_core::{AgentId, ModelAlias};
use harness_core::agents::NodeView;

use crate::ui_msg::UiMsg;

/// Tela ativa (roteador do shell).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Screen {
    Chat,
    Graph,
    Sessions,
    Providers,
    Help,
    CommandPalette,
}

/// Qual entidade o input está mirando.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputMode {
    Chat,
    Message(AgentId),
    Inject(AgentId),
    /// Definindo a chave de API de um provider (input mascarado).
    SetKey(String),
    AddProviderName,
    AddProviderKind {
        name: String,
    },
    AddProviderBaseUrl {
        name: String,
        kind: String,
    },
    /// Command palette (Ctrl+P) — fuzzy search over providers/models + commands.
    CommandPalette { query: String },
}

/// Papel visual da mensagem no chat.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayRole {
    User,
    Assistant,
    Tool,
    System,
}

/// Uma mensagem renderizável no histórico.
#[derive(Debug, Clone)]
pub struct ChatMessage {
    pub role: DisplayRole,
    pub content: String,
    /// Mensagem ainda recebendo deltas de streaming.
    pub streaming: bool,
}

/// Aprovação pendente exibida no modal.
#[derive(Debug, Clone)]
pub struct PendingApproval {
    pub call: ToolCall,
    pub reason: String,
}

/// Ação que o reducer pede ao runtime executar.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    /// Interrompe o agente (e subárvore).
    Interrupt(AgentId),
    /// Envia mensagem para a inbox do agente.
    SendToAgent(AgentId, String),
    /// Injeta contexto no próximo turno do agente.
    InjectContext(AgentId, String),
    /// Carrega uma sessão persistida no chat.
    ResumeSession(String),
    /// Define a chave de API de um provider no vault.
    SetProviderKey(String, String),
    /// Adiciona provider custom ao config.
    AddProvider {
        name: String,
        kind: String,
        base_url: String,
    },
    /// Remove provider custom.
    RemoveProvider(String),
    /// Testa conexão (models()) de um provider.
    TestConnection(String),
    /// Define o modelo ativo (provider/model).
    SetActiveModel(ModelAlias),
    /// Exporta template de providers para arquivo TOML.
    ExportProvidersTemplate(String),
    /// Envia o texto como mensagem do usuário.
    Send(String),
    /// Resolve a aprovação pendente com a decisão dada.
    Approve(ApprovalDecision),
    /// Cancela a geração corrente.
    Cancel,
    Quit,
}

/// Estado do TUI (modelo de leitura mutável; reducers são determinísticos).
#[derive(Debug)]
pub struct AppState {
    pub messages: Vec<ChatMessage>,
    pub input: String,
    pub generating: bool,
    pub awaiting_approval: Option<PendingApproval>,
    pub follow: bool,
    pub scroll: u16,
    pub model: String,
    /// Modelo ativo selecionado pelo usuário (`provider/model`).
    pub active_model: Option<ModelAlias>,
    pub sandbox: String,
    pub usage: TokenUsage,
    pub screen: Screen,
    pub graph_nodes: Vec<NodeView>,
    pub graph_selected: usize,
    pub graph_offset: (u16, u16),
    pub input_mode: InputMode,
    pub sessions: Vec<harness_core::store_port::SessionMeta>,
    pub sessions_selected: usize,
    pub providers: Vec<crate::ui_msg::ProviderView>,
    pub providers_selected: usize,
    /// Status da tela Providers (resultado de teste de conexão etc).
    pub providers_status: String,
}

impl AppState {
    pub fn new(model: &str, sandbox: &str) -> Self {
        let active_model = ModelAlias::parse(model).ok();
        Self {
            messages: vec![ChatMessage {
                role: DisplayRole::System,
                content: "HarnessRS — Enter envia · Esc cancela geração · Ctrl+C sai".into(),
                streaming: false,
            }],
            input: String::new(),
            generating: false,
            awaiting_approval: None,
            follow: true,
            scroll: 0,
            model: model.to_string(),
            active_model,
            sandbox: sandbox.to_string(),
            usage: TokenUsage::default(),
            screen: Screen::Chat,
            graph_nodes: Vec::new(),
            graph_selected: 0,
            graph_offset: (0, 0),
            input_mode: InputMode::Chat,
            sessions: Vec::new(),
            sessions_selected: 0,
            providers: Vec::new(),
            providers_selected: 0,
            providers_status: String::new(),
        }
    }

    /// Mensagem do usuário entra no histórico (antes do loop rodar).
    pub fn push_user_message(&mut self, text: String) {
        self.end_streaming();
        self.messages.push(ChatMessage {
            role: DisplayRole::User,
            content: text,
            streaming: false,
        });
        self.generating = true;
    }

    fn end_streaming(&mut self) {
        for m in &mut self.messages {
            m.streaming = false;
        }
    }

    fn push_tool_line(&mut self, content: String) {
        self.messages.push(ChatMessage {
            role: DisplayRole::Tool,
            content,
            streaming: false,
        });
    }

    /// Reducer de eventos de domínio do core.
    pub fn apply_event(&mut self, event: &Event) {
        match event {
            Event::AssistantDelta { text, .. } => {
                match self
                    .messages
                    .last_mut()
                    .filter(|m| m.role == DisplayRole::Assistant && m.streaming)
                {
                    Some(m) => m.content.push_str(text),
                    None => self.messages.push(ChatMessage {
                        role: DisplayRole::Assistant,
                        content: text.clone(),
                        streaming: true,
                    }),
                }
                self.generating = true;
            }
            Event::ToolCallRequested { name, args, .. } => {
                let pretty = serde_json::to_string(args).unwrap_or_default();
                self.push_tool_line(format!("▶ {name} {pretty}"));
            }
            Event::ToolCallApproved { id, .. } => {
                self.push_tool_line(format!("  ✓ {id} approved"));
            }
            Event::ToolCallDenied { id, reason, .. } => {
                self.push_tool_line(format!("  ✗ {id} denied: {reason}"));
            }
            Event::ToolCallCompleted { id, is_error, .. } => {
                let mark = if *is_error { "erro" } else { "ok" };
                self.push_tool_line(format!("  · {id} completed ({mark})"));
            }
            Event::Error { message } => {
                self.messages.push(ChatMessage {
                    role: DisplayRole::System,
                    content: format!("error: {message}"),
                    streaming: false,
                });
            }
            _ => {}
        }
    }

    /// Reducer de mensagens de UI do runtime.
    pub fn apply_ui_msg(&mut self, msg: UiMsg) {
        match msg {
            UiMsg::Core(ev) => self.apply_event(&ev),
            UiMsg::NeedsApproval { call, reason } => {
                self.awaiting_approval = Some(PendingApproval { call, reason });
            }
            UiMsg::ApprovalResolved => {
                self.awaiting_approval = None;
            }
            UiMsg::ProvidersSync(list) => {
                if !list.is_empty() && self.providers_selected >= list.len() {
                    self.providers_selected = list.len() - 1;
                }
                self.providers = list;
            }
            UiMsg::ProviderTestResult { id, result } => {
                self.providers_status = match result {
                    Ok(info) => format!("{id}: ok ({info})"),
                    Err(e) => format!("{id}: {e}"),
                };
            }
            UiMsg::SessionsSync(sessions) => {
                if !sessions.is_empty() && self.sessions_selected >= sessions.len() {
                    self.sessions_selected = sessions.len() - 1;
                }
                self.sessions = sessions;
            }
            UiMsg::GraphSync(nodes) => {
                if !nodes.is_empty() && self.graph_selected >= nodes.len() {
                    self.graph_selected = nodes.len() - 1;
                }
                self.graph_nodes = nodes;
            }
            UiMsg::SyncHistory(_messages, usage) => {
                self.usage = usage;
            }
            UiMsg::TurnFinished(Ok(())) => {
                self.generating = false;
                self.end_streaming();
            }
            UiMsg::TurnFinished(Err(e)) => {
                self.generating = false;
                self.end_streaming();
                self.messages.push(ChatMessage {
                    role: DisplayRole::System,
                    content: format!("error: {e}"),
                    streaming: false,
                });
            }
        }
    }

    /// Processa tecla principal — retorna Action para o runtime executar.
    pub fn handle_key(&mut self, key: KeyEvent) -> Action {
        // modal de aprovação captura tudo
        if self.awaiting_approval.is_some() {
            return match (key.code, key.modifiers) {
                (KeyCode::Char('y'), _) => Action::Approve(ApprovalDecision::Approve),
                (KeyCode::Char('a'), _) => Action::Approve(ApprovalDecision::ApproveAndAllowlist),
                (KeyCode::Char('n'), _) | (KeyCode::Esc, _) => {
                    Action::Approve(ApprovalDecision::Deny {
                        reason: "user denied".into(),
                    })
                }
                _ => Action::None,
            };
        }
        // roteamento global de telas: Tab/Shift+Tab ciclam, F1-F6 saltam direto.
        // Ctrl+dígito permanece como fallback (terminais com CSI-u); Ctrl+2 em
        // terminais legacy chega como NUL e também abre o grafo.
        match (key.code, key.modifiers) {
            (KeyCode::Tab, KeyModifiers::NONE) => {
                self.screen = next_screen(self.screen);
                return Action::None;
            }
            (KeyCode::BackTab, _) => {
                self.screen = prev_screen(self.screen);
                return Action::None;
            }
            (KeyCode::F(n), KeyModifiers::NONE) => {
                if let Some(scr) = screen_for_f(n) {
                    self.screen = scr;
                    return Action::None;
                }
            }
            (KeyCode::Null, _) => {
                self.screen = Screen::Graph;
                return Action::None;
            }
            // atalhos Ctrl+letra confiáveis (Ctrl+dígito é ambíguo em legacy)
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => return Action::Quit,
            (KeyCode::Char('g'), KeyModifiers::CONTROL) => {
                self.screen = Screen::Graph;
                return Action::None;
            }
            (KeyCode::Char('p'), KeyModifiers::CONTROL) => {
                self.screen = Screen::CommandPalette;
                self.input_mode = InputMode::CommandPalette { query: String::new() };
                return Action::None;
            }
            (KeyCode::Char(':'), KeyModifiers::NONE | KeyModifiers::SHIFT)
                if self.screen == Screen::Chat
                    && self.input_mode == InputMode::Chat
                    && self.input.is_empty() =>
            {
                self.screen = Screen::CommandPalette;
                self.input_mode = InputMode::CommandPalette { query: String::new() };
                return Action::None;
            }
            _ => {}
        }
        match (key.code, key.modifiers) {
            (KeyCode::Char('1'), KeyModifiers::CONTROL) => {
                self.screen = Screen::Chat;
                return Action::None;
            }
            (KeyCode::Char('2'), KeyModifiers::CONTROL) => {
                self.screen = Screen::Graph;
                return Action::None;
            }
            (KeyCode::Char('3'), KeyModifiers::CONTROL) => {
                self.screen = Screen::Help;
                return Action::None;
            }
            (KeyCode::Char('4'), KeyModifiers::CONTROL) => {
                self.screen = Screen::Sessions;
                return Action::None;
            }
            (KeyCode::Char('5'), KeyModifiers::CONTROL) => {
                self.screen = Screen::Providers;
                return Action::None;
            }
            _ => {}
        }
        if self.screen == Screen::Graph {
            return self.handle_graph_key(key);
        }
        if self.screen == Screen::Sessions {
            return self.handle_sessions_key(key);
        }
        if self.screen == Screen::Providers {
            return self.handle_providers_key(key);
        }
        if self.screen == Screen::CommandPalette {
            return self.handle_command_palette_key(key);
        }
        match (key.code, key.modifiers) {
            (KeyCode::Char('c'), KeyModifiers::CONTROL) => Action::Quit,
            (KeyCode::Enter, _) => {
                if self.input.is_empty() {
                    return Action::None;
                }
                match &self.input_mode {
                    InputMode::Chat if self.generating => Action::None,
                    InputMode::Chat => Action::Send(std::mem::take(&mut self.input)),
                    InputMode::Message(id) => {
                        let id = id.clone();
                        let text = std::mem::take(&mut self.input);
                        self.input_mode = InputMode::Chat;
                        Action::SendToAgent(id, text)
                    }
                    InputMode::Inject(id) => {
                        let id = id.clone();
                        let text = std::mem::take(&mut self.input);
                        self.input_mode = InputMode::Chat;
                        Action::InjectContext(id, text)
                    }
                    InputMode::SetKey(provider) => {
                        let provider = provider.clone();
                        let text = std::mem::take(&mut self.input);
                        self.input_mode = InputMode::Chat;
                        Action::SetProviderKey(provider, text)
                    }
                    InputMode::AddProviderName => {
                        let name = std::mem::take(&mut self.input);
                        self.input_mode = InputMode::AddProviderKind { name };
                        Action::None
                    }
                    InputMode::AddProviderKind { name } => {
                        let name = name.clone();
                        let kind_input = std::mem::take(&mut self.input);
                        let kind = if kind_input.is_empty() {
                            "openai-compatible".to_string()
                        } else {
                            kind_input
                        };
                        self.input_mode = InputMode::AddProviderBaseUrl { name, kind };
                        Action::None
                    }
                    InputMode::AddProviderBaseUrl { name, kind } => {
                        let name = name.clone();
                        let kind = kind.clone();
                        let base_url = std::mem::take(&mut self.input);
                        self.input_mode = InputMode::Chat;
                        Action::AddProvider {
                            name,
                            kind,
                            base_url,
                        }
                    }
                    InputMode::CommandPalette { .. } => Action::None,
                }
            }
            (KeyCode::Esc, _) => {
                if self.input_mode != InputMode::Chat {
                    self.input_mode = InputMode::Chat;
                    self.input.clear();
                    Action::None
                } else if self.generating {
                    Action::Cancel
                } else {
                    Action::None
                }
            }
            (KeyCode::Backspace, _) => {
                self.input.pop();
                Action::None
            }
            (KeyCode::PageUp, _) => {
                self.follow = false;
                self.scroll = self.scroll.saturating_add(10);
                Action::None
            }
            (KeyCode::PageDown, _) => {
                self.scroll = self.scroll.saturating_sub(10);
                if self.scroll == 0 {
                    self.follow = true;
                }
                Action::None
            }
            (KeyCode::End, _) => {
                self.follow = true;
                self.scroll = 0;
                Action::None
            }
            (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                self.input.push(c);
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_graph_key(&mut self, key: KeyEvent) -> Action {
        // Se está em modo input (Message/Inject), deixa o handler geral tratar Esc/Enter/texto
        if self.input_mode != InputMode::Chat {
            return match (key.code, key.modifiers) {
                (KeyCode::Esc, _) => {
                    self.input_mode = InputMode::Chat;
                    self.input.clear();
                    Action::None
                }
                (KeyCode::Enter, _) => {
                    if self.input.is_empty() {
                        return Action::None;
                    }
                    match std::mem::replace(&mut self.input_mode, InputMode::Chat) {
                        InputMode::Message(id) => {
                            let text = std::mem::take(&mut self.input);
                            Action::SendToAgent(id, text)
                        }
                        InputMode::Inject(id) => {
                            let text = std::mem::take(&mut self.input);
                            Action::InjectContext(id, text)
                        }
                        other => {
                            self.input_mode = other;
                            Action::None
                        }
                    }
                }
                (KeyCode::Backspace, _) => {
                    self.input.pop();
                    Action::None
                }
                (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                    self.input.push(c);
                    Action::None
                }
                _ => Action::None,
            };
        }
        let n = self.graph_nodes.len();
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.screen = Screen::Chat;
                Action::None
            }
            (KeyCode::Up, _) | (KeyCode::Char('k'), _) => {
                if n > 0 {
                    self.graph_selected = (self.graph_selected + n - 1) % n;
                }
                Action::None
            }
            (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                if n > 0 {
                    self.graph_selected = (self.graph_selected + 1) % n;
                }
                Action::None
            }
            (KeyCode::Left, _) => {
                self.graph_offset.0 = self.graph_offset.0.saturating_sub(2);
                Action::None
            }
            (KeyCode::Right, _) => {
                self.graph_offset.0 = self.graph_offset.0.saturating_add(2);
                Action::None
            }
            // vim-style: h/l movem seleção (mesmo comportamento de up/down, com wrap)
            (KeyCode::Char('h'), _) => {
                if n > 0 {
                    self.graph_selected = (self.graph_selected + n - 1) % n;
                }
                Action::None
            }
            (KeyCode::Char('l'), _) => {
                if n > 0 {
                    self.graph_selected = (self.graph_selected + 1) % n;
                }
                Action::None
            }
            (KeyCode::Char('m'), _) => {
                if let Some(agent) = self.graph_nodes.get(self.graph_selected) {
                    self.input_mode = InputMode::Message(agent.id.clone());
                    self.input.clear();
                    self.screen = Screen::Chat;
                }
                Action::None
            }
            (KeyCode::Char('i'), _) => {
                if let Some(agent) = self.graph_nodes.get(self.graph_selected) {
                    self.input_mode = InputMode::Inject(agent.id.clone());
                    self.input.clear();
                    self.screen = Screen::Chat;
                }
                Action::None
            }
            (KeyCode::Char('x'), _) => {
                if let Some(agent) = self.graph_nodes.get(self.graph_selected) {
                    return Action::Interrupt(agent.id.clone());
                }
                Action::None
            }
            (KeyCode::Enter, _) => {
                self.screen = Screen::Chat;
                Action::None
            }
            _ => Action::None,
        }
    }

    fn handle_sessions_key(&mut self, key: KeyEvent) -> Action {
        let n = self.sessions.len();
        match (key.code, key.modifiers) {
            (KeyCode::Esc, _) => {
                self.screen = Screen::Chat;
                Action::None
            }
            (KeyCode::Up, _) => {
                if n > 0 && self.sessions_selected > 0 {
                    self.sessions_selected -= 1;
                }
                Action::None
            }
            (KeyCode::Down, _) => {
                if n > 0 && self.sessions_selected + 1 < n {
                    self.sessions_selected += 1;
                }
                Action::None
            }
            (KeyCode::Enter, _) => {
                if let Some(s) = self.sessions.get(self.sessions_selected) {
                    self.screen = Screen::Chat;
                    Action::ResumeSession(s.id.clone())
                } else {
                    Action::None
                }
            }
            _ => Action::None,
        }
    }

    fn handle_providers_key(&mut self, key: KeyEvent) -> Action {
        // form/input modes: Enter avança/submete, Esc cancela, demais teclas editam
        if self.input_mode != InputMode::Chat {
            return match (key.code, key.modifiers) {
                (KeyCode::Enter, _) => {
                    let mode = std::mem::replace(&mut self.input_mode, InputMode::Chat);
                    match mode {
                        InputMode::SetKey(provider) => {
                            let text = std::mem::take(&mut self.input);
                            Action::SetProviderKey(provider, text)
                        }
                        InputMode::AddProviderName => {
                            let name = std::mem::take(&mut self.input);
                            if name.is_empty() {
                                Action::None
                            } else {
                                self.input_mode = InputMode::AddProviderKind { name };
                                Action::None
                            }
                        }
                        InputMode::AddProviderKind { name } => {
                            let kind_input = std::mem::take(&mut self.input);
                            let kind = if kind_input.is_empty() {
                                "openai-compatible".to_string()
                            } else {
                                kind_input
                            };
                            self.input_mode = InputMode::AddProviderBaseUrl { name, kind };
                            Action::None
                        }
                        InputMode::AddProviderBaseUrl { name, kind } => {
                            let base_url = std::mem::take(&mut self.input);
                            Action::AddProvider {
                                name,
                                kind,
                                base_url,
                            }
                        }
                        other => {
                            self.input_mode = other;
                            Action::None
                        }
                    }
                }
                (KeyCode::Esc, _) => {
                    self.input_mode = InputMode::Chat;
                    self.input.clear();
                    Action::None
                }
                (KeyCode::Backspace, _) => {
                    self.input.pop();
                    Action::None
                }
                (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                    self.input.push(c);
                    Action::None
                }
                _ => Action::None,
            };
        }
        let n = self.providers.len();
        match (key.code, key.modifiers) {
            (KeyCode::Enter, _) => self
                .providers
                .get(self.providers_selected)
                .map(|p| {
                    // Default model per provider (can be overridden by user later)
                    let default_model = match p.kind.as_str() {
                        "anthropic" => "claude-sonnet-4-5",
                        "openai" => "gpt-4o",
                        "google" => "gemini-2.5-pro",
                        "nim" => "nemotron-3-ultra",
                        "openai-compatible" => "default",
                        _ => "default",
                    };
                    match ModelAlias::parse(&format!("{}/{}", p.id, default_model)) {
                        Ok(alias) => Action::SetActiveModel(alias),
                        Err(_) => Action::None,
                    }
                })
                .unwrap_or(Action::None),
            (KeyCode::Esc, _) => {
                self.screen = Screen::Chat;
                Action::None
            }
            (KeyCode::Down, _) | (KeyCode::Char('j'), _) => {
                if n > 0 {
                    self.providers_selected = (self.providers_selected + 1) % n;
                }
                Action::None
            }
            (KeyCode::Up, _) => {
                if n > 0 {
                    self.providers_selected = (self.providers_selected + n - 1) % n;
                }
                Action::None
            }
            (KeyCode::Char('k'), _) => self
                .providers
                .get(self.providers_selected)
                .map(|p| {
                    self.input_mode = InputMode::SetKey(p.id.clone());
                    self.input.clear();
                    Action::None
                })
                .unwrap_or(Action::None),
            (KeyCode::Char('a'), _) => {
                self.input_mode = InputMode::AddProviderName;
                self.input.clear();
                Action::None
            }
            (KeyCode::Char('t'), _) => self
                .providers
                .get(self.providers_selected)
                .map(|p| Action::TestConnection(p.id.clone()))
                .unwrap_or(Action::None),
            (KeyCode::Char('d'), _) => self
                .providers
                .get(self.providers_selected)
                .map(|p| Action::RemoveProvider(p.id.clone()))
                .unwrap_or(Action::None),
            _ => Action::None,
        }
    }

    fn handle_command_palette_key(&mut self, key: KeyEvent) -> Action {
        // Command palette input mode: Enter selects, Esc closes, typing filters
        if let InputMode::CommandPalette { query } = &mut self.input_mode {
            let mut query = query.clone(); // borrow checker workaround
            match (key.code, key.modifiers) {
                (KeyCode::Esc, _) => {
                    self.screen = Screen::Chat;
                    self.input_mode = InputMode::Chat;
                    Action::None
                }
                (KeyCode::Enter, _) => {
                    // Check if it's a colon command
                    if let Some(cmd) = query.strip_prefix(':') {
                        match cmd {
                            "export" => {
                                self.screen = Screen::Chat;
                                self.input_mode = InputMode::Chat;
                                Action::ExportProvidersTemplate("./providers.template.toml".to_string())
                            }
                            "help" => {
                                self.screen = Screen::Help;
                                self.input_mode = InputMode::Chat;
                                Action::None
                            }
                            _ => {
                                // Unknown command, stay in palette
                                Action::None
                            }
                        }
                    } else {
                        // Fuzzy match providers and select first match
                        let filtered: Vec<_> = self.providers.iter()
                            .filter(|p| {
                                let haystack = format!("{} {}", p.id, p.kind).to_lowercase();
                                haystack.contains(&query.to_lowercase())
                            })
                            .collect();
                        if let Some(provider) = filtered.first() {
                            // Default model per provider
                            let default_model = match provider.kind.as_str() {
                                "anthropic" => "claude-sonnet-4-5",
                                "openai" => "gpt-4o",
                                "google" => "gemini-2.5-pro",
                                "nim" => "nemotron-3-ultra",
                                "openai-compatible" => "default",
                                _ => "default",
                            };
                            let parsed = ModelAlias::parse(&format!("{}/{}", provider.id, default_model));
                            match parsed {
                                Ok(alias) => {
                                    self.screen = Screen::Chat;
                                    self.input_mode = InputMode::Chat;
                                    Action::SetActiveModel(alias)
                                }
                                Err(_) => Action::None,
                            }
                        } else {
                            Action::None
                        }
                    }
                }
                (KeyCode::Backspace, _) => {
                    query.pop();
                    self.input_mode = InputMode::CommandPalette { query };
                    Action::None
                }
                (KeyCode::Char(c), KeyModifiers::NONE | KeyModifiers::SHIFT) => {
                    query.push(c);
                    self.input_mode = InputMode::CommandPalette { query };
                    Action::None
                }
                _ => Action::None,
            }
        } else {
            Action::None
        }
    }
}

fn next_screen(s: Screen) -> Screen {
    match s {
        Screen::Chat => Screen::Graph,
        Screen::Graph => Screen::Sessions,
        Screen::Sessions => Screen::Providers,
        Screen::Providers => Screen::Help,
        Screen::Help => Screen::Chat,
        // CommandPalette não entra no ciclo de Tab — abre via Ctrl+P / ':'
        Screen::CommandPalette => Screen::Chat,
    }
}

fn prev_screen(s: Screen) -> Screen {
    // ciclo principal tem 5 telas (CommandPalette fora) — prev = next^4
    next_screen(next_screen(next_screen(next_screen(s))))
}

fn screen_for_f(n: u8) -> Option<Screen> {
    match n {
        1 => Some(Screen::Chat),
        2 => Some(Screen::Graph),
        3 => Some(Screen::Help),
        4 => Some(Screen::Sessions),
        5 => Some(Screen::Providers),
        6 => Some(Screen::CommandPalette),
        _ => None,
    }
}
