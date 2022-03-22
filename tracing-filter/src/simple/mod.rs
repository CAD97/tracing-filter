use std::cmp;

use sorted_vec::SortedVec;

use {
    crate::DEFAULT_ENV,
    miette::ErrReport,
    smartstring::alias::String as SmartString,
    std::{borrow::Cow, env, ffi::OsStr, fmt},
    tracing_core::LevelFilter,
};

mod parse;

#[derive(Debug, Default)]
pub struct Filter {
    directives: SortedVec<Directive>,
    #[cfg(all(feature = "regex"))]
    regex: Option<regex::Regex>,
}

#[derive(Debug, PartialEq, Eq)]
struct Directive {
    target: Option<SmartString>,
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
        for directive in &*self.directives {
            if let Some(target) = &directive.target {
                write!(f, "{}=", target)?;
            }
            write!(f, "{},", directive.level)?;
        }

        #[cfg(feature = "regex")]
        if let Some(regex) = &self.regex {
            write!(f, "/{}", regex)?;
        }

        Ok(())
    }
}

impl Filter {
    pub fn new() -> Self {
        Self {
            directives: SortedVec::new(),
            #[cfg(feature = "regex")]
            regex: None,
        }
    }

    pub fn from_env(key: impl AsRef<OsStr>) -> (Self, Option<ErrReport>) {
        if let Ok(s) = env::var(key) {
            let (filter, err) = Self::parse(s);
            (filter.unwrap_or_default(), err)
        } else {
            (Self::new(), None)
        }
    }

    pub fn from_default_env() -> (Self, Option<ErrReport>) {
        Self::from_env(DEFAULT_ENV)
    }

    pub fn add_directive<'a>(
        &mut self,
        target: Option<impl Into<Cow<'a, str>>>,
        level: impl Into<LevelFilter>,
    ) {
        let target = target.map(Into::into).map(Into::into);
        let level = level.into();
        self.directives.insert(Directive { target, level });
    }

    pub fn with_directive<'a>(
        mut self,
        target: Option<impl Into<Cow<'a, str>>>,
        level: impl Into<LevelFilter>,
    ) -> Self {
        self.add_directive(target, level);
        self
    }

    pub fn add_level(&mut self, level: impl Into<LevelFilter>) {
        self.add_directive(None::<&str>, level);
    }

    pub fn with_level(mut self, level: impl Into<LevelFilter>) -> Self {
        self.add_level(level);
        self
    }

    pub fn add_target<'a>(
        &mut self,
        target: impl Into<Cow<'a, str>>,
        level: impl Into<LevelFilter>,
    ) {
        self.add_directive(Some(target), level);
    }

    pub fn with_target<'a>(
        mut self,
        target: impl Into<Cow<'a, str>>,
        level: impl Into<LevelFilter>,
    ) -> Self {
        self.add_directive(Some(target), level);
        self
    }

    #[cfg(feature = "regex")]
    pub fn add_regex(&mut self, regex: regex::Regex) {
        match &self.regex {
            Some(_) => panic!("set `tracing_filter::simple::Filter` regex that was already set"),
            None => self.regex = Some(regex),
        }
    }

    #[cfg(feature = "regex")]
    pub fn with_regex(mut self, regex: regex::Regex) -> Self {
        self.add_regex(regex);
        self
    }
}

impl<C> crate::subscriber::Filter<C> for Filter {
    fn enabled(
        &self,
        metadata: &tracing::Metadata<'_>,
        _ctx: tracing_subscriber::subscribe::Context<'_, C>,
    ) -> bool {
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