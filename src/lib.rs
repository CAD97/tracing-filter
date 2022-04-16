macro_rules! try_lock {
    ($lock:expr) => {
        try_lock!($lock, else return)
    };
    ($lock:expr, else $else:expr) => {
        if let Ok(lock) = $lock {
            lock
        } else if ::std::thread::panicking() {
            $else
        } else {
            panic!("lock poisoned")
        }
    }
}

mod diagnostics;
mod layer;
pub mod legacy;
pub mod simple;

pub(crate) const DEFAULT_ENV: &str = "RUST_LOG";

#[doc(inline)]
pub use self::{
    diagnostics::{Diagnostics, DiagnosticsTheme},
    layer::FilterLayer,
};
#[doc(no_inline)]
pub use tracing_subscriber::layer::Filter;

#[cfg(feature = "smallvec")]
type SmallVec<T> = smallvec::SmallVec<[T; 8]>;
#[cfg(not(feature = "smallvec"))]
type SmallVec<T> = Vec<T>;
