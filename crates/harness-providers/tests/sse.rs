//! Red tests: parser SSE incremental (puro, ver spec/03).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use harness_providers::sse::{SseEvent, SseParser};
use proptest::prelude::*;

#[test]
fn parses_single_data_event() {
    let mut p = SseParser::new();
    let events = p.push(b"data: hello\n\n");
    assert_eq!(
        events,
        vec![SseEvent {
            event: None,
            data: "hello".into()
        }]
    );
}

#[test]
fn parses_named_event() {
    let mut p = SseParser::new();
    let events = p.push(b"event: message_start\ndata: {}\n\n");
    assert_eq!(
        events,
        vec![SseEvent {
            event: Some("message_start".into()),
            data: "{}".into()
        }]
    );
}

#[test]
fn joins_multiline_data() {
    let mut p = SseParser::new();
    let events = p.push(b"data: line1\ndata: line2\n\n");
    assert_eq!(events[0].data, "line1\nline2");
}

#[test]
fn ignores_comments() {
    let mut p = SseParser::new();
    let events = p.push(b": keep-alive\ndata: x\n\n");
    assert_eq!(events.len(), 1);
}

#[test]
fn buffers_partial_line_across_pushes() {
    let mut p = SseParser::new();
    assert!(p.push(b"data: hel").is_empty());
    assert!(p.push(b"lo\n").is_empty());
    let events = p.push(b"\n");
    assert_eq!(events[0].data, "hello");
}

#[test]
fn reset_between_events() {
    let mut p = SseParser::new();
    let _ = p.push(b"event: a\ndata: 1\n\n");
    let events = p.push(b"data: 2\n\n");
    assert_eq!(
        events,
        vec![SseEvent {
            event: None,
            data: "2".into()
        }]
    );
}

proptest! {
    /// Chunking arbitrário de bytes nunca muda os eventos produzidos (spec/03).
    #[test]
    fn arbitrary_chunking_is_transparent(
        payload in "[a-z0-9: \\n]{2,64}",
        cuts in proptest::collection::vec(any::<proptest::sample::Index>(), 1..8),
    ) {
        let mut src: Vec<u8> = payload.into_bytes();
        // garante terminação do último evento
        src.extend_from_slice(b"\n\n");

        let mut whole = SseParser::new();
        let expected: Vec<SseEvent> = whole.push(&src);

        // partição do buffer em cortes determinísticos
        let mut points: Vec<usize> = cuts.iter().map(|i| i.index(src.len())).collect();
        points.sort_unstable();
        points.dedup();
        let mut pieces: Vec<&[u8]> = vec![];
        let mut start = 0;
        for p in points {
            pieces.push(&src[start..p]);
            start = p;
        }
        pieces.push(&src[start..]);

        let mut chunked = SseParser::new();
        let mut actual: Vec<SseEvent> = vec![];
        for piece in pieces {
            actual.extend(chunked.push(piece));
        }

        prop_assert_eq!(expected, actual);
    }
}
