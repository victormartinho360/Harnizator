//! Tools builtin (spec/04): read_file, write_file, edit_file, bash, glob, grep.

use serde_json::json;

use crate::{Risk, Tool, ToolCtx, ToolError, ToolOutput};

fn arg_str<'a>(args: &'a serde_json::Value, key: &str) -> Result<&'a str, ToolError> {
    args.get(key)
        .and_then(|v| v.as_str())
        .ok_or_else(|| ToolError::InvalidArgs(format!("missing string arg `{key}`")))
}

fn schema_object(props: serde_json::Value, required: &[&str]) -> serde_json::Value {
    json!({
        "type": "object",
        "properties": props,
        "required": required,
    })
}

/// `read_file` — leitura com offset/limit e sinalização de truncamento.
pub struct ReadFileTool;

#[async_trait::async_trait]
impl Tool for ReadFileTool {
    fn name(&self) -> &'static str {
        "read_file"
    }
    fn description(&self) -> &'static str {
        "Read a UTF-8 file with optional line offset/limit (default: first 2000 lines)."
    }
    fn schema(&self) -> serde_json::Value {
        schema_object(
            json!({
                "path": {"type": "string"},
                "offset": {"type": "integer", "description": "1-based first line"},
                "limit": {"type": "integer", "description": "max lines, default 2000"},
            }),
            &["path"],
        )
    }
    fn risk(&self, _args: &serde_json::Value) -> Risk {
        Risk::Read
    }
    fn path_args(&self, args: &serde_json::Value) -> Vec<String> {
        vec![arg_str(args, "path").unwrap_or_default().to_string()]
    }
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
    ) -> Result<ToolOutput, ToolError> {
        let path = ctx.resolve(arg_str(&args, "path")?)?;
        let content = std::fs::read_to_string(&path).map_err(|e| ToolError::Io(e.to_string()))?;
        let offset = args["offset"].as_u64().unwrap_or(1).max(1) as usize;
        let limit = args["limit"].as_u64().unwrap_or(2000) as usize;
        let lines: Vec<&str> = content.lines().collect();
        let start = offset.saturating_sub(1).min(lines.len());
        let end = (start + limit).min(lines.len());
        let truncated = end < lines.len();
        Ok(ToolOutput {
            content: lines[start..end].join("\n"),
            truncated,
            exit_code: None,
        })
    }
}

/// `write_file` — cria/sobrescreve arquivo dentro do workspace.
pub struct WriteFileTool;

#[async_trait::async_trait]
impl Tool for WriteFileTool {
    fn name(&self) -> &'static str {
        "write_file"
    }
    fn description(&self) -> &'static str {
        "Create or overwrite a file inside the workspace."
    }
    fn schema(&self) -> serde_json::Value {
        schema_object(
            json!({
                "path": {"type": "string"},
                "content": {"type": "string"},
            }),
            &["path", "content"],
        )
    }
    fn risk(&self, _args: &serde_json::Value) -> Risk {
        Risk::Write
    }
    fn path_args(&self, args: &serde_json::Value) -> Vec<String> {
        vec![arg_str(args, "path").unwrap_or_default().to_string()]
    }
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
    ) -> Result<ToolOutput, ToolError> {
        let path = ctx.resolve(arg_str(&args, "path")?)?;
        let content = arg_str(&args, "content")?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ToolError::Io(e.to_string()))?;
        }
        std::fs::write(&path, content).map_err(|e| ToolError::Io(e.to_string()))?;
        Ok(ToolOutput::text(format!("wrote {} bytes", content.len())))
    }
}

/// `edit_file` — substituição literal old→new com match único.
pub struct EditFileTool;

#[async_trait::async_trait]
impl Tool for EditFileTool {
    fn name(&self) -> &'static str {
        "edit_file"
    }
    fn description(&self) -> &'static str {
        "Replace a literal string in a file; must match exactly once unless replace_all."
    }
    fn schema(&self) -> serde_json::Value {
        schema_object(
            json!({
                "path": {"type": "string"},
                "old_string": {"type": "string"},
                "new_string": {"type": "string"},
                "replace_all": {"type": "boolean"},
            }),
            &["path", "old_string", "new_string"],
        )
    }
    fn risk(&self, _args: &serde_json::Value) -> Risk {
        Risk::Write
    }
    fn path_args(&self, args: &serde_json::Value) -> Vec<String> {
        vec![arg_str(args, "path").unwrap_or_default().to_string()]
    }
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
    ) -> Result<ToolOutput, ToolError> {
        let path = ctx.resolve(arg_str(&args, "path")?)?;
        let old = arg_str(&args, "old_string")?;
        let new = arg_str(&args, "new_string")?;
        let replace_all = args["replace_all"].as_bool().unwrap_or(false);
        let content = std::fs::read_to_string(&path).map_err(|e| ToolError::Io(e.to_string()))?;
        let matches = content.matches(old).count();
        if matches == 0 {
            return Err(ToolError::NoMatch);
        }
        if matches > 1 && !replace_all {
            return Err(ToolError::MultipleMatches(matches));
        }
        let updated = if replace_all {
            content.replace(old, new)
        } else {
            content.replacen(old, new, 1)
        };
        std::fs::write(&path, &updated).map_err(|e| ToolError::Io(e.to_string()))?;
        Ok(ToolOutput::text(format!("replaced {matches} match(es)")))
    }
}

/// `glob` — encontra arquivos por padrão, paths relativos ao root.
pub struct GlobTool;

