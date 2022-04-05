use {
    crate::Filter,
    tracing_core::{span, Event, Interest, LevelFilter, Metadata, Subscriber},
    tracing_subscriber::layer::{Context, Layer},
};

// FUTURE: make this work for serialized events (e.g. tracing-memory). this will
//         likely be a huge chunk of work of its own, because it effectively
//         means designing the serialization format in order to abstract over it
//         and the tracing context / subscriber registry storage implementation.
//         Also: not using the upstream Filter trait anymore.

/// A [`Layer`] which elevates a [`Filter`] from applying to a single
/// subscriber to the entire layered subscribe stack.
pub struct FilterLayer<F> {
    filter: F,
}

impl<F> FilterLayer<F> {
    pub fn new(filter: F) -> Self {
        Self { filter }
    }
}

impl<S: Subscriber, F: 'static + Filter<S>> Layer<S> for FilterLayer<F> {
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> Interest {
        self.filter.callsite_enabled(metadata)
    }

    fn enabled(&self, metadata: &Metadata<'_>, ctx: Context<'_, S>) -> bool {
        self.filter.enabled(metadata, &ctx)
    }

    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        self.filter.on_new_span(attrs, id, ctx);
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        self.filter.max_level_hint()
    }

    fn on_record(&self, id: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        self.filter.on_record(id, values, ctx)
    }

    fn on_event(&self, _event: &Event<'_>, _ctx: Context<'_, S>) {
        // FUTURE: allow event filtering; tokio-rs/tracing#2008
    }

    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        self.filter.on_enter(id, ctx)
    }

    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        self.filter.on_exit(id, ctx)
    }

    fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
        self.filter.on_close(id, ctx)
    }
}
