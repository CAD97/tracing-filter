use tracing_subscriber::field::RecordFields;

pub fn mock() -> MockFields {
    MockFields {}
}

#[derive(Debug)]
pub struct MockFields {}

impl MockFields {
    pub fn assert<R: RecordFields>(&self, fields: R) {
        unimplemented!("assert(Fields)");
    }
}
