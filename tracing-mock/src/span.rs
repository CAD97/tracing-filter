use tracing::Collect;
use tracing_subscriber::registry::{LookupSpan, SpanRef};

#[derive(Debug)]
pub struct MockSpan {}

impl MockSpan {
    pub(crate) fn assert<C: Collect + for<'a> LookupSpan<'a>>(self, span: SpanRef<'_, C>) {
        unimplemented!("assert(Span)");
    }
}
