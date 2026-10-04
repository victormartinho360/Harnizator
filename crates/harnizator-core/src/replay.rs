//! Replay: reconstrói o histórico de mensagens de um agente a partir do
//! event log persistido (spec/07 — resume).

use crate::events::Event;
use crate::message::{ContentBlock, Message};
use crate::store_port::StoredEvent;
use crate::{AgentId, Role};

/// Reconstrói a conversa de um agente (deltas contíguos viram uma mensagem
/// só; tool_use/tool_result remontados do audit trail).
pub fn replay_agent(events: &[StoredEvent], agent: &AgentId) -> Vec<Message> {
    let mut messages: Vec<Message> = Vec::new();
    let mut text_buf = String::new();

    let flush_assistant =
        |messages: &mut Vec<Message>, buf: &mut String, extra: &mut Vec<ContentBlock>| {
            if !buf.is_empty() || !extra.is_empty() {
                let mut blocks: Vec<ContentBlock> = Vec::new();
                if !buf.is_empty() {
                    blocks.push(ContentBlock::Text {
                        text: std::mem::take(buf),
                    });
                }
                blocks.append(extra);
                messages.push(Message {
                    role: Role::Assistant,
                    content: blocks,
                });
            }
        };
    let mut pending_tool_uses: Vec<ContentBlock> = Vec::new();

    for stored in events {
        if &stored.agent != agent {
            continue;
        }
        match &stored.event {
            Event::AssistantDelta { text, .. } => text_buf.push_str(text),
            Event::ToolCallRequested { id, name, args, .. } => {
                pending_tool_uses.push(ContentBlock::ToolUse {
                    id: id.clone(),
                    name: name.clone(),
                    input: args.clone(),
                });
            }
            Event::ToolCallApproved { .. } => {}
            Event::ToolCallDenied { id, reason, .. } => {
                flush_assistant(&mut messages, &mut text_buf, &mut pending_tool_uses);
                messages.push(Message {
                    role: Role::User,
                    content: vec![ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: format!("denied: {reason}"),
                        is_error: true,
                    }],
                });
            }
            Event::ToolCallCompleted {
                id,
                is_error,
                output,
                ..
            } => {
                flush_assistant(&mut messages, &mut text_buf, &mut pending_tool_uses);
                messages.push(Message {
                    role: Role::User,
                    content: vec![ContentBlock::ToolResult {
                        tool_use_id: id.clone(),
                        content: output.clone(),
                        is_error: *is_error,
                    }],
                });
            }
            Event::ContextInjected { text, .. } => {
                flush_assistant(&mut messages, &mut text_buf, &mut pending_tool_uses);
                messages.push(Message::user(format!("[context-injected] {text}")));
            }
            _ => {}
        }
    }
    flush_assistant(&mut messages, &mut text_buf, &mut pending_tool_uses);
    messages
}
