//! Support for simple filters, which match `env_logger`'s filter format.

use {
    crate::{Diagnostics, DEFAULT_ENV},
    compact_str::CompactString,
    std::{borrow::Cow, cmp, env, ffi::OsStr, fmt},
    tracing_core::{Interest, LevelFilter, Metadata},
    tracing_subscriber::layer::Context,
};

mod parse;

/// A filter matching the semantics of the [`env_logger`] crate's filter format.
///
/// [`env_logger`]: https://docs.rs/env_logger/0.9.0/env_logger/
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
#[derive(Debug, Default)]
pub struct Filter {
    directives: Vec<Directive>,
    regex: Option<regex::Regex>,
}

#[derive(Debug, PartialEq, Eq)]
struct Directive {
    target: Option<CompactString>,
    level: LevelFilter,
}

impl fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for directive in &*self.directives {
            if let Some(target) = &directive.target {
                write!(f, "{}=", target)?;
            }
            write!(f, "{},", directive.level)?;
        }

        if let Some(regex) = &self.regex {
            write!(f, "/{}", regex)?;
        }

        Ok(())
    }
}

impl Filter {
    /// Create a new filter, ignoring any invalid directives. It is highly
    /// recommended you use [`Self::parse`] instead, and display the warnings for
    /// ignored directives.
    pub fn new(spec: &str) -> Self {
        spec.parse().unwrap_or_else(|_| Self::empty())
    }

    /// Create an empty filter (i.e. one that filters nothing out).
    pub const fn empty() -> Self {
        Self {
            directives: Vec::new(),
            regex: None,
        }
    }

    /// Create a filter from the default `RUST_LOG` environment.
    pub fn from_default_env() -> Option<(Self, Option<Diagnostics<'static>>)> {
        Self::from_env(DEFAULT_ENV)
    }

    /// Create a filter from the environment.
    pub fn from_env(key: impl AsRef<OsStr>) -> Option<(Self, Option<Diagnostics<'static>>)> {
        let s = env::var(key).ok()?;
        let (filter, err) = Self::parse(&s);
        Some((
            filter.unwrap_or_default(),
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
    // TODO: parse[_default]_env; configure default then override by environment

    /// Add a new filter directive.
    pub fn add_directive<'a>(
        &mut self,
        target: Option<impl Into<Cow<'a, str>>>,
        level: impl Into<LevelFilter>,
    ) {
        let target = target.map(Into::into).map(Into::into);
        let level = level.into();
        let directive = Directive { target, level };
        let ix = self.directives.binary_search_by(|x: &Directive| {
            let a = x.target.as_ref().map(|x| x.len()).unwrap_or(0);
            let b = directive.target.as_ref().map(|x| x.len()).unwrap_or(0);
            match a.cmp(&b) {
                cmp::Ordering::Equal => x.target.cmp(&directive.target),
                ordering => ordering,
            }
        });
        match ix {
            Ok(ix) => self.directives[ix] = directive,
            Err(ix) => self.directives.insert(ix, directive),
        }
    }

    /// Builder-API version of [`Self::add_directive`].
    pub fn with_directive<'a>(
        mut self,
        target: Option<impl Into<Cow<'a, str>>>,
        level: impl Into<LevelFilter>,
    ) -> Self {
        self.add_directive(target, level);
        self
    }

    /// Add a new filter directive at the given level.
    pub fn add_level(&mut self, level: impl Into<LevelFilter>) {
        self.add_directive(None::<&str>, level);
    }

    /// Builder-API version of [`Self::add_level`].
    pub fn with_level(mut self, level: impl Into<LevelFilter>) -> Self {
        self.add_level(level);
        self
    }

    /// Add a new filter directive for a given target at a given level.
    pub fn add_target<'a>(
        &mut self,
        target: impl Into<Cow<'a, str>>,
        level: impl Into<LevelFilter>,
    ) {
        self.add_directive(Some(target), level);
    }

    /// Builder-API version of [`Self::add_target`].
    pub fn with_target<'a>(
        mut self,
        target: impl Into<Cow<'a, str>>,
        level: impl Into<LevelFilter>,
    ) -> Self {
        self.add_directive(Some(target), level);
        self
    }

    /// Add a regex filter to this filter.
    ///
    /// # Panics
    ///
    /// Panics if a regex filter has already been set.
    pub fn add_regex(&mut self, regex: regex::Regex) {
        match &self.regex {
            Some(_) => panic!("set `tracing_filter::simple::Filter` regex that was already set"),
            None => self.regex = Some(regex),
        }
    }

    /// Builder-API version of [`Self::add_regex`].
    pub fn with_regex(mut self, regex: regex::Regex) -> Self {
        self.add_regex(regex);
        self
    }

    fn is_enabled(&self, metadata: &Metadata<'_>) -> bool {
        // this code is adapted directly from env_logger 0.9.0
        // env_logger is licensed under MIT OR Apache-2.0

        let level = *metadata.level();
        let target = metadata.target();

        if self.directives.is_empty() {
            return level <= LevelFilter::ERROR;
        }

        for directive in self.directives.iter().rev() {
            match &directive.target {
                Some(name) if !target.starts_with(&**name) => {},
                Some(..) | None => return level <= directive.level,
            }
        }

        false
    }
}

impl<S> crate::Filter<S> for Filter {
    fn callsite_enabled(&self, metadata: &Metadata<'_>) -> Interest {
        if self.is_enabled(metadata) {
            Interest::always()
        } else {
            Interest::never()
        }
    }

    fn enabled(&self, metadata: &Metadata<'_>, _ctx: &Context<'_, S>) -> bool {
        self.is_enabled(metadata)
    }
}
