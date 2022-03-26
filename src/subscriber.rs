use tracing::Event;
use tracing_core::{span, Interest};

use {
    tracing_core::{Collect, Metadata},
    tracing_subscriber::subscribe::{Context, Subscribe},
};

// FUTURE: make this work for serialized events (e.g. tracing-memory). this will
//         likely be a huge chunk of work of its own, because it effectively
//         means designing the serialization format in order to abstract over it
//         and the tracing context / subscriber registry storage implementation.
pub trait Filter<C> {
    fn interest(&self, metadata: &Metadata<'_>) -> Interest {
        let _ = metadata;
        Interest::sometimes()
    }

    fn enabled(&self, metadata: &Metadata<'_>, ctx: Context<'_, C>) -> bool {
        let _ = (metadata, ctx);
        true
    }

    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, C>) {
        let _ = (attrs, id, ctx);
    }

    fn on_enter(&self, id: &span::Id, ctx: Context<'_, C>) {
        let _ = (id, ctx);
    }

    fn on_record(&self, id: &span::Id, values: &span::Record<'_>, ctx: Context<'_, C>) {
        let _ = (id, values, ctx);
    }

    fn on_exit(&self, id: &span::Id, ctx: Context<'_, C>) {
        let _ = (id, ctx);
    }

    fn on_close(&self, id: &span::Id, ctx: Context<'_, C>) {
        let _ = (id, ctx);
    }

    fn event_enabled(&self, event: &Event<'_>, ctx: Context<'_, C>) -> bool {
        let _ = (event, ctx);
        true
    }
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
