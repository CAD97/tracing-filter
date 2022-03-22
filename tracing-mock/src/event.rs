use tracing::{Collect, Event, Level};
use tracing_subscriber::registry::{LookupSpan, SpanRef};

use crate::{fields, metadata, MockFields, MockMetadata, MockSpan};

pub fn mock() -> MockEvent {
    MockEvent {
        metadata: metadata::mock(),
        spantrace: vec![],
        fields: fields::mock(),
    }
}

#[derive(Debug)]
pub struct MockEvent {
    metadata: MockMetadata,
    spantrace: Vec<MockSpan>,
    fields: MockFields,
}

impl MockEvent {
    pub fn name(self, name: String) -> Self {
        Self {
            metadata: self.metadata.name(name),
            ..self
        }
    }

    pub fn level(self, level: Level) -> Self {
        Self {
            metadata: self.metadata.level(level),
            ..self
        }
    }

    pub fn target(self, target: String) -> Self {
        Self {
            metadata: self.metadata.target(target),
            ..self
        }
    }

    pub fn field<T>(self, key: String, value: T) {
        unimplemented!("MockEvent::field")
    }
}

impl MockEvent {
    pub(crate) fn assert<C: Collect + for<'a> LookupSpan<'a>>(
        self,
        event: &Event<'_>,
        mut parent: Option<SpanRef<'_, C>>,
    ) {
        self.metadata.assert(event.metadata());
        // self.fields.assert(event);
        let mut spantrace = self.spantrace.into_iter();
        while let Some(mock) = spantrace.next() {
            match parent {
                Some(span) => {
                    parent = span.parent();
                    mock.assert(span);
                },
                None => {
                    panic!(
                        "missing event {} spantrace {:#?}",
                        event.metadata().name(),
                        spantrace.as_slice(),
                    );
                },
            }
        }
    }
}
