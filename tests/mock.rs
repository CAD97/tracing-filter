use std::{
    sync::{
        atomic::{AtomicU8, Ordering},
        Arc,
    },
    thread,
};
use tracing::Collect;
use tracing_subscriber::Subscribe;

pub fn subscribe() -> MockSubscribe {
    MockSubscribe {
        expect_event: Arc::default(),
        expect_span: Arc::default(),
        name: thread::current()
            .name()
            .unwrap_or("MockSubscribe")
            .to_string()
            .into_boxed_str()
            .into(),
    }
}

#[derive(Debug, Clone)]
pub struct MockSubscribe {
    expect_event: Arc<AtomicU8>,
    expect_span: Arc<AtomicU8>,
    name: Arc<str>,
}

impl<C: Collect> Subscribe<C> for MockSubscribe {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        ctx: tracing_subscriber::subscribe::Context<'_, C>,
    ) {
        let _ = (event, ctx);
        if self.expect_event.fetch_sub(1, Ordering::SeqCst) == 0 {
            panic!("[{}] received unexpected event", self.name);
        }
    }

    fn on_enter(
        &self,
        id: &tracing_core::span::Id,
        ctx: tracing_subscriber::subscribe::Context<'_, C>,
    ) {
        let _ = (id, ctx);
        if self.expect_span.fetch_sub(1, Ordering::SeqCst) == 0 {
            panic!("[{}] received unexpected span", self.name);
        }
    }
}

impl MockSubscribe {
    pub fn expect_event(&self) {
        if self.expect_event.fetch_add(1, Ordering::SeqCst) != 0 {
            panic!("[{}] did not receive expected event", self.name);
        }
    }

    pub fn expect_no_event(&self) {
        if self.expect_event.load(Ordering::SeqCst) != 0 {
            panic!("[{}] did not receive expected event", self.name);
        }
    }

    pub fn expect_span(&self) {
        if self.expect_span.fetch_add(1, Ordering::SeqCst) != 0 {
            panic!("[{}] did not receive expected span", self.name);
        }
    }

    pub fn expect_no_span(&self) {
        if self.expect_span.load(Ordering::SeqCst) != 0 {
            panic!("[{}] did not receive expected span", self.name);
        }
    }

    pub fn assert_clear(&self) {
        self.expect_no_event();
        self.expect_no_span();
    }
}
