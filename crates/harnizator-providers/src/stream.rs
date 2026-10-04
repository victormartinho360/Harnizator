//! Constrói um StreamResult a partir de um response SSE do reqwest.

use std::collections::VecDeque;

use futures::{StreamExt, stream};

use crate::provider::{ProviderError, StreamChunk, StreamResult};
use crate::sse::{SseEvent, SseParser};

/// Estado interno do stream.
struct State {
    bytes: futures::stream::BoxStream<'static, Result<bytes::Bytes, reqwest::Error>>,
    parser: SseParser,
    pending: VecDeque<Result<StreamChunk, ProviderError>>,
    finished: bool,
    saw_stop: bool,
    map: Box<dyn FnMut(&SseEvent) -> MapOutcome + Send>,
}

/// Resultado do mapeamento de um evento SSE por provider.
pub struct MapOutcome {
    pub chunks: Vec<Result<StreamChunk, ProviderError>>,
    pub is_stop: bool,
}

/// Transforma um response HTTP SSE em stream de chunks normalizados.
///
/// Se o stream terminar sem evento de parada, emite
/// `Err(Stream("stream ended without stop event"))` (spec/03).
pub fn sse_response_stream(
    resp: reqwest::Response,
    mut map: impl FnMut(&SseEvent) -> MapOutcome + Send + 'static,
) -> StreamResult {
    let state = State {
        bytes: Box::pin(resp.bytes_stream()),
        parser: SseParser::new(),
        pending: VecDeque::new(),
        finished: false,
        saw_stop: false,
        map: Box::new(move |e| map(e)),
    };

    Box::pin(stream::unfold(state, |mut st| async move {
        loop {
            if let Some(item) = st.pending.pop_front() {
                return Some((item, st));
            }
            if st.finished {
                if !st.saw_stop {
                    st.saw_stop = true; // emite o erro uma única vez
                    return Some((
                        Err(ProviderError::Stream(
                            "stream ended without stop event".to_string(),
                        )),
                        st,
                    ));
                }
                return None;
            }
            match st.bytes.next().await {
                Some(Ok(chunk)) => {
                    for ev in st.parser.push(&chunk) {
                        let outcome = (st.map)(&ev);
                        if outcome.is_stop {
                            st.saw_stop = true;
                        }
                        st.pending.extend(outcome.chunks);
                    }
                }
                Some(Err(e)) => {
                    st.finished = true;
                    st.pending
                        .push_back(Err(ProviderError::Stream(e.to_string())));
                }
                None => st.finished = true,
            }
        }
    }))
}
