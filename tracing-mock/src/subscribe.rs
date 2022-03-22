use bitflags::bitflags;
use std::{
    collections::VecDeque,
    sync::{Arc, Mutex},
    thread,
};
use tracing::{Collect, Event, Metadata};
use tracing_core::span;
use tracing_subscriber::{registry::LookupSpan, subscribe::Context, Subscribe};
use tracking_mock::MockSpan;

use crate::{MockEvent, MockFields};

pub fn mock() -> MockSubscribe {
    MockSubscribe {
        expected: Arc::new(Mutex::new(VecDeque::new())),
        name: thread::current().name().unwrap_or("mock_layer").to_string(),
        strict: ExpectKind::empty(),
    }
}

#[derive(Debug)]
pub enum Expect {
    NewSpan(MockSpan, MockFields),
    Record(MockSpan, MockFields),
    Event(MockEvent),
    Enter(MockSpan),
    Exit(MockSpan),
    Close(MockSpan),
}

bitflags! {
    pub struct ExpectKind: u8 {
        const NEW_SPAN = 1 << 0;
        const RECORD = 1 << 1;
        const FOLLOWS_FROM = 1 << 2;
        const EVENT = 1 << 3;
        const ENTER = 1 << 4;
        const EXIT = 1 << 5;
        const CLOSE = 1 << 6;
        const ID_CHANGE = 1 << 7;
    }
}

#[derive(Debug, Clone)]
pub struct MockSubscribe {
    pub(super) expected: Arc<Mutex<VecDeque<Expect>>>,
    pub(super) name: String,
    pub(super) strict: ExpectKind,
}

impl MockSubscribe {
    pub fn strict(mut self, kind: ExpectKind) -> Self {
        self.strict.set(kind, true);
        self
    }

    pub fn expect(&self, expectation: impl IntoIterator<Item = Expect>) {
        let mut expected = self.expected.lock().unwrap();
        if !expected.is_empty() {
            panic!("missing expected callbacks; {expected:#?}");
        }
        expected.extend(expectation);
    }

    pub fn finish(&self) {
        self.expect([]);
    }
}

impl<C: Collect + for<'a> LookupSpan<'a>> Subscribe<C> for MockSubscribe {
    fn register_callsite(&self, metadata: &'static Metadata<'static>) -> tracing_core::Interest {
        println!("[{}] register_callsite: {metadata:#?}", self.name);
        tracing_core::Interest::always()
    }

    fn enabled(&self, metadata: &Metadata<'_>, _ctx: Context<'_, C>) -> bool {
        println!("[{}] enabled: {metadata:#?}", self.name);
        true
    }

    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, C>) {
        println!(
            "[{}] new_span: id={id:?}; {:#?}",
            self.name,
            attrs.metadata(),
        );
        let span = ctx.span(id).unwrap();
        let mut expected = self.expected.lock().unwrap();
        if matches!(expected.front(), Some(Expect::NewSpan(..))) {
            if let Some(Expect::NewSpan(expected_span, expected_fields)) = expected.pop_front() {
                expected_span.assert(span);
                expected_fields.assert(attrs);
            } else {
                unreachable!()
            }
        } else if self.strict.contains(ExpectKind::NEW_SPAN) {
            panic!("unexpected new_span");
        }
    }

    fn on_record(&self, id: &span::Id, values: &span::Record<'_>, ctx: Context<'_, C>) {
        println!("[{}] record: id={id:?}; {:#?}", self.name, values);
        let span = ctx.span(id).unwrap();
        let mut expected = self.expected.lock().unwrap();
        if matches!(expected.front(), Some(Expect::Record(..))) {
            if let Some(Expect::Record(expected_span, expected_fields)) = expected.pop_front() {
                expected_span.assert(span);
                expected_fields.assert(values);
            } else {
                unreachable!()
            }
        } else if self.strict.contains(ExpectKind::RECORD) {
            panic!("unexpected on_record")
        }
    }

    fn on_follows_from(&self, latter: &span::Id, former: &span::Id, _ctx: Context<'_, C>) {
        println!("[{}] on_follows_from: {former:?} => {latter:?}", self.name);
        if self.strict.contains(ExpectKind::FOLLOWS_FROM) {
            unimplemented!("MockSubscribe::on_follows_from");
        }
    }

    fn on_event(&self, event: &Event<'_>, ctx: Context<'_, C>) {
        println!("[{}] event: {event:#?}", self.name);
        let mut expected = self.expected.lock().unwrap();
        if matches!(expected.front(), Some(Expect::Event(_))) {
            if let Some(Expect::Event(expected)) = expected.pop_front() {
                expected.assert(
                    event,
                    event.parent().map(|id| ctx.span(id).unwrap()).or_else(|| {
                        ctx.current_span()
                            .id()
                            .filter(|_| !event.is_root())
                            .and_then(|id| ctx.span(id))
                    }),
                );
            } else {
                unreachable!()
            }
        } else if self.strict.contains(ExpectKind::EVENT) {
            panic!("unexpected on_event");
        }
    }

    fn on_enter(&self, id: &span::Id, ctx: Context<'_, C>) {
        println!("[{}] enter: id={id:?}", self.name);
        let span = ctx.span(id).unwrap();
        let mut expected = self.expected.lock().unwrap();
        if matches!(expected.front(), Some(Expect::Enter(_))) {
            if let Some(Expect::Enter(expected)) = expected.pop_front() {
                expected.assert(span);
            } else {
                unreachable!()
            }
        } else if self.strict.contains(ExpectKind::ENTER) {
            panic!("unexpected on_enter");
        }
    }

    fn on_exit(&self, id: &span::Id, ctx: Context<'_, C>) {
        println!("[{}] exit: id={id:?}", self.name);
        let span = ctx.span(id).unwrap();
        let mut expected = self.expected.lock().unwrap();
        if matches!(expected.front(), Some(Expect::Exit(_))) {
            if let Some(Expect::Exit(expected)) = expected.pop_front() {
                expected.assert(span);
            } else {
                unreachable!()
            }
        } else if self.strict.contains(ExpectKind::EXIT) {
            panic!("unexpected on_enter");
        }
    }

    fn on_close(&self, id: span::Id, ctx: Context<'_, C>) {
        println!("[{}] enter: id={id:?}", self.name);
        let span = ctx.span(&id).unwrap();
        let mut expected = self.expected.lock().unwrap();
        if matches!(expected.front(), Some(Expect::Close(_))) {
            if let Some(Expect::Close(expected)) = expected.pop_front() {
                expected.assert(span);
            } else {
                unreachable!()
            }
        } else if self.strict.contains(ExpectKind::CLOSE) {
            panic!("unexpected on_enter");
        }
    }

    fn on_id_change(&self, old: &span::Id, new: &span::Id, _ctx: Context<'_, C>) {
        println!("[{}] id_change: old={old:?}; new={new:?}", self.name);
        if self.strict.contains(ExpectKind::ID_CHANGE) {
            unimplemented!("MockSubscribe::id_change");
        }
    }
}
