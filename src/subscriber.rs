use {
    tracing_core::{Collect, Metadata},
    tracing_subscriber::subscribe::{Context, Subscribe},
};

// FUTURE: make this work for serialized events (e.g. tracing-memory). this will
//         likely be a huge chunk of work of its own, because it effectively
//         means designing the serialization format in order to abstract over it
//         and the tracing context / subscriber registry storage implementation.
pub use tracing_subscriber::subscribe::Filter;

pub struct FilterSubscriber<F> {
    filter: F,
}

impl<F> FilterSubscriber<F> {
    pub fn new(filter: F) -> Self {
        Self { filter }
    }
}

impl<C: Collect, F: 'static + Filter<C>> Subscribe<C> for FilterSubscriber<F> {
    fn enabled(&self, metadata: &Metadata<'_>, ctx: Context<'_, C>) -> bool {
        self.filter.enabled(metadata, &ctx)
    }

    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> tracing_core::Interest {
        self.filter.callsite_enabled(metadata)
    }

    fn on_new_span(
        &self,
        attrs: &tracing_core::span::Attributes<'_>,
        id: &tracing_core::span::Id,
        ctx: Context<'_, C>,
    ) {
        self.filter.on_new_span(attrs, id, ctx);
    }

    fn max_level_hint(&self) -> Option<tracing_core::LevelFilter> {
        self.filter.max_level_hint()
    }

    fn on_record(
        &self,
        id: &tracing_core::span::Id,
        values: &tracing_core::span::Record<'_>,
        ctx: Context<'_, C>,
    ) {
        self.filter.on_record(id, values, ctx)
    }

    fn on_event(&self, _event: &tracing::Event<'_>, _ctx: Context<'_, C>) {
        // TODO: allow event filtering; tokio-rs/tracing#2008
    }

    fn on_enter(&self, id: &tracing_core::span::Id, ctx: Context<'_, C>) {
        self.filter.on_enter(id, ctx)
    }

    fn on_exit(&self, id: &tracing_core::span::Id, ctx: Context<'_, C>) {
        self.filter.on_exit(id, ctx)
    }

    fn on_close(&self, id: tracing_core::span::Id, ctx: Context<'_, C>) {
        self.filter.on_close(id, ctx)
    }
}
