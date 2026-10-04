//! Parser SSE incremental e puro (spec/03).
//!
//! Bytes entram por `push()`; eventos completos (terminados em linha
//! em branco) saem. Tolera chunking arbitrário de bytes.

/// Um evento SSE completo.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SseEvent {
    /// Nome do campo `event:`, se presente.
    pub event: Option<String>,
    /// Linhas `data:` unidas por `\n`.
    pub data: String,
}

/// Parser incremental de Server-Sent Events.
#[derive(Debug, Default)]
pub struct SseParser {
    buf: Vec<u8>,
    cur_event: Option<String>,
    cur_data: Vec<String>,
}

impl SseParser {
    pub fn new() -> Self {
        Self::default()
    }

    /// Alimenta bytes e devolve os eventos completados por este push.
    pub fn push(&mut self, bytes: &[u8]) -> Vec<SseEvent> {
        self.buf.extend_from_slice(bytes);
        let mut out = Vec::new();
        while let Some(pos) = self.buf.iter().position(|b| *b == b'\n') {
            let mut line: Vec<u8> = self.buf.drain(..=pos).collect();
            line.pop(); // \n
            if line.last() == Some(&b'\r') {
                line.pop();
            }
            // linhas são UTF-8 por contrato; bytes inválidos viram replacement
            let line = String::from_utf8_lossy(&line).into_owned();
            if let Some(ev) = self.process_line(&line) {
                out.push(ev);
            }
        }
        out
    }

    fn process_line(&mut self, line: &str) -> Option<SseEvent> {
        if line.is_empty() {
            return self.dispatch();
        }
        if line.starts_with(':') {
            return None; // comentário / keep-alive
        }
        let (field, value) = match line.split_once(':') {
            Some((f, v)) => (f, v.strip_prefix(' ').unwrap_or(v)),
            None => (line, ""),
        };
        match field {
            "data" => self.cur_data.push(value.to_string()),
            "event" => self.cur_event = Some(value.to_string()),
            _ => {} // id, retry: ignorados
        }
        None
    }

    fn dispatch(&mut self) -> Option<SseEvent> {
        if self.cur_data.is_empty() && self.cur_event.is_none() {
            return None;
        }
        let event = self.cur_event.take();
        let data = std::mem::take(&mut self.cur_data).join("\n");
        Some(SseEvent { event, data })
    }
}
