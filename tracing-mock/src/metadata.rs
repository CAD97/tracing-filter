use tracing::{Level, Metadata};

pub fn mock() -> MockMetadata {
    MockMetadata {
        ..Default::default()
    }
}

#[derive(Debug, Default)]
pub struct MockMetadata {
    name: Option<String>,
    level: Option<Level>,
    target: Option<String>,
}

impl MockMetadata {
    pub fn name(self, name: String) -> Self {
        Self {
            name: Some(name),
            ..self
        }
    }

    pub fn level(self, level: Level) -> Self {
        Self {
            level: Some(level),
            ..self
        }
    }

    pub fn target(self, target: String) -> Self {
        Self {
            target: Some(target),
            ..self
        }
    }
}

impl MockMetadata {
    pub(crate) fn assert(self, metadata: &Metadata<'_>) {
        if let Some(name) = self.name {
            assert_eq!(name, metadata.name());
        }
        if let Some(level) = self.level {
            assert_eq!(&level, metadata.level());
        }
        if let Some(target) = self.target {
            assert_eq!(target, metadata.target());
        }
    }
}
