use std::time::SystemTime;

use opentelemetry::{
    global::{self, BoxedSpan},
    trace::{Span, Status, TraceContextExt, TraceId, Tracer},
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

impl ObjectReceiverLogger {
    pub fn new(
        endpoint: &UDPEndpoint,
        tsi: u64,
        toi: u128,
        fdt_instance_id: Option<u32>,
        now: SystemTime,
    ) -> Self {
        let tracer = global::tracer("FluteLogger");
        let name = match toi {
            0 => "FDT",
            _ => "FLUTEObject",
        };

        let mut span = tracer
            .span_builder(name)
            .with_trace_id(TraceId::from(endpoint.trace_id(
                tsi,
                toi,
                fdt_instance_id,
                now,
            )))
            .start(&tracer);

        span.set_attribute(KeyValue::new("flute.toi", toi.to_string()));
        span.set_attribute(KeyValue::new("flute.tsi", tsi.to_string()));
        span.set_attribute(KeyValue::new("flute.port", endpoint.port.to_string()));
        if let Some(source_address) = endpoint.source_address.as_ref() {
            span.set_attribute(KeyValue::new(
                "flute.source_address",
                source_address.to_string(),
            ));
        }
        span.set_attribute(KeyValue::new(
            "flute.destination_group_address",
            endpoint.destination_group_address.to_string(),
        ));

        span.add_event("object", vec![KeyValue::new("start", "")]);
        let cx = Context::current_with_span(span);
        Self { cx }
    }

    pub fn block_span(&mut self) -> BoxedSpan {
        let tracer = global::tracer("FluteLogger");
        tracer.start_with_context("block", &self.cx)
    }

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

    pub fn error(&mut self, description: &str) -> BoxedSpan {
        let tracer = global::tracer("FluteLogger");

        let span = self.cx.span();
        span.set_status(Status::Error {
            description: std::borrow::Cow::Owned(description.to_string()),
        });

        span.add_event(
            "error",
            vec![KeyValue::new("error_description", description.to_string())],
        );

        tracer.start_with_context("error", &self.cx)
    }
}