#[async_trait::async_trait]
impl Tool for GlobTool {
    fn name(&self) -> &'static str {
        "glob"
    }
    fn description(&self) -> &'static str {
        "Find files by glob pattern (relative to workspace root)."
    }
    fn schema(&self) -> serde_json::Value {
        schema_object(json!({"pattern": {"type": "string"}}), &["pattern"])
    }
    fn risk(&self, _args: &serde_json::Value) -> Risk {
        Risk::Read
    }
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
    ) -> Result<ToolOutput, ToolError> {
        let pattern = arg_str(&args, "pattern")?;
        if pattern.starts_with('/') || pattern.split('/').any(|seg| seg == "..") {
            return Err(ToolError::PathEscape(pattern.to_string()));
        }
        let full = format!("{}/{}", ctx.root().display(), pattern);
        let mut matches = Vec::new();
        for entry in glob::glob(&full).map_err(|e| ToolError::InvalidArgs(e.to_string()))? {
            let path = entry.map_err(|e| ToolError::Io(e.to_string()))?;
            if let Ok(rel) = path.strip_prefix(ctx.root()) {
                matches.push(rel.display().to_string());
            }
        }
        matches.sort();
        let truncated = matches.len() > 1000;
        matches.truncate(1000);
        Ok(ToolOutput {
            content: matches.join("\n"),
            truncated,
            exit_code: None,
        })
    }
}

/// `grep` — busca regex recursiva, saída `path:line: texto` (máx 100).
pub struct GrepTool;

#[async_trait::async_trait]
impl Tool for GrepTool {
    fn name(&self) -> &'static str {
        "grep"
    }
    fn description(&self) -> &'static str {
        "Search file contents with a regex; outputs `path:line: text` (max 100 matches)."
    }
    fn schema(&self) -> serde_json::Value {
        schema_object(json!({"pattern": {"type": "string"}}), &["pattern"])
    }
    fn risk(&self, _args: &serde_json::Value) -> Risk {
        Risk::Read
    }
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
    ) -> Result<ToolOutput, ToolError> {
        let pattern = arg_str(&args, "pattern")?;
        let re = regex::Regex::new(pattern).map_err(|e| ToolError::InvalidArgs(e.to_string()))?;
        let mut matches = Vec::new();
        walk(ctx.root(), ctx.root(), &re, &mut matches);
        let truncated = matches.len() >= 100;
        Ok(ToolOutput {
            content: matches.join("\n"),
            truncated,
            exit_code: None,
        })
    }
}

fn walk(dir: &std::path::Path, root: &std::path::Path, re: &regex::Regex, out: &mut Vec<String>) {
    if out.len() >= 100 {
        return;
    }
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        if out.len() >= 100 {
            return;
        }
        let path = entry.path();
        if path.is_dir() {
            walk(&path, root, re, out);
        } else if let Ok(content) = std::fs::read_to_string(&path) {
            for (i, line) in content.lines().enumerate() {
                if out.len() >= 100 {
                    return;
                }
                if re.is_match(line) {
                    let rel = path.strip_prefix(root).unwrap_or(&path);
                    out.push(format!("{}:{}: {}", rel.display(), i + 1, line));
                }
            }
        }
    }
}

/// `bash` — comando no root do workspace, timeout e tail truncation.
pub struct BashTool;

const MAX_OUTPUT_BYTES: usize = 16_000;
const DEFAULT_TIMEOUT_MS: u64 = 30_000;

#[async_trait::async_trait]
impl Tool for BashTool {
    fn name(&self) -> &'static str {
        "bash"
    }
    fn description(&self) -> &'static str {
        "Run a shell command in the workspace root. Output tail-truncated to 16KB. Default timeout 30s (arg timeout_ms)."
    }
    fn schema(&self) -> serde_json::Value {
        schema_object(
            json!({
                "command": {"type": "string"},
                "timeout_ms": {"type": "integer"},
            }),
            &["command"],
        )
    }
    fn risk(&self, _args: &serde_json::Value) -> Risk {
        Risk::Execute
    }
    async fn execute(
        &self,
        args: serde_json::Value,
        ctx: &ToolCtx,
    ) -> Result<ToolOutput, ToolError> {
        let command = arg_str(&args, "command")?;
        let timeout_ms = args["timeout_ms"].as_u64().unwrap_or(DEFAULT_TIMEOUT_MS);
        let child = tokio::process::Command::new("/bin/sh")
            .arg("-c")
            .arg(command)
            .current_dir(ctx.root())
            .kill_on_drop(true)
            .output();
        let result =
            tokio::time::timeout(std::time::Duration::from_millis(timeout_ms), child).await;
        let output = match result {
            Err(_) => {
                return Ok(ToolOutput {
                    content: format!("error: command timed out after {timeout_ms}ms"),
                    truncated: false,
                    exit_code: Some(-1),
                });
            }
            Ok(Err(e)) => return Err(ToolError::Io(e.to_string())),
            Ok(Ok(o)) => o,
        };
        let mut text = String::from_utf8_lossy(&output.stdout).into_owned();
        let stderr = String::from_utf8_lossy(&output.stderr);
        if !stderr.is_empty() {
            text.push_str("\n[stderr]\n");
            text.push_str(&stderr);
        }
        let truncated = text.len() > MAX_OUTPUT_BYTES;
        if truncated {
            let tail_start = text.len() - MAX_OUTPUT_BYTES;
            text = text[floor_char_boundary(&text, tail_start)..].to_string();
        }
        Ok(ToolOutput {
            content: text,
            truncated,
            exit_code: output.status.code(),
        })
    }
}

fn floor_char_boundary(s: &str, mut i: usize) -> usize {
    while i > 0 && !s.is_char_boundary(i) {
        i -= 1;
    }
    i
}
