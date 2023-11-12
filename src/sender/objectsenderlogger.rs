use std::time::SystemTime;

use opentelemetry::{
    global::{self},
    trace::{Span, TraceContextExt, TraceId, Tracer},
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

impl ObjectSenderLogger {
    pub fn new(
        endpoint: &UDPEndpoint,
        tsi: u64,
        toi: u128,
        fdt_instance_id: Option<u32>,
        now: SystemTime,
    ) -> Self {
        let tracer = global::tracer("FluteLogger");
        let name = match toi {
            0 => "FDT Transfer",
            _ => "Object Transfer",
        };

        let trace_id = endpoint.trace_id(tsi, toi, fdt_instance_id, now);

        log::info!(
            "Create {:?} {} {} {:?} trace_id={:?}",
            endpoint,
            tsi,
            toi,
            fdt_instance_id,
            trace_id
        );
        let mut span = tracer
            .span_builder(name)
            .with_trace_id(TraceId::from(trace_id))
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
        Self { _cx: cx }
    }
}
