use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    thread,
};
use tracing::Collect;
use tracing_subscriber::Subscribe;

pub fn subscribe() -> MockSubscribe {
    MockSubscribe {
        expect: Arc::default(),
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
    expect: Arc<AtomicUsize>,
    name: Arc<str>,
}

impl<C: Collect> Subscribe<C> for MockSubscribe {
    fn on_event(
        &self,
        event: &tracing::Event<'_>,
        ctx: tracing_subscriber::subscribe::Context<'_, C>,
    ) {
        let _ = (event, ctx);
        if self.expect.fetch_sub(1, Ordering::SeqCst) == 0 {
            panic!("[{}] received unexpected event", self.name);
        }
    }
}

impl MockSubscribe {
    pub fn expect_event(&self) {
        if self.expect.fetch_add(1, Ordering::SeqCst) != 0 {
            panic!("[{}] did not receive expected event", self.name);
        }
    }

    pub fn expect_no_event(&self) {
        if self.expect.load(Ordering::SeqCst) != 0 {
            panic!("[{}] did not receive expected event", self.name);
        }
    }

    pub fn assert_clear(&self) {
        if self.expect.load(Ordering::SeqCst) != 0 {
            panic!("[{}] did not receive expected event", self.name);
        }
    }
}
