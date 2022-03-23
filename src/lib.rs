pub mod simple;
mod subscriber;

pub(crate) const DEFAULT_ENV: &str = "RUST_LOG";

pub use self::subscriber::{Filter, FilterSubscriber};
