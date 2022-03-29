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

pub mod legacy;
pub mod simple;
mod subscriber;

pub(crate) const DEFAULT_ENV: &str = "RUST_LOG";

pub use self::subscriber::{Filter, FilterSubscriber};

#[cfg(feature = "smallvec")]
type SmallVec<T> = smallvec::SmallVec<[T; 8]>;
#[cfg(not(feature = "smallvec"))]
type SmallVec<T> = Vec<T>;
