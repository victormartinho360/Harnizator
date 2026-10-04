//! Red tests: mapeamento Anthropic SSE → StreamChunk (spec/03).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harnizator_providers::anthropic::AnthropicEvent;
use harnizator_providers::sse::{SseEvent, SseParser};

fn sse(event: &str, data: &str) -> SseEvent {
    let mut p = SseParser::new();
    let raw = format!("event: {event}\ndata: {data}\n\n");
    let events = p.push(raw.as_bytes());
    events.into_iter().next().unwrap()
}

#[test]
fn maps_message_start_with_input_tokens() {
    let ev = sse(
        "message_start",
        r#"{"type":"message_start","message":{"usage":{"input_tokens":10,"output_tokens":1}}}"#,
    );
    assert_eq!(
        AnthropicEvent::from_sse(&ev).unwrap(),
        AnthropicEvent::MessageStart { input_tokens: 10 }
    );
}

#[test]
fn maps_text_delta() {
    let ev = sse(
        "content_block_delta",
        r#"{"type":"content_block_delta","delta":{"type":"text_delta","text":"Hello"}}"#,
    );
    assert_eq!(
        AnthropicEvent::from_sse(&ev).unwrap(),
        AnthropicEvent::TextDelta("Hello".into())
    );
}

#[test]
fn maps_message_delta_output_tokens() {
    let ev = sse(
        "message_delta",
        r#"{"type":"message_delta","usage":{"output_tokens":5}}"#,
    );
    assert_eq!(
        AnthropicEvent::from_sse(&ev).unwrap(),
        AnthropicEvent::OutputUsage { output_tokens: 5 }
    );
}

#[test]
fn maps_message_stop() {
    let ev = sse("message_stop", r#"{"type":"message_stop"}"#);
    assert_eq!(
        AnthropicEvent::from_sse(&ev).unwrap(),
        AnthropicEvent::MessageStop
    );
}

#[test]
fn ignores_pings_and_content_block_start() {
    let ping = sse("ping", r#"{"type":"ping"}"#);
    assert_eq!(
        AnthropicEvent::from_sse(&ping).unwrap(),
        AnthropicEvent::Ignored
    );
    let cbs = sse(
        "content_block_start",
        r#"{"type":"content_block_start","content_block":{"type":"text","text":""}}"#,
    );
    assert_eq!(
        AnthropicEvent::from_sse(&cbs).unwrap(),
        AnthropicEvent::Ignored
    );
}

#[test]
fn full_fixture_stream_snapshot() {
    let fixture = std::fs::read_to_string(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/anthropic_text.sse"
    ))
    .unwrap();
    let mut p = SseParser::new();
    let mapped: Vec<AnthropicEvent> = p
        .push(fixture.as_bytes())
        .iter()
        .map(|e| AnthropicEvent::from_sse(e).unwrap())
        .collect();
    insta::assert_debug_snapshot!(mapped);
}
