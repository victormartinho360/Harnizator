//! Red tests for harness-core domain types (Wave 0).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harness_core::*;

#[test]
fn model_alias_parses_provider_and_model() {
    let alias = ModelAlias::parse("anthropic/claude-sonnet-4-5").unwrap();
    assert_eq!(alias.provider(), "anthropic");
    assert_eq!(alias.model(), "claude-sonnet-4-5");
}

#[test]
fn model_alias_requires_slash() {
    let err = ModelAlias::parse("claude-sonnet-4-5").unwrap_err();
    assert!(matches!(err, ModelAliasError::MissingSeparator));
}

#[test]
fn model_alias_supports_model_names_containing_extra_slash() {
    // openrouter-style: provider = first segment, model = the rest
    let alias = ModelAlias::parse("openrouter/anthropic/claude-3").unwrap();
    assert_eq!(alias.provider(), "openrouter");
    assert_eq!(alias.model(), "anthropic/claude-3");
}

#[test]
fn message_user_text_constructor() {
    let msg = Message::user("hello");
    assert!(matches!(msg.role, Role::User));
    assert_eq!(msg.text(), "hello");
}

#[test]
fn event_serializes_roundtrip() {
    let events = vec![
        Event::AssistantDelta { text: "hi".into() },
        Event::AgentSpawned {
            agent: AgentId::new("a1"),
            parent: None,
            label: "root".into(),
        },
        Event::Error {
            message: "boom".into(),
        },
    ];
    let json = serde_json::to_string(&events).unwrap();
    let back: Vec<Event> = serde_json::from_str(&json).unwrap();
    assert_eq!(back, events);
}

#[test]
fn snapshot_event_debug_format_is_stable() {
    let ev = Event::AssistantDelta {
        text: "hello".into(),
    };
    insta::assert_debug_snapshot!(ev);
}
