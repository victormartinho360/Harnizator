//! Flaglist: padrões glob simples com `*` para comandos bash (spec/04).

/// Política de comandos sinalizados como seguros.
#[derive(Debug, Clone, Default)]
pub struct FlagPolicy {
    patterns: Vec<String>,
}

impl FlagPolicy {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adiciona um grupo nomeado de padrões (do config `[sandbox.flags]`).
    pub fn add_group(&mut self, _group: &str, patterns: &[&str]) {
        self.patterns.extend(patterns.iter().map(|s| s.to_string()));
    }

    /// O comando é considerado "flagged" (auto-aprovável em `OnlyFlagged`).
    pub fn is_flagged(&self, command: &str) -> bool {
        let cmd = command.trim();
        self.patterns.iter().any(|p| glob_match(p, cmd))
    }
}

/// Glob mínimo: `*` casa 0+ chars; casamento ancorado no texto inteiro.
pub fn glob_match(pattern: &str, text: &str) -> bool {
    let p = pattern.trim();
    let t = text.trim();
    if !p.contains('*') {
        return p == t;
    }
    let parts: Vec<&str> = p.split('*').collect();
    let mut rest = t;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !rest.starts_with(part) {
                return false;
            }
            rest = &rest[part.len()..];
        } else if i == parts.len() - 1 && !p.ends_with('*') {
            return rest.ends_with(part);
        } else if let Some(pos) = rest.find(part) {
            rest = &rest[pos + part.len()..];
        } else {
            return false;
        }
    }
    true
}
