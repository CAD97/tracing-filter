use {
    tracing_core::{Collect, Metadata},
    tracing_subscriber::{
        field::RecordFields,
        registry::{LookupSpan, SpanRef},
        subscribe::{Context, Subscribe},
    },
};

// FUTURE: make this work for serialized events (e.g. tracing-memory). this will
//         likely be a huge chunk of work of its own, because it effectively
//         means designing the serialization format in order to abstract over it
//         and the tracing context / subscriber registry storage implementation.
pub trait Filter<C> {
    fn enabled(&self, metadata: &Metadata<'_>, ctx: Context<'_, C>) -> bool;
}

pub struct FilterSubscriber<F> {
    filter: F,
}

impl<F> FilterSubscriber<F> {
    pub fn new(filter: F) -> Self {
        Self { filter }
    }
}

impl<C: Collect, F: 'static + Filter<C>> Subscribe<C> for FilterSubscriber<F>
where
    C: for<'a> LookupSpan<'a>,
{
    fn enabled(&self, metadata: &Metadata<'_>, ctx: Context<'_, C>) -> bool {
        self.filter.enabled(metadata, ctx)
    }

    fn on_new_span(
        &self,
        attrs: &tracing_core::span::Attributes<'_>,
        id: &tracing_core::span::Id,
        ctx: Context<'_, C>,
    ) {
        let span = ctx.span(id).expect("span not found");
        on_span(span, attrs);
    }

    fn on_record(
        &self,
        id: &tracing_core::span::Id,
        values: &tracing_core::span::Record<'_>,
        ctx: Context<'_, C>,
    ) {
        let span = ctx.span(id).expect("span not found");
        on_span(span, values);
    }

    // NB: event fields cannot be processed for filtering yet;
    //     see DEVNOTES "Event field matching"
}

fn on_span<'a, F: RecordFields, R: LookupSpan<'a>>(span: SpanRef<'a, R>, fields: &F) {
    // TODO: record span info. Not needed yet; we only have the simple filter.
    let _ = (span, fields);
}
