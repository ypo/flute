use opentelemetry::{
    global::{self, BoxedSpan},
    trace::{Span, Status, TraceContextExt, Tracer},
    Context, KeyValue,
};

pub struct ObjectReceiverLogger {
    cx: Context,
}

impl std::fmt::Debug for ObjectReceiverLogger {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectReceiverLogger").finish()
    }
}

impl ObjectReceiverLogger {
    pub fn new(tsi: u64, toi: u128) -> Self {
        let tracer = global::tracer("ObjectReceiverLogger");
        let name = match toi {
            0 => "FDT",
            _ => "FLUTEObject",
        };
        let mut span = tracer.start(name);
        span.add_event(
            "object",
            vec![
                KeyValue::new("tsi", tsi.to_string()),
                KeyValue::new("toi", toi.to_string()),
            ],
        );
        let cx = Context::current_with_span(span);
        Self { cx }
    }

    pub fn block_span(&mut self) -> BoxedSpan {
        let tracer = global::tracer("ObjectReceiverLogger");
        tracer.start_with_context("block", &self.cx)
    }

    pub fn fdt_attached(&mut self) -> BoxedSpan {
        let tracer = global::tracer("ObjectReceiverLogger");
        tracer.start_with_context("fdt_attached", &self.cx)
    }

    pub fn complete(&mut self) -> BoxedSpan {
        let tracer = global::tracer("ObjectReceiverLogger");

        let span = self.cx.span();
        span.set_status(Status::Ok);

        tracer.start_with_context("complete", &self.cx)
    }

    pub fn error(&mut self, description: &str) -> BoxedSpan {
        let tracer = global::tracer("ObjectReceiverLogger");

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
