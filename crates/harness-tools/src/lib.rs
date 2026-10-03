//! harness-tools: tools builtin + sandbox policy (spec/04).

pub mod belt;
pub mod builtins;
pub mod flags;
pub mod sandbox;

use std::path::{Path, PathBuf};

pub use belt::Toolbelt;
pub use flags::FlagPolicy;
pub use sandbox::SandboxMode;

/// Contexto de execução de tools: workspace root canonicalizado.
#[derive(Debug, Clone)]
pub struct ToolCtx {
    root: PathBuf,
}

impl ToolCtx {
    pub fn new(root: &Path) -> Result<Self, ToolError> {
        let root = root
            .canonicalize()
            .map_err(|e| ToolError::Io(e.to_string()))?;
        Ok(Self { root })
    }

    pub fn root(&self) -> &Path {
        &self.root
    }

    /// Resolve um path de argumento dentro do root; escapadas (via `..`,
    /// absoluto fora do root ou symlink) → `PathEscape`.
    pub fn resolve(&self, arg_path: &str) -> Result<PathBuf, ToolError> {
        let arg = Path::new(arg_path);
        let joined = if arg.is_absolute() {
            arg.to_path_buf()
        } else {
            self.root.join(arg)
        };
        // canonicalize resolve symlinks; para alvos inexistentes (write_file),
        // canonicaliza o pai e recomputa o nome do arquivo
        let resolved = match joined.canonicalize() {
            Ok(p) => p,
            Err(_) => {
                let parent = joined
                    .parent()
                    .ok_or_else(|| ToolError::PathEscape(arg_path.to_string()))?
                    .canonicalize()
                    .map_err(|e| ToolError::Io(e.to_string()))?;
                let name = joined
                    .file_name()
                    .ok_or_else(|| ToolError::PathEscape(arg_path.to_string()))?;
                parent.join(name)
            }
        };
        if !resolved.starts_with(&self.root) {
            return Err(ToolError::PathEscape(arg_path.to_string()));
        }
        Ok(resolved)
    }
}

/// Categoria de risco de uma tool call (spec/04).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Risk {
    Read,
    Write,
    Execute,
}

/// Saída de uma tool.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ToolOutput {
    pub content: String,
    pub truncated: bool,
    pub exit_code: Option<i32>,
}

impl ToolOutput {
    pub fn text(content: String) -> Self {
        Self {
            content,
            truncated: false,
            exit_code: None,
        }
    }

    /// Se o resultado deve ser tratado como erro pelo LLM (tool_result is_error).
    pub fn is_error_flag(&self) -> bool {
        self.exit_code.is_some_and(|c| c != 0) || self.content.starts_with("error:")
    }
}

/// Erros de tool.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("io: {0}")]
    Io(String),
    #[error("path escapes the workspace root: {0}")]
    PathEscape(String),
    #[error("no match found for old_string")]
    NoMatch,
    #[error("old_string matches {0} times; it must be unique (or set replace_all)")]
    MultipleMatches(usize),
    #[error("invalid args: {0}")]
    InvalidArgs(String),
    #[error("unknown tool: {0}")]
    UnknownTool(String),
}

/// Tool builtin do harness.
#[async_trait::async_trait]
pub trait Tool: Send + Sync {
    fn name(&self) -> &'static str;
    fn description(&self) -> &'static str;
    fn schema(&self) -> serde_json::Value;
    /// Risco da invocação com esses args.
    fn risk(&self, args: &serde_json::Value) -> Risk;
    /// Args que são paths de arquivo (para checagem de escape no sandbox).
    fn path_args(&self, args: &serde_json::Value) -> Vec<String> {
        let _ = args;
        Vec::new()
    }
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
    ) -> Result<ToolOutput, ToolError>;
}
