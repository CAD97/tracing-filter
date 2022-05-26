//! Support for simple filters, which match `env_logger`'s filter format.

use {
    crate::{Diagnostics, DEFAULT_ENV},
    compact_str::CompactStr,
    sorted_vec::ReverseSortedVec,
    std::{borrow::Cow, cmp, cmp::Reverse, env, ffi::OsStr, fmt},
    tracing_core::{Interest, LevelFilter, Metadata},
    tracing_subscriber::layer::Context,
};

mod parse;

/// A filter matching the semantics of the `env_logger` crate's filter format.
#[derive(Debug, Default)]
pub struct Filter {
    directives: ReverseSortedVec<Directive>,
    regex: Option<regex::Regex>,
}

#[derive(Debug, PartialEq, Eq)]
struct Directive {
    target: Option<CompactStr>,
    level: LevelFilter,
}

impl PartialOrd for Directive {
    fn partial_cmp(&self, rhs: &Self) -> Option<cmp::Ordering> {
        Some(self.cmp(rhs))
    }
}

impl Ord for Directive {
    fn cmp(&self, rhs: &Self) -> cmp::Ordering {
        self.target
            .as_deref()
            .map(str::len)
            .cmp(&rhs.target.as_deref().map(str::len))
    }
}

impl fmt::Display for Filter {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for Reverse(directive) in &*self.directives {
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
    pub fn empty() -> Self {
        Self {
            directives: ReverseSortedVec::new(),
            regex: None,
        }
    }

    /// Create a filter from the default `RUST_LOG` environment.
    pub fn from_default_env() -> (Self, Option<Diagnostics<'static>>) {
        Self::from_env(DEFAULT_ENV)
    }

    /// Create a filter from the environment.
    pub fn from_env(key: impl AsRef<OsStr>) -> (Self, Option<Diagnostics<'static>>) {
        if let Ok(s) = env::var(key) {
            let (filter, err) = Self::parse(&s);
            (
                filter.unwrap_or_default(),
                #[allow(clippy::manual_map)]
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
            )
        } else {
            (Self::empty(), None)
        }
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
        self.directives.insert(Reverse(Directive { target, level }));
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

        for Reverse(directive) in self.directives.iter() {
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
