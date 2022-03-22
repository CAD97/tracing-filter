use {
    tracing_core::{Collect, Metadata},
    tracing_subscriber::subscribe::{Context, Subscribe},
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

impl<C: Collect, F: 'static + Filter<C>> Subscribe<C> for FilterSubscriber<F> {
    fn enabled(&self, metadata: &Metadata<'_>, ctx: Context<'_, C>) -> bool {
        self.filter.enabled(metadata, ctx)
    }
}
