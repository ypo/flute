use std::collections::HashMap;

use opentelemetry::{
    global::{self, BoxedSpan},
    propagation::{Extractor, Injector},
    trace::{Span, SpanKind, Status, TraceContextExt, Tracer},
    Context, KeyValue,
};

use crate::common::udpendpoint::UDPEndpoint;

pub struct ObjectReceiverLogger {
    cx: Context,
}

impl std::fmt::Debug for ObjectReceiverLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectReceiverLogger").finish()
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

struct HeaderInjector<'a>(pub &'a mut HashMap<String, String>);
impl Injector for HeaderInjector<'_> {
    fn set(&mut self, key: &str, value: String) {
        self.0.insert(key.to_string(), value);
    }
}

impl ObjectReceiverLogger {
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
            0 => "FDT",
            _ => "FLUTEObject",
        };

        let mut span;
        if let Some(propagator) = propagator {
            let parent_cx = Self::extract_context_from_propagator(propagator);
            span = tracer
                .span_builder(name)
                .with_kind(SpanKind::Consumer)
                .start_with_context(&tracer, &parent_cx)
        } else {
            span = tracer
                .span_builder(name)
                .with_kind(SpanKind::Consumer)
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

        let cx = Context::default().with_span(span);
        Self { cx }
    }

    pub fn get_propagator(&self) -> HashMap<String, String> {
        let propagator = global::get_text_map_propagator(|propagator| {
            let mut headers = HashMap::new();
            propagator.inject_context(&self.cx, &mut HeaderInjector(&mut headers));
            headers
        });

        propagator
    }
    /*
    pub fn block_span(&mut self) -> BoxedSpan {
        let tracer = global::tracer("FluteLogger");
        tracer.start_with_context("block", &self.cx)
    }
    */

    pub fn fdt_attached(&mut self) -> BoxedSpan {
        let tracer = global::tracer("FluteLogger");
        tracer.start_with_context("fdt_attached", &self.cx)
    }

    pub fn complete(&mut self) -> BoxedSpan {
        let tracer = global::tracer("FluteLogger");

        let span = self.cx.span();
        span.set_status(Status::Ok);

        tracer.start_with_context("complete", &self.cx)
    }

    pub fn interrupted(&mut self, description: &str) -> BoxedSpan {
        let tracer = global::tracer("FluteLogger");

        let span = self.cx.span();
        span.set_status(Status::Ok);

        span.set_attribute(KeyValue::new("description", description.to_string()));
        tracer.start_with_context("interrupted", &self.cx)
    }

    pub fn error(&mut self, description: &str) -> BoxedSpan {
        let tracer = global::tracer("FluteLogger");

        let span = self.cx.span();
        span.set_status(Status::Error {
            description: std::borrow::Cow::Owned(description.to_string()),
        });

        span.set_attribute(KeyValue::new("error_description", description.to_string()));
        tracer.start_with_context("error", &self.cx)
    }
}
