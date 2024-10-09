use std::collections::HashMap;

use opentelemetry::{
    global::{self},
    propagation::Extractor,
    trace::{Span, SpanKind, TraceContextExt, Tracer},
    Context, KeyValue,
};

use crate::common::udpendpoint::UDPEndpoint;

pub struct ObjectSenderLogger {
    _cx: Context,
}

impl std::fmt::Debug for ObjectSenderLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectSenderLogger").finish()
    }
}

struct HeaderExtractor<'a>(pub &'a HashMap<String, String>);
impl Extractor for HeaderExtractor<'_> {
    fn get(&self, key: &str) -> Option<&str> {
        self.0.get(key).map(|s| s.as_str())
    }

    fn keys(&self) -> Vec<&str> {
        self.0.keys().map(|s| s.as_str()).collect()
    }
}

impl ObjectSenderLogger {
    fn extract_context_from_propagator(req: &HashMap<String, String>) -> Context {
        global::get_text_map_propagator(|propagator| propagator.extract(&HeaderExtractor(req)))
    }

    pub fn new(
        endpoint: &UDPEndpoint,
        tsi: u64,
        toi: u128,
        propagator: Option<&HashMap<String, String>>,
    ) -> Self {
        let tracer = global::tracer("FluteLogger");
        let name = match toi {
            0 => "FDT Transfer",
            _ => "Object Transfer",
        };

        let mut span;
        if let Some(propagator) = propagator {
            let parent_cx = Self::extract_context_from_propagator(propagator);
            span = tracer
                .span_builder(name)
                .with_kind(SpanKind::Producer)
                .start_with_context(&tracer, &parent_cx)
        } else {
            span = tracer
                .span_builder(name)
                .with_kind(SpanKind::Producer)
                .start(&tracer);
        }

        span.set_attribute(KeyValue::new(
            opentelemetry_semantic_conventions::attribute::NETWORK_TRANSPORT,
            "flute",
        ));

        span.set_attribute(KeyValue::new(
            opentelemetry_semantic_conventions::attribute::NETWORK_PEER_ADDRESS,
            endpoint.destination_group_address.clone(),
        ));

        span.set_attribute(KeyValue::new(
            opentelemetry_semantic_conventions::attribute::NETWORK_PEER_PORT,
            endpoint.port as i64,
        ));

        if let Some(source_address) = endpoint.source_address.as_ref() {
            span.set_attribute(KeyValue::new(
                opentelemetry_semantic_conventions::attribute::NETWORK_LOCAL_ADDRESS,
                source_address.clone(),
            ));
        }

        span.set_attribute(KeyValue::new("flute.toi", toi.to_string()));
        span.set_attribute(KeyValue::new("flute.tsi", tsi.to_string()));

        let cx = Context::current_with_span(span);
        Self { _cx: cx }
    }
}
