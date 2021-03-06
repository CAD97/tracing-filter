//! Support for legacy filters, which match tracing-subscriber's legacy
//! `EnvFilter` format.
//!
//! [`EnvFilter`]: https://docs.rs/tracing-subscriber/0.3/tracing_subscriber/struct.EnvFilter.html

use {
    self::matcher::SpanMatcher,
    crate::{Diagnostics, DEFAULT_ENV},
    std::{cell::RefCell, collections::HashMap, env, ffi::OsStr, fmt, sync::RwLock},
    thread_local::ThreadLocal,
    tracing_core::{callsite, span, Interest, LevelFilter, Metadata, Subscriber},
    tracing_subscriber::{
        layer::Context,
        registry::{LookupSpan, SpanRef},
    },
};

mod directive;
mod matcher;
mod parse;
#[cfg(test)]
mod tests;

/// A filter matching tracing-subscriber's legacy [`EnvFilter`] format.
///
/// [`EnvFilter`]: https://docs.rs/tracing-subscriber/0.3/tracing_subscriber/struct.EnvFilter.html
///
/// # Example
///
/// Use the `RUST_LOG` filter if it is set, but provide a fallback filter if the
/// environment variable is not set.
///
/// ```rust
/// # use tracing_filter::{DiagnosticsTheme, legacy::Filter};
/// # use {tracing::{error, warn}, tracing_subscriber::prelude::*};
/// let (filter, diagnostics) = Filter::from_default_env()
///     .unwrap_or_else(|| Filter::parse("noisy_crate=warn,info"));
///
/// tracing_subscriber::registry()
///     .with(filter.layer())
///     .with(tracing_subscriber::fmt::layer())
///     .init();
///
/// if let Some(diagnostics) = diagnostics {
///     if let Some(error) = diagnostics.error(DiagnosticsTheme::default()) {
///         error!("{error}");
///     }
///     if let Some(warn) = diagnostics.warn(DiagnosticsTheme::default()) {
///         warn!("{warn}");
///     }
/// }
/// ```
#[derive(Debug)]
pub struct Filter {
    scope: ThreadLocal<RefCell<Vec<LevelFilter>>>,
    statics: directive::Statics,
    dynamics: directive::Dynamics,
    by_cs: RwLock<HashMap<callsite::Identifier, matcher::CallsiteMatcher>>,
}

impl Filter {
    /// Create a new filter, ignoring any invalid directives. It is highly
    /// recommended you use [`Self::parse`] instead, and display the warnings for
    /// ignored directives.
    pub fn new(spec: &str) -> Self {
        Self::parse(spec).0
    }

    /// Create an empty filter (i.e. one that filters nothing out).
    pub fn empty() -> Self {
        Self::parse("").0
    }

    /// Create a filter from the default `RUST_LOG` environment.
    pub fn from_default_env() -> Option<(Self, Option<Diagnostics<'static>>)> {
        Self::from_env(DEFAULT_ENV)
    }

    /// Create a filter from the environment.
    ///
    /// Returns `None` if the environment variable is not set.
    pub fn from_env(key: impl AsRef<OsStr>) -> Option<(Self, Option<Diagnostics<'static>>)> {
        let s = env::var(key).ok()?;
        let (filter, err) = Self::parse(&s);
        Some((
            filter,
            match err {
                None => None,
                Some(x) => Some({
                    Diagnostics {
                        error: x.error,
                        ignored: x.ignored,
                        disabled: x.disabled,
                        source: s.into(),
                    }
                }),
            },
        ))
    }

    /// Lift this filter to a filter layer.
    pub fn layer(self) -> crate::FilterLayer<Self> {
        crate::FilterLayer::new(self)
    }
}

impl Filter {
    fn has_dynamics(&self) -> bool {
        !self.dynamics.directives.is_empty()
    }

    fn cares_about_span<R: for<'a> LookupSpan<'a>>(&self, span: SpanRef<'_, R>) -> bool {
        let ext = span.extensions();
        ext.get::<SpanMatcher>().is_some()
    }

    fn base_interest(&self) -> Interest {
        if self.has_dynamics() {
            Interest::sometimes()
        } else {
            Interest::never()
        }
    }
}

impl<S: Subscriber + for<'a> LookupSpan<'a>> crate::Filter<S> for Filter {
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

    fn on_new_span(&self, attrs: &span::Attributes<'_>, id: &span::Id, ctx: Context<'_, S>) {
        let by_cs = try_lock!(self.by_cs.read());
        if let Some(cs) = by_cs.get(&attrs.metadata().callsite()) {
            let span = ctx.span(id).expect("span should be registered");
            let matcher = cs.to_span_matcher(attrs);
            span.extensions_mut().insert(matcher);
        }
    }

    fn on_record(&self, id: &span::Id, values: &span::Record<'_>, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("span should be registered");
        let ext = span.extensions();
        if let Some(matcher) = ext.get::<SpanMatcher>() {
            matcher.record_update(values);
        }
    }

    fn on_enter(&self, id: &span::Id, ctx: Context<'_, S>) {
        // We _could_ push IDs to the stack instead, and use that to allow
        // changing the filter while a span is already entered. But that seems
        // much less efficient...
        let span = ctx.span(id).expect("span should be registered");
        let ext = span.extensions();
        if let Some(matcher) = ext.get::<SpanMatcher>() {
            self.scope
                .get_or_default()
                .borrow_mut()
                .push(matcher.level());
        }
    }

    fn on_exit(&self, id: &span::Id, ctx: Context<'_, S>) {
        let span = ctx.span(id).expect("span should be registered");
        if self.cares_about_span(span) {
            self.scope.get_or_default().borrow_mut().pop();
        }
    }

    fn on_close(&self, id: span::Id, ctx: Context<'_, S>) {
        let span = ctx.span(&id).expect("span should be registered");
        span.extensions_mut().remove::<SpanMatcher>();
    }
}

impl fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut wrote_any = false;

        for directive in self.statics.directives.iter() {
            if wrote_any {
                write!(f, ",")?;
            }
            write!(f, "{}", directive)?;
            wrote_any = true;
        }

        for directive in self.dynamics.directives.iter() {
            if wrote_any {
                write!(f, ",")?;
            }
            write!(f, "{}", directive)?;
            wrote_any = true;
        }

        Ok(())
    }
}
