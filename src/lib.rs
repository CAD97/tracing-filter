pub mod filter;
pub mod simple;

pub(crate) const DEFAULT_ENV: &str = "RUST_LOG";

#[doc(no_inline)]
pub use self::filter::FilterSubscriber;
