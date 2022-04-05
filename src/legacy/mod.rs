//! Support for legacy filters, which match tracing-subscriber's legacy
//! `EnvFilter` format.
//!
//! [`EnvFilter`]: https://docs.rs/tracing-subscriber/0.3/tracing_subscriber/struct.EnvFilter.html

use {
    crate::DEFAULT_ENV,
    miette::ErrReport,
    std::{cell::RefCell, collections::HashMap, env, ffi::OsStr, fmt, sync::RwLock},
    thread_local::ThreadLocal,
    tracing_core::{callsite, span, Interest, LevelFilter, Metadata, Subscriber},
    tracing_subscriber::layer::Context,
};

mod directive;
mod matcher;
mod parse;
#[cfg(test)]
mod tests;

/// A filter matching tracing-subscriber's legacy [`EnvFilter`] format.
///
/// [`EnvFilter`]: https://docs.rs/tracing-subscriber/0.3/tracing_subscriber/struct.EnvFilter.html
#[derive(Debug)]
pub struct Filter {
    scope: ThreadLocal<RefCell<Vec<LevelFilter>>>,
    statics: directive::Statics,
    dynamics: directive::Dynamics,
    by_id: RwLock<HashMap<span::Id, matcher::SpanMatcher>>,
    by_cs: RwLock<HashMap<callsite::Identifier, matcher::CallsiteMatcher>>,
}

impl Filter {
    /// Create a new filter, ignoring any invalid directives. It is highly
    /// recommended you use [`Self::parse`] instead, and display the warnings for
    /// ignored directives.
    pub fn new(spec: impl AsRef<str> + Into<String>) -> Self {
        Self::parse(spec).0
    }

    /// Create a filter from the default `RUST_LOG` environment.
    pub fn from_default_env() -> (Self, Option<ErrReport>) {
        Self::from_env(DEFAULT_ENV)
    }

    /// Create a filter from the environment.
    pub fn from_env(key: impl AsRef<OsStr>) -> (Self, Option<ErrReport>) {
        if let Ok(s) = env::var(key) {
            let (filter, err) = Self::parse(s);
            (filter, err)
        } else {
            Self::parse("")
        }
    }
}

impl Filter {
    fn has_dynamics(&self) -> bool {
        !self.dynamics.directives.is_empty()
    }

    fn cares_about_span(&self, span: &span::Id) -> bool {
        let by_id = try_lock!(self.by_id.read(), else return false);
        by_id.contains_key(span)
    }

    fn base_interest(&self) -> Interest {
        if self.has_dynamics() {
            Interest::sometimes()
        } else {
            Interest::never()
        }
    }
}

impl<S: Subscriber> crate::Filter<S> for Filter {
    fn enabled(&self, metadata: &Metadata<'_>, _ctx: &Context<'_, S>) -> bool {
        let level = metadata.level();

        // Is it possible for a dynamic filter directive to enable this event?
        // If not, we can avoid the thread local access & iterating over the
        // spans in the current scope.

        if self.has_dynamics() && self.dynamics.level >= *level {
            if metadata.is_span() {
                // If the metadata is a span, see if we care about its callsite.
                let enabled = self
                    .by_cs
                    .read()
                    .map(|cs| cs.contains_key(&metadata.callsite()))
                    .unwrap_or_default();
                if enabled {
                    return true;
                }
            }

            let enabled = self
                .scope
                .get_or_default()
                .borrow()
                .iter()
                .any(|filter| filter >= level);
            if enabled {
                return true;
            }
        }

        // Otherwise, fall back to checking if the callsite is statically enabled.
        self.statics.enabled(metadata)
    }

    fn callsite_enabled(&self, metadata: &Metadata<'_>) -> tracing_core::Interest {
        if self.has_dynamics() && metadata.is_span() {
            // If this metadata describes a span, first, check if there is a
            // dynamic filter that should be constructed for it. If so, it
            // should always be enabled, since it influences filtering.
            if let Some(matcher) = self.dynamics.matcher(metadata) {
                let mut by_cs = try_lock!(self.by_cs.write(), else return self.base_interest());
                by_cs.insert(metadata.callsite(), matcher);
                return Interest::always();
            }
        }

        if self.statics.enabled(metadata) {
            Interest::always()
        } else {
            self.base_interest()
        }
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        if self.dynamics.has_value_filters() {
            // If we perform any filtering on span field *values*, we will
            // enable *all* spans, because their field values are not known
            // until recording.
            return Some(LevelFilter::TRACE);
        }
        std::cmp::max(self.statics.level.into(), self.dynamics.level.into())
    }

    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, _: Context<'_, S>) {
        let by_cs = try_lock!(self.by_cs.read());
        if let Some(cs) = by_cs.get(&attrs.metadata().callsite()) {
            let span = cs.to_span_matcher(attrs);
            try_lock!(self.by_id.write()).insert(id.clone(), span);
        }
    }

    fn on_record(&self, id: &span::Id, values: &span::Record<'_>, _ctx: Context<'_, S>) {
        if let Some(span) = try_lock!(self.by_id.read()).get(id) {
            span.record_update(values);
        }
    }

    fn on_enter(&self, id: &span::Id, _: Context<'_, S>) {
        // We _could_ push IDs to the stack instead, and use that to allow
        // changing the filter while a span is already entered. But that seems
        // much less efficient...
        if let Some(span) = try_lock!(self.by_id.read()).get(id) {
            self.scope.get_or_default().borrow_mut().push(span.level());
        }
    }

    fn on_exit(&self, id: &span::Id, _ctx: Context<'_, S>) {
        if self.cares_about_span(id) {
            self.scope.get_or_default().borrow_mut().pop();
        }
    }

    fn on_close(&self, id: span::Id, _ctx: Context<'_, S>) {
        // If we don't need a write lock, avoid taking one.
        if !self.cares_about_span(&id) {
            return;
        }

        let mut by_id = try_lock!(self.by_id.write());
        by_id.remove(&id);
    }
}

impl fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut wrote_any = false;

        for directive in self.statics.directives.iter() {
            if wrote_any {
                write!(f, ",")?;
            }
            write!(f, "{directive}")?;
            wrote_any = true;
        }

        for directive in self.dynamics.directives.iter() {
            if wrote_any {
                write!(f, ",")?;
            }
            write!(f, "{directive}")?;
            wrote_any = true;
        }

        Ok(())
    }
}
